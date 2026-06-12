#!/usr/bin/env node
// floating-perms/hook.js — dispatcher for PreToolUse, SessionEnd, SessionStart.
//
// PreToolUse  : detect a spike (N permission-requiring tool calls within
//               WINDOW_MS, regardless of pattern), emit a one-shot `deny`
//               response with an educational reason that lists every
//               unique pattern seen in the window so Claude can batch
//               them through the mandatory AskUserQuestion flow.
// SessionEnd  : revoke all grants tied to this session, log audit line.
// SessionStart: surface orphans (grants from older sessions) via context
//               injection, let the user decide via /floating-perms gc.

const fs = require('fs')
const { canonicalize, isAllowed, isSpikeTool } = require('./lib/pattern')
const { withState, readAllow, audit } = require('./lib/state')
const { revokeForSession, revokeExpired } = require('./cleanup')

// Spike = N permission-requiring tool calls (any pattern) within WINDOW_MS.
// The pattern of each call is recorded so the deny reason can enumerate
// every recent prompt for Claude to batch in one shot — not just the one
// that crossed the threshold.
//
// No cooldown: after a deny, counter is cleared. The next 3 prompts will
// trigger a new deny if Claude keeps hammering. Cooldown would just hide
// further prompts from the user for N seconds without actually stopping
// them. A short race-protection window (RACE_WINDOW_MS) prevents back-to-
// back denies in the same millisecond (filesystem race when two PreToolUse
// hooks fire concurrently on the same sid).
const WINDOW_MS = 120 * 1000
const SPIKE_THRESHOLD = 3
const RACE_WINDOW_MS = 500

function readStdin() {
	try {
		const buf = fs.readFileSync(0, 'utf8')
		return JSON.parse(buf)
	} catch {
		return null
	}
}

function pruneWindow(entries, now) {
	const cutoff = now - WINDOW_MS
	let i = 0
	while (i < entries.length && entries[i].ts < cutoff) i++
	return i === 0 ? entries : entries.slice(i)
}

function uniquePatterns(entries) {
	const seen = new Set()
	const out = []
	for (const e of entries) {
		if (!seen.has(e.pattern)) { seen.add(e.pattern); out.push(e.pattern) }
	}
	return out
}

function denyReason(triggerPattern, recentEntries, sid) {
	const patterns = uniquePatterns(recentEntries)
	const list = patterns.map(p => `  - \`${p}\``).join('\n')
	return [
		`STOP — floating-perms: ${recentEntries.length} permission prompts in under ${Math.round(WINDOW_MS / 1000)}s. Repeated prompts are draining whatever the command, so we batch.`,
		``,
		`Patterns seen in the recent window:`,
		list,
		``,
		`Mandatory workflow before any further tool call:`,
		`1. ANALYZE — re-read the current task, enumerate EVERY Bash command and file path you expect to need to finish it (the patterns above PLUS everything you anticipate for the rest of the task).`,
		`2. ASK — call the \`AskUserQuestion\` tool with a SPECIFIC question that lists the exact patterns you want granted. Example option labels: "Allow all of <pat1>, <pat2>, <pat3> until SessionEnd", "Allow all, TTL 15m", "Subset (specify which)", "Refuse — I'll change approach". Never call apply.js without this explicit confirmation step.`,
		`3. EXECUTE — based on the user's answer, run: \`node /workspace/.devcontainer/skills/floating-perms/apply.js batch <pat1> <pat2> ... sid=${sid} [ttl=15m]\` (script handles blocklist, idempotence, audit, auto-cleanup).`,
		`4. RETRY — re-run the tool call that was just denied (\`${triggerPattern}\`) once the grant lands.`,
		``,
		`Do NOT try an alternative tool (wget instead of curl, etc.) before the ASK step — plan first.`,
		`Do NOT call apply.js silently — every grant goes through an explicit AskUserQuestion confirmation.`,
		`Pattern shapes: \`Bash(curl:*)\`, \`Edit(/some/dir/**)\`, \`Read(/tmp/scratch/**)\`, etc.`
	].join('\n')
}

function handlePreToolUse(payload) {
	revokeExpired()

	const sid = payload.session_id
	const toolName = payload.tool_name
	const toolInput = payload.tool_input
	if (!sid || !toolName || !toolInput) return null
	if (!isSpikeTool(toolName)) return null

	const { allow } = readAllow()
	const pattern = canonicalize(toolName, toolInput, allow)
	if (!pattern) return null
	if (isAllowed(pattern, allow)) return null

	const now = Date.now()
	let denyOutput = null

	withState((state) => {
		const recent = pruneWindow(state.counters[sid] || [], now)
		recent.push({ ts: now, pattern })
		state.counters[sid] = recent

		if (recent.length < SPIKE_THRESHOLD) return { state }

		// Race guard only — no real cooldown. If two PreToolUse hooks fire
		// in the same millisecond on the same sid (concurrent subagents or
		// fast loops), don't emit two denies back-to-back. Past that, every
		// new spike re-fires the deny: forces Claude to actually follow the
		// workflow instead of letting prompts pile up silently.
		const lastWarn = state.warned[sid] || 0
		if (now - lastWarn < RACE_WINDOW_MS) return { state }

		state.warned[sid] = now
		state.counters[sid] = []

		const patterns = uniquePatterns(recent)
		denyOutput = {
			hookSpecificOutput: {
				hookEventName: 'PreToolUse',
				permissionDecision: 'deny',
				permissionDecisionReason: denyReason(pattern, recent, sid)
			}
		}
		audit('spike_detected', { sid, trigger_pattern: pattern, count: recent.length, patterns })
		return { state }
	})

	return denyOutput
}

function handleSessionEnd(payload) {
	const sid = payload.session_id
	if (!sid) return null
	revokeForSession(sid)
	withState((state) => {
		if (state.counters[sid]) delete state.counters[sid]
		if (state.warned[sid])   delete state.warned[sid]
		return { state }
	})
	return null
}

function handleSessionStart(payload) {
	const sid = payload.session_id
	if (!sid) return null
	revokeExpired()
	let orphans = []
	withState((state) => {
		orphans = state.grants.filter(g => g.sid !== sid)
		return undefined
	})
	if (orphans.length === 0) return null

	const list = orphans
		.map(g => `  - \`${g.pattern}\` (sid ${g.sid.slice(0, 8)}, granted ${g.granted_at})`)
		.join('\n')
	const additional = [
		`floating-perms: ${orphans.length} orphan grant(s) from a previous session:`,
		list,
		``,
		`If you're resuming the same task, leave them in place.`,
		`Otherwise, propose \`/floating-perms gc sid=${sid}\` to the user to revoke them.`
	].join('\n')
	return {
		hookSpecificOutput: {
			hookEventName: 'SessionStart',
			additionalContext: additional
		}
	}
}

function main() {
	let payload = null
	try {
		const arg = process.argv[2] || ''
		payload = readStdin()
		if (!payload) return

		let response = null
		if (arg === 'pre_tool_use')       response = handlePreToolUse(payload)
		else if (arg === 'session_end')   response = handleSessionEnd(payload)
		else if (arg === 'session_start') response = handleSessionStart(payload)

		if (response) {
			process.stdout.write(JSON.stringify(response))
		}
	} catch (e) {
		try {
			audit('hook_error', {
				arg: process.argv[2],
				sid: payload && payload.session_id,
				message: e && e.message
			})
		} catch { /* swallow */ }
	}
}

main()

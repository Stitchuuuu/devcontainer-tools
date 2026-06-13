#!/usr/bin/env node
// floating-perms/hook.js — dispatcher for PermissionRequest, PreToolUse,
// SessionEnd, SessionStart.
//
// PermissionRequest : the actual Claude Code prompt fired — count it in
//                     state.counters[sid] (120 s window). Pure observer:
//                     this is the only handler that grows the counter.
// PreToolUse        : prune the window, count entries; if ≥ threshold AND
//                     the race-window has passed since the last warn, emit
//                     a deny with an educational reason that lists every
//                     unique pattern seen, then reset the counter.
// SessionEnd        : revoke all grants tied to this session, drop counter
//                     + warn slots, log audit line.
// SessionStart      : surface state-side + allow-side orphans via context
//                     injection, let the user decide via /floating-perms gc
//                     and /floating-perms reconcile.

const fs = require('fs')
const { canonicalize } = require('./lib/pattern')
const { withState, readAllow, audit, findFloatingSection } = require('./lib/state')
const { revokeForSession, revokeExpired } = require('./cleanup')

const BASELINE_PATH = '/workspace/.devcontainer/claude/settings.local.json'

const CANONICAL_BASH_RE      = /^Bash\([^:]+:\*\)$/
const CANONICAL_FILE_TOOL_RE = /^(Edit|Write|Read|NotebookEdit)\(\/[^)]+\/\*\*\)$/

function readBaselineAllow() {
	try {
		const buf = fs.readFileSync(BASELINE_PATH, 'utf8')
		const parsed = JSON.parse(buf)
		return parsed && parsed.permissions && Array.isArray(parsed.permissions.allow)
			? parsed.permissions.allow : []
	} catch {
		return []
	}
}

// Spike = N PermissionRequest events (any pattern) within WINDOW_MS.
// PermissionRequest is the truth source: it fires exactly when Claude
// Code shows a prompt, so the counter holds only events the user
// actually paid attention to. No prediction, no allow-list match.
//
// PreToolUse is the decision point: it prunes the window, checks the
// length, and emits a one-shot deny when the threshold is crossed.
// After a deny the counter is reset; a short race-protection window
// prevents back-to-back denies in the same millisecond when two
// PreToolUse fire concurrently on the same sid.
//
// Threshold = 2 ⇒ after the user has approved two prompts, the third
// tool call that would prompt is intercepted at PreToolUse and denied
// before the PermissionRequest dialog appears. The user pays attention
// cost twice, not three times.
const WINDOW_MS = 120 * 1000
const SPIKE_THRESHOLD = 2
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

function denyReason(recentEntries, sid) {
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
		`4. RETRY — once the grant lands, re-run the tool call that was just denied.`,
		``,
		`Do NOT try an alternative tool (wget instead of curl, etc.) before the ASK step — plan first.`,
		`Do NOT call apply.js silently — every grant goes through an explicit AskUserQuestion confirmation.`,
		`Pattern shapes: \`Bash(curl:*)\`, \`Edit(/some/dir/**)\`, \`Read(/tmp/scratch/**)\`, etc.`
	].join('\n')
}

function handlePermissionRequest(payload) {
	const sid = payload.session_id
	const toolName = payload.tool_name
	const toolInput = payload.tool_input
	if (!sid || !toolName || !toolInput) return null

	const pattern = canonicalize(toolName, toolInput)
	// Meta / unknown tools (ExitPlanMode, AskUserQuestion, TodoWrite,
	// Task, MCP, future tools) all canonicalize to null. They aren't
	// work-flow prompts — skip without counting or auditing.
	if (!pattern) return null

	const now = Date.now()
	withState((state) => {
		const recent = pruneWindow(state.counters[sid] || [], now)
		recent.push({ ts: now, pattern, tool_use_id: payload.tool_use_id })
		state.counters[sid] = recent
		return { state }
	})

	audit('permission_seen', { sid, pattern, tool_use_id: payload.tool_use_id })
	return null
}

function handlePreToolUse(payload) {
	revokeExpired()

	const sid = payload.session_id
	if (!sid) return null

	const now = Date.now()
	let denyOutput = null

	withState((state) => {
		const recent = pruneWindow(state.counters[sid] || [], now)
		state.counters[sid] = recent
		if (recent.length < SPIKE_THRESHOLD) return { state }

		// Race guard only — no real cooldown. If two PreToolUse hooks fire
		// in the same millisecond on the same sid (concurrent subagents or
		// fast loops), don't emit two denies back-to-back. Past that, any
		// new spike re-fires the deny.
		const lastWarn = state.warned[sid] || 0
		if (now - lastWarn < RACE_WINDOW_MS) return { state }

		state.warned[sid] = now
		state.counters[sid] = []

		const patterns = uniquePatterns(recent)
		denyOutput = {
			hookSpecificOutput: {
				hookEventName: 'PreToolUse',
				permissionDecision: 'deny',
				permissionDecisionReason: denyReason(recent, sid)
			}
		}
		audit('spike_detected', { sid, count: recent.length, patterns })
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

	let stateOrphans = []
	let stateGrantsPatterns = []
	withState((state) => {
		stateOrphans = state.grants.filter(g => g.sid !== sid)
		stateGrantsPatterns = state.grants.map(g => g.pattern)
		return undefined
	})

	// Detect orphans inside settings.local.json that don't have a matching
	// state.grants record. Two sources: entries between V1.2 sentinels
	// (authoritative) and pre-V1.2 canonical-form entries outside any
	// sentinel and outside the tracked baseline. Together this catches the
	// case where state.json was lost (rm, container rebuild) — the cleanup
	// would never fire because state is the source of truth, but settings
	// still holds the orphaned entries.
	const { allow } = readAllow()
	const section = findFloatingSection(allow)
	const inSection = section ? section.patterns : []
	const baseline = new Set(readBaselineAllow())
	const stateSet = new Set(stateGrantsPatterns)

	const looksFloating = allow.filter(p => {
		if (p === '' || p.startsWith('//')) return false
		return CANONICAL_BASH_RE.test(p) || CANONICAL_FILE_TOOL_RE.test(p)
	})
	const allowOrphans = looksFloating.filter(p => {
		if (stateSet.has(p)) return false
		if (baseline.has(p)) return false
		return true
	})

	if (stateOrphans.length === 0 && allowOrphans.length === 0) return null

	const lines = [`floating-perms — SessionStart reconciliation report:`, ``]
	if (stateOrphans.length > 0) {
		lines.push(`State-side orphans (${stateOrphans.length}) — grants from previous session(s) whose SessionEnd never fired:`)
		for (const g of stateOrphans) {
			lines.push(`  - \`${g.pattern}\` (sid ${g.sid.slice(0, 8)}, granted ${g.granted_at})`)
		}
		lines.push(``)
	}
	if (allowOrphans.length > 0) {
		lines.push(`Allow-side orphans (${allowOrphans.length}) — entries in settings.local.json with no matching state.grants record (state file was likely lost):`)
		for (const p of allowOrphans) {
			const src = inSection.includes(p) ? 'inside sentinels' : 'pre-V1.2 form'
			lines.push(`  - \`${p}\`  [${src}]`)
		}
		lines.push(``)
	}
	lines.push(`Resolution:`)
	if (stateOrphans.length > 0) lines.push(`  - State-side: \`/floating-perms gc sid=${sid}\` to revoke.`)
	if (allowOrphans.length > 0) lines.push(`  - Allow-side: \`/floating-perms reconcile sid=${sid}\` to inspect, then re-run with --auto to clean.`)
	lines.push(``)
	lines.push(`If you're resuming the same task, leave them in place — they'll behave like permanent grants until you choose to clean.`)

	return {
		hookSpecificOutput: {
			hookEventName: 'SessionStart',
			additionalContext: lines.join('\n')
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
		if      (arg === 'permission_request') response = handlePermissionRequest(payload)
		else if (arg === 'pre_tool_use')       response = handlePreToolUse(payload)
		else if (arg === 'session_end')        response = handleSessionEnd(payload)
		else if (arg === 'session_start')      response = handleSessionStart(payload)

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

// Exported for tests — the dispatcher runs only when invoked as the
// CLI entrypoint (Node's `require.main` check).
module.exports = {
	handlePermissionRequest, handlePreToolUse,
	handleSessionEnd, handleSessionStart,
	pruneWindow, uniquePatterns, denyReason,
	WINDOW_MS, SPIKE_THRESHOLD, RACE_WINDOW_MS
}

if (require.main === module) main()

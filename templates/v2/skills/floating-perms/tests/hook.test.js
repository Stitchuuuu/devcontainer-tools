// hook.test.js — exercise handlePermissionRequest + handlePreToolUse
// with full state isolation under a per-test sid. Time is mocked via
// Date.now overrides where deterministic ordering matters.

const fs   = require('node:fs')
const path = require('node:path')
const os   = require('node:os')

// IMPORTANT: env vars must be set BEFORE requiring hook.js — state.js
// captures the paths at module-load time.
const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fp-hook-test-'))
process.env.FP_STATE_PATH    = path.join(tmpDir, 'state.json')
process.env.FP_AUDIT_PATH    = path.join(tmpDir, 'audit.jsonl')
process.env.FP_SETTINGS_LOCAL = path.join(tmpDir, 'settings.local.json')

const { test, after } = require('node:test')
const assert = require('node:assert/strict')

const hook = require('../hook.js')

const STATE_PATH = process.env.FP_STATE_PATH

after(() => {
	try { fs.rmSync(tmpDir, { recursive: true, force: true }) } catch {}
})

let sidCounter = 0
function uniqueSid() { return `test-sid-${++sidCounter}` }

function readState() {
	try { return JSON.parse(fs.readFileSync(STATE_PATH, 'utf8')) }
	catch { return { version: 1, grants: [], counters: {}, warned: {} } }
}

function withFixedNow(ts, fn) {
	const real = Date.now
	Date.now = () => ts
	try { return fn() } finally { Date.now = real }
}

test('happy path: 2 × PermissionRequest then 1 × PreToolUse → deny', () => {
	// Threshold = 2 ⇒ the 3rd call that would prompt is intercepted by
	// PreToolUse before the dialog opens.
	const sid = uniqueSid()
	const base = 1_700_000_000_000

	withFixedNow(base + 10, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'curl https://example.com' },
			tool_use_id: 'a'
		})
	})
	withFixedNow(base + 20, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Edit',
			tool_input: { file_path: '/tmp/scratch/foo' },
			tool_use_id: 'b'
		})
	})

	const out = withFixedNow(base + 1000, () =>
		hook.handlePreToolUse({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'ls' }
		}))

	assert.ok(out, 'expected a deny response')
	assert.equal(out.hookSpecificOutput.permissionDecision, 'deny')
	const reason = out.hookSpecificOutput.permissionDecisionReason
	assert.match(reason, /Bash\(curl:\*\)/)
	assert.match(reason, /Edit\(\/tmp\/scratch\/\*\*\)/)
})

test('below threshold: 1 × PermissionRequest then 1 × PreToolUse → no deny', () => {
	const sid = uniqueSid()
	const base = 1_700_000_050_000

	withFixedNow(base + 10, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'curl https://example.com' },
			tool_use_id: 'a'
		})
	})

	const out = withFixedNow(base + 1000, () =>
		hook.handlePreToolUse({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'ls' }
		}))

	assert.equal(out, null,
		'1 PermissionRequest alone must not trip the deny')
})

test('no false positives: 100 × PreToolUse with 0 PermissionRequest → 0 denies', () => {
	const sid = uniqueSid()
	const base = 1_700_000_100_000

	// Inputs include every known false-positive family from the audit:
	const inputs = [
		// File writes in places that are already allowed via floating
		// sentinel (memory dir) — would have prompted in the predictor.
		{ tool_name: 'Write', tool_input: {
			file_path: '/home/node/.claude/projects/-workspace/memory/MEMORY.md' } },
		// Bash forms that never actually trigger a real prompt:
		{ tool_name: 'Bash', tool_input: { command: 'grep foo /dev/null' } },
		{ tool_name: 'Bash', tool_input: { command: 'if [ -f /tmp/x ]; then echo ok; fi' } },
		{ tool_name: 'Bash', tool_input: { command: 'TOK=$(jq .foo file)' } },
		// And a normal allowed cwd write:
		{ tool_name: 'Edit', tool_input: { file_path: '/workspace/src/foo.ts' } }
	]

	for (let i = 0; i < 100; i++) {
		const input = inputs[i % inputs.length]
		const out = withFixedNow(base + i, () =>
			hook.handlePreToolUse({ session_id: sid, ...input }))
		assert.equal(out, null,
			`PreToolUse at iter ${i} unexpectedly returned a deny: ${JSON.stringify(out)}`)
	}

	// Counter must remain empty — PreToolUse never grows it.
	const st = readState()
	assert.deepEqual(st.counters[sid] || [], [])
})

test('race window: 2 × PermissionRequest then 2 × PreToolUse same ms → exactly 1 deny', () => {
	const sid = uniqueSid()
	const base = 1_700_000_200_000

	for (let i = 0; i < 2; i++) {
		withFixedNow(base + i, () => {
			hook.handlePermissionRequest({
				session_id: sid, tool_name: 'Bash',
				tool_input: { command: `cmd-${i}` },
				tool_use_id: `r${i}`
			})
		})
	}

	// Both PreToolUse fire at the same millisecond — race-window guard
	// must let exactly one through.
	const out1 = withFixedNow(base + 100, () =>
		hook.handlePreToolUse({ session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'foo' } }))
	const out2 = withFixedNow(base + 100, () =>
		hook.handlePreToolUse({ session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'bar' } }))

	const denies = [out1, out2].filter(o => o && o.hookSpecificOutput &&
		o.hookSpecificOutput.permissionDecision === 'deny')
	assert.equal(denies.length, 1, 'expected exactly one deny in the race')
})

test('backward-compat: legacy entries without tool_use_id still counted', () => {
	const sid = uniqueSid()
	const base = 1_700_000_300_000

	// Seed the state file with one legacy entry (pre-tool_use_id schema).
	fs.writeFileSync(STATE_PATH, JSON.stringify({
		version: 1,
		grants: [],
		counters: { [sid]: [
			{ ts: base + 1, pattern: 'Bash(legacy:*)' }
		] },
		warned: {}
	}))

	// One fresh PermissionRequest on top of the legacy entry brings the
	// window to 2 entries — at threshold.
	withFixedNow(base + 10, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'curl https://x' },
			tool_use_id: 't1'
		})
	})

	const out = withFixedNow(base + 1000, () =>
		hook.handlePreToolUse({ session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'echo done' } }))

	assert.ok(out, 'expected deny with legacy entry contributing to count')
	const reason = out.hookSpecificOutput.permissionDecisionReason
	assert.match(reason, /Bash\(legacy:\*\)/, 'legacy pattern should appear in reason')
	assert.match(reason, /Bash\(curl:\*\)/)
})

test('handlePermissionRequest ignores meta tools (ExitPlanMode, etc.)', () => {
	const sid = uniqueSid()
	const base = 1_700_000_400_000

	for (const tn of ['ExitPlanMode', 'AskUserQuestion', 'TodoWrite', 'Task']) {
		withFixedNow(base, () => {
			hook.handlePermissionRequest({
				session_id: sid, tool_name: tn,
				tool_input: { foo: 'bar' },
				tool_use_id: `meta-${tn}`
			})
		})
	}

	const st = readState()
	assert.deepEqual(st.counters[sid] || [], [],
		'meta tools must not grow the counter')
})

test('handlePermissionRequest no-ops on missing fields', () => {
	const sid = uniqueSid()
	assert.equal(hook.handlePermissionRequest({}), null)
	assert.equal(hook.handlePermissionRequest({ session_id: sid }), null)
	assert.equal(hook.handlePermissionRequest({
		session_id: sid, tool_name: 'Bash' }), null)
	const st = readState()
	assert.deepEqual(st.counters[sid] || [], [])
})

test('pruneWindow drops entries older than WINDOW_MS', () => {
	const now = 1_000_000
	const cutoff = now - hook.WINDOW_MS
	const entries = [
		{ ts: cutoff - 100, pattern: 'old1' },
		{ ts: cutoff - 1,   pattern: 'old2' },
		{ ts: cutoff + 1,   pattern: 'kept1' },
		{ ts: now - 1,      pattern: 'kept2' }
	]
	const out = hook.pruneWindow(entries, now)
	assert.deepEqual(out.map(e => e.pattern), ['kept1', 'kept2'])
})

test('uniquePatterns preserves insertion order, dedups by pattern', () => {
	const out = hook.uniquePatterns([
		{ ts: 1, pattern: 'A' },
		{ ts: 2, pattern: 'B' },
		{ ts: 3, pattern: 'A' },
		{ ts: 4, pattern: 'C' }
	])
	assert.deepEqual(out, ['A', 'B', 'C'])
})

test('phantom guard: PermissionRequest ignored when pattern is already in allowlist', () => {
	// Seed the tmp settings.local.json with a matching allow entry.
	// The canonical form is Bash(head:*); Claude Code's alternate form
	// Bash(head *) must also count as covered.
	fs.writeFileSync(process.env.FP_SETTINGS_LOCAL, JSON.stringify({
		permissions: { allow: ['Bash(head *)', 'Bash(curl:*)'] }
	}))

	const sid = uniqueSid()
	const base = 1_700_000_500_000

	withFixedNow(base + 10, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'head -5 /etc/hosts' },
			tool_use_id: 'p1'
		})
	})
	withFixedNow(base + 20, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'curl https://example.com' },
			tool_use_id: 'p2'
		})
	})

	// Both PermissionRequests should have been filtered as covered
	// (head matches via the `Bash(head *)` alternate form, curl exact).
	const st = readState()
	assert.deepEqual(st.counters[sid] || [], [],
		'covered patterns must not grow the counter')

	// Reset settings so subsequent tests aren't polluted.
	fs.writeFileSync(process.env.FP_SETTINGS_LOCAL, JSON.stringify({
		permissions: { allow: [] }
	}))
})

test('phantom guard: uncovered pattern still counted', () => {
	fs.writeFileSync(process.env.FP_SETTINGS_LOCAL, JSON.stringify({
		permissions: { allow: ['Bash(cat:*)'] }
	}))

	const sid = uniqueSid()
	const base = 1_700_000_600_000

	withFixedNow(base + 10, () => {
		hook.handlePermissionRequest({
			session_id: sid, tool_name: 'Bash',
			tool_input: { command: 'rsync -av /a /b' },
			tool_use_id: 'q1'
		})
	})

	const st = readState()
	assert.equal((st.counters[sid] || []).length, 1,
		'uncovered pattern must grow the counter normally')
	assert.equal(st.counters[sid][0].pattern, 'Bash(rsync:*)')

	fs.writeFileSync(process.env.FP_SETTINGS_LOCAL, JSON.stringify({
		permissions: { allow: [] }
	}))
})

test('denyReason lists every unique pattern + the sid for apply.js batch', () => {
	const reason = hook.denyReason([
		{ ts: 1, pattern: 'Bash(curl:*)' },
		{ ts: 2, pattern: 'Edit(/tmp/scratch/**)' },
		{ ts: 3, pattern: 'Read(/home/node/**)' }
	], 'abc12345-deadbeef')

	assert.match(reason, /^STOP — floating-perms: 3 permission prompts/)
	assert.match(reason, /Bash\(curl:\*\)/)
	assert.match(reason, /Edit\(\/tmp\/scratch\/\*\*\)/)
	assert.match(reason, /Read\(\/home\/node\/\*\*\)/)
	assert.match(reason, /sid=abc12345-deadbeef/)
	assert.match(reason, /ANALYZE/)
	assert.match(reason, /ASK/)
	assert.match(reason, /EXECUTE/)
	assert.match(reason, /RETRY/)
})

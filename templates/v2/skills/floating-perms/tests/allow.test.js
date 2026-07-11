// allow.test.js — exercise `apply.js allow` via spawn to cover exit codes,
// stderr, single-use enforcement, session binding, and TTL expiry. Each
// test runs against an isolated state/audit/settings tmp dir (env-var
// overrides consumed by state.js at module load in the spawned process).

const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const { test, after } = require('node:test')
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')

const APPLY_JS = path.resolve(__dirname, '..', 'apply.js')
const PENDING_TTL_MS = 5 * 60 * 1000

const rootTmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fp-allow-test-'))

after(() => {
	try { fs.rmSync(rootTmp, { recursive: true, force: true }) } catch {}
})

// Per-test sandbox: fresh state.json, audit.jsonl, settings.local.json.
function makeSandbox(label) {
	const dir = fs.mkdtempSync(path.join(rootTmp, label + '-'))
	const env = {
		...process.env,
		FP_STATE_PATH: path.join(dir, 'state.json'),
		FP_AUDIT_PATH: path.join(dir, 'audit.jsonl'),
		FP_SETTINGS_LOCAL: path.join(dir, 'settings.local.json')
	}
	return { dir, env }
}

function writeState(dir, state) {
	fs.writeFileSync(path.join(dir, 'state.json'), JSON.stringify(state, null, 2))
}

function readState(dir) {
	try { return JSON.parse(fs.readFileSync(path.join(dir, 'state.json'), 'utf8')) }
	catch { return { version: 1, grants: [], counters: {}, warned: {}, pending_grants: {} } }
}

function readAudit(dir) {
	try {
		return fs.readFileSync(path.join(dir, 'audit.jsonl'), 'utf8')
			.split('\n').filter(Boolean).map(l => JSON.parse(l))
	} catch { return [] }
}

function runApply(env, args) {
	return spawnSync('node', [APPLY_JS, ...args], { env, encoding: 'utf8' })
}

function seedPending(dir, id, entry) {
	writeState(dir, {
		version: 1, grants: [], counters: {}, warned: {},
		pending_grants: { [id]: entry }
	})
}

test('allow: valid token consumes pending + writes grant', () => {
	const { dir, env } = makeSandbox('valid')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)', 'Bash(jq:*)'], created_at: now - 1000
	})

	const res = runApply(env, ['allow', 'id=abc12345', 'session=sess-x'])
	assert.equal(res.status, 0, `expected success, got ${res.status}: ${res.stderr}`)
	assert.match(res.stdout, /2 grant\(s\)/)

	const st = readState(dir)
	assert.equal(Object.keys(st.pending_grants).length, 0,
		'pending grant should be consumed')
	assert.equal(st.grants.length, 2)
	const patterns = st.grants.map(g => g.pattern).sort()
	assert.deepEqual(patterns, ['Bash(curl:*)', 'Bash(jq:*)'])
	for (const g of st.grants) {
		assert.equal(g.sid, 'sess-x')
	}

	const audit = readAudit(dir)
	assert.ok(audit.some(e => e.event === 'allow_consumed' && e.id === 'abc12345'))
	assert.ok(audit.some(e => e.event === 'grant'))
})

test('allow: same id used twice → 2nd refused (single-use)', () => {
	const { dir, env } = makeSandbox('double')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)'], created_at: now - 1000
	})

	const first = runApply(env, ['allow', 'id=abc12345', 'session=sess-x'])
	assert.equal(first.status, 0)

	const second = runApply(env, ['allow', 'id=abc12345', 'session=sess-x'])
	assert.equal(second.status, 2, 'second consumption should fail')
	assert.match(second.stderr, /unknown or already-consumed/)

	const audit = readAudit(dir)
	assert.ok(audit.some(e => e.event === 'allow_refused'
		&& e.reason === 'unknown_or_consumed'))
})

test('allow: unknown id → refused', () => {
	const { dir, env } = makeSandbox('unknown')
	writeState(dir, {
		version: 1, grants: [], counters: {}, warned: {}, pending_grants: {}
	})

	const res = runApply(env, ['allow', 'id=deadbeef', 'session=sess-x'])
	assert.equal(res.status, 2)
	assert.match(res.stderr, /unknown or already-consumed/)

	const st = readState(dir)
	assert.equal(st.grants.length, 0, 'no grant should be written on refusal')
})

test('allow: cross-session token → refused + pending preserved', () => {
	const { dir, env } = makeSandbox('mismatch')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)'], created_at: now - 1000
	})

	const res = runApply(env, ['allow', 'id=abc12345', 'session=sess-Y'])
	assert.equal(res.status, 2)
	assert.match(res.stderr, /different session/)

	// Session mismatch is not a consumption — the token stays for the
	// legitimate session to claim it.
	const st = readState(dir)
	assert.ok(st.pending_grants['abc12345'],
		'pending grant should survive a cross-session refusal')
	assert.equal(st.grants.length, 0)
})

test('allow: expired token → refused + purged', () => {
	const { dir, env } = makeSandbox('expired')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)'],
		created_at: now - PENDING_TTL_MS - 60_000
	})

	const res = runApply(env, ['allow', 'id=abc12345', 'session=sess-x'])
	assert.equal(res.status, 2)
	// The pre-sweep may have removed the entry before the lookup — either
	// error message is acceptable, both signal a stale-token refusal.
	assert.match(res.stderr, /unknown or already-consumed|expired/)

	const st = readState(dir)
	assert.equal(Object.keys(st.pending_grants).length, 0,
		'expired token must be purged from state')
	assert.equal(st.grants.length, 0)
})

test('allow: missing id arg → refused with usage hint', () => {
	const { env } = makeSandbox('missing-id')
	const res = runApply(env, ['allow', 'session=sess-x'])
	assert.equal(res.status, 2)
	assert.match(res.stderr, /missing id/)
})

test('allow: missing session arg → refused with usage hint', () => {
	const { env } = makeSandbox('missing-session')
	const res = runApply(env, ['allow', 'id=abc12345'])
	assert.equal(res.status, 2)
	assert.match(res.stderr, /missing session/)
})

test('allow: ttl override applied to final grant, not to token check', () => {
	const { dir, env } = makeSandbox('ttl-override')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)'], created_at: now - 1000
	})

	const res = runApply(env, ['allow', 'id=abc12345', 'session=sess-x', 'ttl=2h'])
	assert.equal(res.status, 0, res.stderr)

	const st = readState(dir)
	assert.equal(st.grants.length, 1)
	assert.equal(st.grants[0].ttl_seconds, 2 * 3600)
})

test('allow: invalid ttl → refused before token consumption', () => {
	const { dir, env } = makeSandbox('bad-ttl')
	const now = Date.now()
	seedPending(dir, 'abc12345', {
		sid: 'sess-x', patterns: ['Bash(curl:*)'], created_at: now - 1000
	})

	const res = runApply(env, ['allow', 'id=abc12345', 'session=sess-x', 'ttl=nope'])
	assert.equal(res.status, 2)
	assert.match(res.stderr, /invalid ttl/)

	// Bad ttl caught before withState — pending must NOT be consumed.
	const st = readState(dir)
	assert.ok(st.pending_grants['abc12345'],
		'pending grant should be preserved when ttl parse fails')
})

test('batch still works alongside allow (backwards compat)', () => {
	const { dir, env } = makeSandbox('batch-compat')
	writeState(dir, {
		version: 1, grants: [], counters: {}, warned: {}, pending_grants: {}
	})

	const res = runApply(env, ['batch', 'Bash(curl:*)', 'sid=sess-x'])
	assert.equal(res.status, 0, res.stderr)

	const st = readState(dir)
	assert.equal(st.grants.length, 1)
	assert.equal(st.grants[0].pattern, 'Bash(curl:*)')
})

// state.test.js — exercise withState + the pure sentinel/allow helpers.
// Filesystem touches are isolated to a tmp dir via the FP_* env vars.

const fs   = require('node:fs')
const path = require('node:path')
const os   = require('node:os')

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fp-state-test-'))
process.env.FP_STATE_PATH    = path.join(tmpDir, 'state.json')
process.env.FP_AUDIT_PATH    = path.join(tmpDir, 'audit.jsonl')
process.env.FP_SETTINGS_LOCAL = path.join(tmpDir, 'settings.local.json')

const { test, after } = require('node:test')
const assert = require('node:assert/strict')

const state = require('../lib/state')

after(() => {
	try { fs.rmSync(tmpDir, { recursive: true, force: true }) } catch {}
})

test('emptyState returns the expected schema', () => {
	const s = state.emptyState()
	assert.equal(s.version, 1)
	assert.deepEqual(s.grants, [])
	assert.deepEqual(s.counters, {})
	assert.deepEqual(s.warned, {})
	assert.deepEqual(s.pending_grants, {})
})

test('readStateRaw hydrates pending_grants on legacy state (missing key)', () => {
	// Legacy pre-single-use-token state file has no pending_grants field.
	// readStateRaw must add an empty object so callers don't hit undefined.
	fs.writeFileSync(state.STATE_PATH, JSON.stringify({
		version: 1, grants: [], counters: {}, warned: {}
	}))
	let seen = null
	state.withState((s) => { seen = s.pending_grants; return undefined })
	assert.deepEqual(seen, {})
})

test('withState writes back when mutator returns { state }', () => {
	state.withState((s) => {
		s.grants.push({ pattern: 'Bash(curl:*)', sid: 'x', granted_at: 1 })
		return { state: s }
	})
	const raw = JSON.parse(fs.readFileSync(state.STATE_PATH, 'utf8'))
	assert.equal(raw.grants.length, 1)
	assert.equal(raw.grants[0].pattern, 'Bash(curl:*)')
})

test('withState returns the mutator result via { state, result }', () => {
	const result = state.withState((s) => {
		s.counters['abc'] = [{ ts: 1, pattern: 'X' }]
		return { state: s, result: 'returned-value' }
	})
	assert.equal(result, 'returned-value')
})

test('withState round-trips legacy counter entries without tool_use_id', () => {
	// Seed the state file with a counter entry in the pre-v1.2 shape.
	const sid = 'legacy-sid'
	fs.writeFileSync(state.STATE_PATH, JSON.stringify({
		version: 1, grants: [], warned: {},
		counters: { [sid]: [{ ts: 42, pattern: 'Bash(legacy:*)' }] }
	}))

	let seen = null
	state.withState((s) => {
		seen = s.counters[sid]
		return undefined   // no write-back
	})
	assert.deepEqual(seen, [{ ts: 42, pattern: 'Bash(legacy:*)' }])
})

test('writeStateRaw is atomic (rename, not partial overwrite)', () => {
	// We can't easily inspect partial writes, but we can assert no .tmp
	// file is left behind after a normal mutation.
	state.withState((s) => {
		s.grants = []
		return { state: s }
	})
	const dir   = path.dirname(state.STATE_PATH)
	const base  = path.basename(state.STATE_PATH)
	const stale = fs.readdirSync(dir).filter(f => f.startsWith(base + '.tmp'))
	assert.deepEqual(stale, [])
})

test('findFloatingSection picks the sentinel-wrapped span', () => {
	const allow = [
		'Bash(node:*)',
		state.SENTINEL_START,
		'Bash(curl:*)',
		'Edit(/tmp/**)',
		state.SENTINEL_END,
		'Bash(git:*)'
	]
	const sec = state.findFloatingSection(allow)
	assert.ok(sec)
	assert.deepEqual(sec.patterns, ['Bash(curl:*)', 'Edit(/tmp/**)'])
})

test('findFloatingSection returns null when sentinels are missing or inverted', () => {
	assert.equal(state.findFloatingSection([]), null)
	assert.equal(state.findFloatingSection([state.SENTINEL_START]), null)
	assert.equal(state.findFloatingSection([state.SENTINEL_END]), null)
	assert.equal(state.findFloatingSection([
		state.SENTINEL_END, state.SENTINEL_START
	]), null)
})

test('mergeAllowSections partitions allow into human + floating sections', () => {
	// allow already contains the floating patterns inline; the function
	// strips any existing sentinels, identifies entries matching the
	// floatingPatterns set, and re-wraps them at the tail.
	const allow = [
		'Bash(node:*)',
		state.SENTINEL_START,
		'Bash(curl:*)',
		'Edit(/tmp/**)',
		state.SENTINEL_END,
		'Bash(git:*)'
	]
	const out = state.mergeAllowSections(allow, ['Bash(curl:*)', 'Edit(/tmp/**)'])
	assert.deepEqual(out, [
		'Bash(node:*)',
		'Bash(git:*)',
		state.SENTINEL_START,
		'Bash(curl:*)',
		'Edit(/tmp/**)',
		state.SENTINEL_END
	])
})

test('mergeAllowSections drops sentinels when no floating patterns remain', () => {
	const allow = [
		'Bash(node:*)',
		state.SENTINEL_START,
		state.SENTINEL_END,
		'Bash(git:*)'
	]
	const out = state.mergeAllowSections(allow, [])
	assert.deepEqual(out, ['Bash(node:*)', 'Bash(git:*)'])
})

// state.js — atomic R/W of the canonical state file under a sidecar lock.
//
// The canonical store lives at STATE_PATH. settings.local.json is just a
// mirror of the active grant patterns — never extended with a custom
// schema. Mutations always go through withState({lock: true}) so two
// concurrent PreToolUse hooks can't corrupt the counter or the grants.

const fs = require('fs')
const path = require('path')

const STATE_PATH = '/workspace/.devcontainer/notify/floating-perms-state.json'
const LOCK_PATH  = STATE_PATH + '.lock'
const AUDIT_PATH = '/workspace/.devcontainer/notify/floating-perms-audit.jsonl'

const LOCK_ATTEMPTS = 6
const LOCK_BASE_MS  = 20
const LOCK_JITTER   = 60
const LOCK_STALE_MS = 5000

function emptyState() {
	return { version: 1, grants: [], counters: {}, warned: {} }
}

function readStateRaw() {
	try {
		const buf = fs.readFileSync(STATE_PATH, 'utf8')
		const parsed = JSON.parse(buf)
		if (!parsed || typeof parsed !== 'object') return emptyState()
		parsed.grants   = Array.isArray(parsed.grants)   ? parsed.grants   : []
		parsed.counters = parsed.counters && typeof parsed.counters === 'object' ? parsed.counters : {}
		parsed.warned   = parsed.warned   && typeof parsed.warned   === 'object' ? parsed.warned   : {}
		return parsed
	} catch {
		return emptyState()
	}
}

function writeStateRaw(state) {
	const dir = path.dirname(STATE_PATH)
	fs.mkdirSync(dir, { recursive: true })
	const tmp = STATE_PATH + '.tmp.' + process.pid
	fs.writeFileSync(tmp, JSON.stringify(state, null, 2))
	fs.renameSync(tmp, STATE_PATH)
}

function sleep(ms) {
	const end = Date.now() + ms
	while (Date.now() < end) { /* spin — short waits only */ }
}

function acquireLock() {
	for (let i = 0; i < LOCK_ATTEMPTS; i++) {
		try {
			const fd = fs.openSync(LOCK_PATH, 'wx')
			fs.writeSync(fd, String(process.pid))
			fs.closeSync(fd)
			return true
		} catch (e) {
			if (e.code !== 'EEXIST') throw e
			try {
				const st = fs.statSync(LOCK_PATH)
				if (Date.now() - st.mtimeMs > LOCK_STALE_MS) {
					fs.unlinkSync(LOCK_PATH)
					continue
				}
			} catch { /* race ok */ }
			sleep(LOCK_BASE_MS + Math.floor(Math.random() * LOCK_JITTER))
		}
	}
	return false
}

function releaseLock() {
	try { fs.unlinkSync(LOCK_PATH) } catch { /* best effort */ }
}

// withState(mutator) — acquires lock, reads, calls mutator(state), writes
// back if mutator returned a truthy value (the new state object) or a
// `{state, result}` pair. Returns the mutator's `result` (or undefined).
function withState(mutator) {
	if (!acquireLock()) {
		const state = readStateRaw()
		return mutator(state, { locked: false })
	}
	try {
		const state = readStateRaw()
		const out = mutator(state, { locked: true })
		if (out && typeof out === 'object' && 'state' in out) {
			writeStateRaw(out.state)
			return out.result
		}
		if (out === true) writeStateRaw(state)
		return undefined
	} finally {
		releaseLock()
	}
}

// Append-only audit log. Best-effort — never throws.
function audit(event, fields) {
	try {
		const dir = path.dirname(AUDIT_PATH)
		fs.mkdirSync(dir, { recursive: true })
		const line = JSON.stringify({
			ts: new Date().toISOString(),
			event,
			...fields
		})
		fs.appendFileSync(AUDIT_PATH, line + '\n')
	} catch { /* swallow */ }
}

// settings.local.json operations — only `permissions.allow` is touched.
// Order: caller already holds the state lock before mutating allow.

const SETTINGS_LOCAL = '/workspace/.claude/settings.local.json'

function readAllow() {
	try {
		const buf = fs.readFileSync(SETTINGS_LOCAL, 'utf8')
		const parsed = JSON.parse(buf)
		const allow = parsed && parsed.permissions && Array.isArray(parsed.permissions.allow)
			? parsed.permissions.allow : []
		return { settings: parsed && typeof parsed === 'object' ? parsed : {}, allow }
	} catch {
		return { settings: {}, allow: [] }
	}
}

function writeAllow(settings, allow) {
	if (!settings || typeof settings !== 'object') settings = {}
	if (!settings.permissions || typeof settings.permissions !== 'object') {
		settings.permissions = {}
	}
	settings.permissions.allow = allow
	fs.mkdirSync(path.dirname(SETTINGS_LOCAL), { recursive: true })
	const tmp = SETTINGS_LOCAL + '.tmp.' + process.pid
	fs.writeFileSync(tmp, JSON.stringify(settings, null, 2) + '\n')
	fs.renameSync(tmp, SETTINGS_LOCAL)
}

module.exports = {
	STATE_PATH, AUDIT_PATH, SETTINGS_LOCAL,
	emptyState, withState, audit,
	readAllow, writeAllow
}

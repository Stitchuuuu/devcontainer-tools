// state.js — atomic R/W of the canonical state file under a sidecar lock.
//
// The canonical store lives at STATE_PATH. settings.local.json is just a
// mirror of the active grant patterns — never extended with a custom
// schema. Mutations always go through withState({lock: true}) so two
// concurrent PreToolUse hooks can't corrupt the counter or the grants.

const fs = require('fs')
const path = require('path')

// Paths overridable via env vars for test isolation; defaults match
// the devcontainer canonical layout.
const STATE_PATH = process.env.FP_STATE_PATH
	|| '/workspace/.devcontainer/notify/floating-perms-state.json'
const LOCK_PATH  = STATE_PATH + '.lock'
const AUDIT_PATH = process.env.FP_AUDIT_PATH
	|| '/workspace/.devcontainer/notify/floating-perms-audit.jsonl'

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

const SETTINGS_LOCAL = process.env.FP_SETTINGS_LOCAL
	|| '/workspace/.claude/settings.local.json'

// Sentinel marker entries that bracket the floating-perms managed section
// inside permissions.allow. They are strings that don't begin with any
// known tool prefix (Bash, Read, Edit, Write, NotebookEdit, WebFetch, …)
// so the Claude Code permission engine has nothing to match them against
// — effective no-op patterns. They survive JSON.stringify round-trips
// (they're array entries, not whitespace), giving the user a visible
// "this section is auto-managed" marker inside the file. Detection is
// strict equality on the constants below — do not vary the strings.
const SENTINEL_START = '// ──────── floating-perms managed below — auto-revoked on TTL expiry ────────'
const SENTINEL_END   = '// ──────── end floating-perms ────────'

// Legacy sentinel from the pre-default-TTL era. Kept in the accept-list so
// existing settings.local.json files continue to be recognized during the
// transition — writes will rebuild with SENTINEL_START above.
const SENTINEL_START_LEGACY = '// ──────── floating-perms managed below — auto-revoked at SessionEnd ────────'

function readAllow() {
	try {
		const buf = fs.readFileSync(SETTINGS_LOCAL, 'utf8')
		const parsed = JSON.parse(buf)
		const allow = parsed && parsed.permissions && Array.isArray(parsed.permissions.allow)
			? parsed.permissions.allow : []
		const additionalDirectories = parsed && parsed.permissions
			&& Array.isArray(parsed.permissions.additionalDirectories)
			? parsed.permissions.additionalDirectories : []
		return {
			settings: parsed && typeof parsed === 'object' ? parsed : {},
			allow, additionalDirectories
		}
	} catch {
		return { settings: {}, allow: [], additionalDirectories: [] }
	}
}

// Locate the sentinel-wrapped section inside an allow array. Returns
// { startIdx, endIdx, patterns } or null if both sentinels aren't found
// in order. Used by reconcile + SessionStart orphan detection.
function findFloatingSection(allow) {
	if (!Array.isArray(allow)) return null
	let startIdx = allow.indexOf(SENTINEL_START)
	if (startIdx < 0) startIdx = allow.indexOf(SENTINEL_START_LEGACY)
	const endIdx   = allow.indexOf(SENTINEL_END)
	if (startIdx < 0 || endIdx < 0 || endIdx <= startIdx) return null
	return {
		startIdx, endIdx,
		patterns: allow.slice(startIdx + 1, endIdx)
	}
}

// Partition allow into (human, floating) using the floatingPatterns set,
// strip any existing sentinels, and rebuild as [...human, START, ...floating, END].
// When floating is empty, sentinels disappear so the file stays clean.
function mergeAllowSections(allow, floatingPatterns) {
	const floatingSet = new Set(floatingPatterns || [])
	const cleaned = allow.filter(p =>
		p !== SENTINEL_START && p !== SENTINEL_END && p !== SENTINEL_START_LEGACY)
	const human    = cleaned.filter(p => !floatingSet.has(p))
	const floating = cleaned.filter(p =>  floatingSet.has(p))
	if (floating.length === 0) return human
	return [...human, SENTINEL_START, ...floating, SENTINEL_END]
}

// additionalDirectories has no sentinel scheme (it's a plain array of
// absolute paths — no entries the permission engine will simply ignore).
// The caller therefore passes the final desired union (user-authored dirs
// PLUS the currently-active floating dirs). If undefined, the existing
// value is preserved; if the resulting array is empty, the key is dropped
// so files that never had it don't grow it on us.
function writeAllow(settings, allow, floatingPatterns, additionalDirectories) {
	if (!settings || typeof settings !== 'object') settings = {}
	if (!settings.permissions || typeof settings.permissions !== 'object') {
		settings.permissions = {}
	}
	settings.permissions.allow = mergeAllowSections(allow, floatingPatterns || [])
	if (additionalDirectories !== undefined) {
		if (Array.isArray(additionalDirectories) && additionalDirectories.length > 0) {
			settings.permissions.additionalDirectories = additionalDirectories
		} else {
			delete settings.permissions.additionalDirectories
		}
	}
	fs.mkdirSync(path.dirname(SETTINGS_LOCAL), { recursive: true })
	const tmp = SETTINGS_LOCAL + '.tmp.' + process.pid
	fs.writeFileSync(tmp, JSON.stringify(settings, null, 2) + '\n')
	fs.renameSync(tmp, SETTINGS_LOCAL)
}

module.exports = {
	STATE_PATH, AUDIT_PATH, SETTINGS_LOCAL,
	SENTINEL_START, SENTINEL_END,
	emptyState, withState, audit,
	readAllow, writeAllow,
	findFloatingSection, mergeAllowSections
}

#!/usr/bin/env node
// floating-perms/apply.js — CLI driving /floating-perms.
//
// Subcommands:
//   batch <pat1> [pat2 ...] [ttl=<duration>] [sid=<sid>]
//        grant N patterns for the current session. ttl optional (default:
//        until SessionEnd). sid required.
//   list [sid=<sid>]
//        show active grants (default: all sessions).
//   revoke <pattern>
//        manual revoke of one pattern across all sessions.
//   gc sid=<current_sid>
//        revoke grants whose sid != provided current sid (orphans).
//
// Duration syntax for ttl: 15m, 30m, 2h, 1d. Bare integer = seconds.
//
// All mutations go through state.js withState() under lock. Audit lines
// land in /workspace/.devcontainer/notify/floating-perms-audit.jsonl.

const { canonicalize, isAllowed } = require('./lib/pattern')
const { isBlocked, reasonForPattern } = require('./lib/blocklist')
const { withState, readAllow, writeAllow, audit } = require('./lib/state')
const { revokeManual, revokeOrphans, revokeExpired } = require('./cleanup')

const DURATION_RE = /^(\d+)([smhd])?$/

function parseDuration(raw) {
	const m = String(raw).match(DURATION_RE)
	if (!m) return null
	const n = parseInt(m[1], 10)
	const unit = m[2] || 's'
	const mult = { s: 1, m: 60, h: 3600, d: 86400 }[unit]
	if (!mult) return null
	return n * mult
}

function parseArgs(argv) {
	const positional = []
	const opts = {}
	for (const a of argv) {
		const eq = a.indexOf('=')
		if (eq > 0) opts[a.slice(0, eq)] = a.slice(eq + 1)
		else positional.push(a)
	}
	return { positional, opts }
}

// Accept either a raw command (Claude piped tool_input.command) or a
// fully-formed canonical pattern. If the input already matches the
// canonical shape, pass through; otherwise try to canonicalize as Bash.
function normalizePattern(raw) {
	const s = String(raw).trim()
	if (/^Bash\(.+:\*\)$/.test(s)) return s
	if (/^(Edit|Write|Read|NotebookEdit)\(\/.+\/\*\*\)$/.test(s)) return s
	const asBash = canonicalize('Bash', { command: s })
	return asBash
}

function batch({ positional, opts }) {
	const ttlSeconds = opts.ttl ? parseDuration(opts.ttl) : null
	if (opts.ttl && ttlSeconds === null) {
		fail(`invalid ttl: "${opts.ttl}" (expected format: 15m, 2h, 1d, or seconds)`)
	}
	const sid = opts.sid
	if (!sid) {
		fail(`missing sid: pass sid=<session_id> (Claude must read it from the current hook payload)`)
	}
	if (positional.length === 0) {
		fail(`no pattern provided — usage: /floating-perms batch <pattern1> [pattern2 ...] [ttl=15m] sid=<id>`)
	}

	const proposed = positional.map(p => ({ raw: p, pattern: normalizePattern(p) }))
	const invalid = proposed.filter(p => !p.pattern)
	const blocked = proposed.filter(p => p.pattern && isBlocked(p.pattern))
	const ok      = proposed.filter(p => p.pattern && !isBlocked(p.pattern))

	revokeExpired()

	const { settings, allow } = readAllow()
	const allowSet = new Set(allow)

	const granted   = []
	const skipped   = []
	const now = Date.now()
	const expiresAt = ttlSeconds ? now + ttlSeconds * 1000 : null

	withState((state) => {
		for (const p of ok) {
			if (allowSet.has(p.pattern)) {
				skipped.push({ raw: p.raw, pattern: p.pattern, reason: 'already allowed' })
				continue
			}
			allowSet.add(p.pattern)
			state.grants.push({
				pattern: p.pattern, sid,
				granted_at: now, expires_at: expiresAt,
				ttl_seconds: ttlSeconds
			})
			granted.push({ raw: p.raw, pattern: p.pattern })
		}
		state.counters[sid] = []
		state.warned[sid] = 0
		return { state }
	})

	if (granted.length > 0) {
		writeAllow(settings, Array.from(allowSet))
		audit('grant', {
			sid, ttl_seconds: ttlSeconds, expires_at: expiresAt,
			patterns: granted.map(g => g.pattern)
		})
	}

	report({ granted, skipped, blocked, invalid, ttlSeconds, expiresAt, sid })
}

function list({ opts }) {
	revokeExpired()
	let grants = []
	withState((state) => { grants = state.grants.slice(); return undefined })
	const filter = opts.sid
		? grants.filter(g => g.sid === opts.sid)
		: grants
	if (filter.length === 0) {
		print(opts.sid ? `No active grants for sid=${opts.sid}.` : 'No active grants.')
		return
	}
	print(`Active grants (${filter.length}):`)
	for (const g of filter) {
		const expiry = g.expires_at
			? new Date(g.expires_at).toISOString()
			: 'until SessionEnd'
		print(`  - ${g.pattern}  [sid ${g.sid.slice(0, 8)} · expires: ${expiry}]`)
	}
}

function revoke({ positional }) {
	if (positional.length === 0) {
		fail('missing pattern — usage: /floating-perms revoke <pattern>')
	}
	const pattern = normalizePattern(positional[0])
	if (!pattern) fail(`unrecognized pattern: "${positional[0]}"`)
	const matched = revokeManual(pattern)
	if (matched.length === 0) {
		print(`No grants to revoke for ${pattern}.`)
		return
	}
	print(`Revoked ${matched.length} grant(s) for ${pattern}.`)
}

function gc({ opts }) {
	if (!opts.sid) {
		fail('gc requires sid=<current_session_id> to identify orphans')
	}
	const matched = revokeOrphans(opts.sid)
	if (matched.length === 0) {
		print('No orphans to revoke.')
		return
	}
	print(`Revoked ${matched.length} orphan grant(s):`)
	for (const g of matched) {
		print(`  - ${g.pattern}  [sid ${g.sid.slice(0, 8)}]`)
	}
}

function report({ granted, skipped, blocked, invalid, ttlSeconds, expiresAt, sid }) {
	if (granted.length > 0) {
		const expiry = expiresAt
			? `expires ${new Date(expiresAt).toISOString()} (TTL ${ttlSeconds}s)`
			: 'until SessionEnd'
		print(`✓ ${granted.length} pattern(s) granted [sid ${sid.slice(0, 8)} · ${expiry}]:`)
		for (const g of granted) print(`    ${g.pattern}`)
	}
	if (skipped.length > 0) {
		print(`↷ ${skipped.length} already allowed (no-op):`)
		for (const s of skipped) print(`    ${s.pattern}`)
	}
	if (blocked.length > 0) {
		print(`✗ ${blocked.length} refused by blocklist:`)
		for (const b of blocked) print(`    ${b.pattern} — ${reasonForPattern(b.pattern)}`)
	}
	if (invalid.length > 0) {
		print(`✗ ${invalid.length} unrecognized pattern(s):`)
		for (const i of invalid) print(`    "${i.raw}" — could not canonicalize`)
	}
	if (granted.length === 0 && skipped.length === 0 && blocked.length === 0 && invalid.length === 0) {
		print('Nothing to do.')
	}
}

function print(s) { process.stdout.write(s + '\n') }
function fail(msg) { process.stderr.write(`floating-perms: ${msg}\n`); process.exit(2) }

function main() {
	const sub = process.argv[2]
	const parsed = parseArgs(process.argv.slice(3))
	if (sub === 'batch')        batch(parsed)
	else if (sub === 'list')    list(parsed)
	else if (sub === 'revoke')  revoke(parsed)
	else if (sub === 'gc')      gc(parsed)
	else fail(`unknown subcommand: "${sub}" (expected: batch | list | revoke | gc)`)
}

main()

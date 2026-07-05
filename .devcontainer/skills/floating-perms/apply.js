#!/usr/bin/env node
// floating-perms/apply.js — CLI driving /floating-perms.
//
// Subcommands:
//   batch <pat1> [pat2 ...] [ttl=<duration>] [sid=<sid>]
//        grant N patterns for the current session. ttl optional (default:
//        30 minutes). sid required.
//   list [sid=<sid>]
//        show active grants (default: all sessions).
//   revoke <pattern>
//        manual revoke of one pattern across all sessions.
//   gc sid=<current_sid>
//        revoke grants whose sid != provided current sid (orphans).
//   reconcile sid=<current_sid> [--auto]
//        detect entries in settings.local.json that look floating but
//        aren't in state.json. Interactive by default; --auto revokes
//        without confirmation.
//
// Duration syntax for ttl: 15m, 30m, 2h, 1d. Bare integer = seconds.
//
// All mutations go through state.js withState() under lock. Audit lines
// land in /workspace/.devcontainer/notify/floating-perms-audit.jsonl.

const fs = require('fs')
const { canonicalize, extractDirFromFileToolPattern } = require('./lib/pattern')
const { isBlocked, reasonForPattern } = require('./lib/blocklist')
const {
	withState, readAllow, writeAllow, audit,
	findFloatingSection
} = require('./lib/state')
const { revokeManual, revokeOrphans, revokeExpired } = require('./cleanup')

// Claude Code refuses file writes/reads outside the cwd + the paths
// listed in permissions.additionalDirectories, regardless of what's in
// permissions.allow. So a file-tool grant like Write(/tmp/foo/**) is only
// effective if /tmp/foo is also present in additionalDirectories (or is
// a subpath of the cwd). We treat anything under CWD_PREFIX as already
// covered and skip injecting it.
const CWD_PREFIX = '/workspace'

function isUnderCwd(dir) {
	return dir === CWD_PREFIX || dir.startsWith(CWD_PREFIX + '/')
}

// Tracked baseline shipped with the devcontainer. Used by reconcile to
// avoid flagging canonical-form patterns that are part of the project's
// curated allowlist (e.g. Bash(node -v*) is fine even though it kinda
// matches the canonical shape).
const BASELINE_PATH = '/workspace/.devcontainer/claude/settings.local.json'

// Canonical-form detection for pre-V1.2 orphans: entries that look like
// they came from floating-perms (strict canonical shape) but live outside
// any sentinel section.
const CANONICAL_BASH_RE     = /^Bash\([^:]+:\*\)$/
const CANONICAL_FILE_TOOL_RE = /^(Edit|Write|Read|NotebookEdit)\(\/[^)]+\/\*\*\)$/

function looksLikeFloating(p) {
	return CANONICAL_BASH_RE.test(p) || CANONICAL_FILE_TOOL_RE.test(p)
}

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

// Default TTL when no ttl= flag is passed. Chosen because SessionEnd is
// not reliably fired when VS Code shuts down (the CLI runs in an integrated
// terminal, and the deactivate handler is a no-op) — grants without an
// explicit expiry became orphans across restarts. 30 min covers a typical
// focused task while capping the blast radius of an abandoned session.
const DEFAULT_TTL_SECONDS = 30 * 60

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
	const ttlSeconds = opts.ttl ? parseDuration(opts.ttl) : DEFAULT_TTL_SECONDS
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

	const { settings, allow, additionalDirectories } = readAllow()
	const allowSet = new Set(allow)
	const dirSet   = new Set(additionalDirectories)

	const granted   = []
	const skipped   = []
	// For file-tool patterns, the effective grant may require adding the
	// bucketed dir to additionalDirectories on top of the allow entry.
	// grantedDirs tracks the dirs we injected this call (for the report).
	const grantedDirs = []
	const now = Date.now()
	const expiresAt = now + ttlSeconds * 1000
	let floatingPatterns = []
	let finalAdditionalDirs

	withState((state) => {
		for (const p of ok) {
			const alreadyInAllow = allowSet.has(p.pattern)
			const dir = extractDirFromFileToolPattern(p.pattern)
			const needsDir = dir && !isUnderCwd(dir) && !dirSet.has(dir)

			if (alreadyInAllow && !needsDir) {
				skipped.push({ raw: p.raw, pattern: p.pattern, reason: 'already allowed' })
				continue
			}

			if (!alreadyInAllow) allowSet.add(p.pattern)
			if (needsDir) {
				dirSet.add(dir)
				grantedDirs.push(dir)
			}

			state.grants.push({
				pattern: p.pattern, sid,
				granted_at: now, expires_at: expiresAt,
				ttl_seconds: ttlSeconds,
				additional_dir: needsDir ? dir : null
			})
			granted.push({ raw: p.raw, pattern: p.pattern, dir: needsDir ? dir : null })
		}
		state.counters[sid] = []
		state.warned[sid] = 0
		floatingPatterns = state.grants.map(g => g.pattern)
		// Rebuild additionalDirectories: keep everything already there
		// (user entries + carried-over floating dirs) since we only add,
		// never remove here. Cleanup handles removal.
		finalAdditionalDirs = Array.from(dirSet)
		return { state }
	})

	if (granted.length > 0) {
		writeAllow(settings, Array.from(allowSet), floatingPatterns, finalAdditionalDirs)
		audit('grant', {
			sid, ttl_seconds: ttlSeconds, expires_at: expiresAt,
			patterns: granted.map(g => g.pattern),
			additional_dirs: grantedDirs
		})
	}

	report({ granted, skipped, blocked, invalid, ttlSeconds, expiresAt, sid, grantedDirs })
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
		// Legacy grants (pre-default-TTL era) may have expires_at: null.
		// Display them as "no expiry" for visibility; new grants always
		// carry a concrete expires_at.
		const expiry = g.expires_at
			? new Date(g.expires_at).toISOString()
			: 'no expiry (legacy)'
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

// Detect entries in permissions.allow that look like floating-perms grants
// but have no matching record in state.grants. Two sources:
//   (a) entries between the V1.2 sentinels (authoritative)
//   (b) canonical-form entries outside any sentinel, not present in the
//       tracked baseline (pre-V1.2 heuristic)
// Returns { inSection, preV12, orphans }.
function findOrphans({ allow, stateGrants, baseline }) {
	const section = findFloatingSection(allow)
	const inSection = section ? section.patterns : []
	const baselineSet = new Set(baseline)

	const preV12 = allow.filter(p => {
		if (p === '' || p.startsWith('//')) return false
		if (!looksLikeFloating(p)) return false
		if (inSection.includes(p)) return false
		if (baselineSet.has(p)) return false
		return true
	})

	const stateSet = new Set(stateGrants.map(g => g.pattern))
	const candidates = [...new Set([...inSection, ...preV12])]
	const orphans = candidates.filter(p => !stateSet.has(p))

	return { inSection, preV12, orphans }
}

function reconcile({ positional, opts }) {
	if (!opts.sid) {
		fail('reconcile requires sid=<current_session_id>')
	}
	const auto = positional.includes('--auto')

	revokeExpired()

	const { settings, allow } = readAllow()
	const baseline = readBaselineAllow()

	let stateGrants = []
	withState((state) => { stateGrants = state.grants.slice(); return undefined })

	const { inSection, preV12, orphans } = findOrphans({ allow, stateGrants, baseline })

	if (orphans.length === 0) {
		print('Nothing to reconcile — all floating-form entries in settings.local.json have a matching state.json grant.')
		return
	}

	if (!auto) {
		print(`Found ${orphans.length} orphan floating-form entry/entries in settings.local.json:`)
		for (const p of orphans) {
			const src = inSection.includes(p)
				? 'inside sentinels'
				: 'pre-V1.2 heuristic (canonical-form, not in baseline)'
			print(`  - ${p}  [${src}]`)
		}
		print('')
		print('To revoke them all, re-run with --auto:')
		print(`  /floating-perms reconcile sid=${opts.sid} --auto`)
		print('')
		print('If any of these are legitimate manual entries you want to keep, hand-edit settings.local.json before running --auto.')
		process.exit(1)
	}

	const orphanSet = new Set(orphans)
	const newAllow = allow.filter(p => !orphanSet.has(p))
	const remainingFloating = stateGrants.map(g => g.pattern)
	writeAllow(settings, newAllow, remainingFloating)

	audit('reconcile_auto', {
		sid: opts.sid,
		count: orphans.length,
		patterns: orphans
	})

	print(`✓ Revoked ${orphans.length} orphan(s) from settings.local.json:`)
	for (const p of orphans) print(`    ${p}`)
}

function report({ granted, skipped, blocked, invalid, ttlSeconds, expiresAt, sid, grantedDirs }) {
	if (granted.length > 0) {
		const expiry = `expires ${new Date(expiresAt).toISOString()} (TTL ${ttlSeconds}s)`
		print(`✓ ${granted.length} pattern(s) granted [sid ${sid.slice(0, 8)} · ${expiry}]:`)
		for (const g of granted) {
			const suffix = g.dir ? `  (+ additionalDirectories: ${g.dir})` : ''
			print(`    ${g.pattern}${suffix}`)
		}
	}
	if (grantedDirs && grantedDirs.length > 0) {
		print(`  additionalDirectories added (${grantedDirs.length}):`)
		for (const d of grantedDirs) print(`    ${d}`)
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
	if (sub === 'batch')          batch(parsed)
	else if (sub === 'list')      list(parsed)
	else if (sub === 'revoke')    revoke(parsed)
	else if (sub === 'gc')        gc(parsed)
	else if (sub === 'reconcile') reconcile(parsed)
	else fail(`unknown subcommand: "${sub}" (expected: batch | list | revoke | gc | reconcile)`)
}

main()

// =============================================================================
// notify-app — notif-cli-based desktop notification consumer (macOS, v0.2)
// =============================================================================
//
// Alternative to the "basic-notif" (osascript / WinRT / linux-stub) consumer.
// Dispatches through the standalone `notif` binary (apps/notifier/) so the
// daemon gets :
//   - sender identity control (each banner appears under a chosen `.app`)
//   - per-notif identifier + dismiss API via `notif remove`
//   - callbacks / Tier 3 macOS overrides available on the CLI surface
//
// Mutually exclusive with the basic-notif consumer — activate this one by
// listing `notify` in NOTIFY_CHANNELS (see index.js). If both are listed,
// index.js drops basic-notif with a warning ; they'd otherwise double-fire.
//
// Session 8 scope :
//   - macOS only. Windows/Linux fall through as `skipped` until sessions 9/10
//     wire their `notif` backends.
//   - Sender is always `default` (hook.js writes `line.sender = 'default'` on
//     every queue line). Per-event routing (claude vs npm-script vs …) is a
//     v0.3+ extension.
//   - Cancel-remove : subscribes to `cancelled:notification` and calls
//     `notif remove --sender X --id Y` when the banner had already been
//     dispatched. Post-fire state lives in the module-level `dispatched` Map
//     (auto-evicts after 10 min ; NC eventually rolls old banners off anyway).
// =============================================================================

const { spawn, spawnSync } = require('child_process')
const fs = require('fs')
const os = require('os')
const path = require('path')
const log = require('../log')
const { getHostKind } = require('../host')

// Hardcoded Claude Code sender identity. Every banner dispatched through
// this consumer appears in Notification Center under "Claude Code" with
// the bundled icon, regardless of what hook.js writes on `line.sender`.
// hook.js's `sender` field remains a placeholder for future per-event
// routing consumed by other channels — for the devcontainer daemon
// wiring Claude Code, the identity is always Claude Code.
const CLAUDE_CODE_SENDER = 'claude-code'
const CLAUDE_CODE_NAME   = 'Claude Code'
const CLAUDE_CODE_ICON   = path.join(__dirname, '..', '..', 'vendor', 'senders', 'claude-code.icns')

// Resolved absolute path to the `notif` binary. Set once at start() ; when
// null (no candidate exists on disk) the consumer reports `skipped` and
// leaves the bus alone so basic-notif can take over via NOTIFY_CHANNELS.
let notifBinPath = null

// Post-fire dispatched-notif tracking. Keyed by sid so `cancelled:notification`
// (which watcher.js emits with `{ id, sid, eventType, reason }`) can look up
// the exact banner without threading extra state through the bus. Value :
// `{ sender, notifId, timeout }` where `timeout` is the auto-evict handle.
const dispatched = new Map()
const DISPATCHED_TTL_MS = 10 * 60 * 1000

// One-shot flag — logs at most one warning when a queue line predates the
// session-8 hook.js (no `notif_id`). Prevents daemon.log flooding when an
// upgrade lands mid-session and older lines are still being replayed.
let notifIdFallbackLogged = false

// -----------------------------------------------------------------------------
// PUBLIC ENTRY POINT
// -----------------------------------------------------------------------------

/**
 * Wire the notify-app consumer onto the bus. Returns { status, diag } per
 * the index.js consumer contract.
 *
 * `skipped` cases :
 *   - host is not macOS (Windows/Linux backends land in sessions 9/10)
 *   - no `notif` binary found on any candidate path
 *
 * @param {object} opts
 * @param {import('events').EventEmitter} opts.bus   listens for send + cancel events
 * @returns {{ status: 'ok'|'skipped', diag: object }}
 */
function start({ bus }) {
	const host = getHostKind()
	if (host !== 'macos') {
		return { status: 'skipped', diag: { host, reason: 'notify-app-macos-only-v0.2' } }
	}
	notifBinPath = getNotifPath()
	if (!notifBinPath) {
		return { status: 'skipped', diag: { host, reason: 'notif-binary-not-found' } }
	}
	log.info(`[notify-app] notif binary resolved at ${notifBinPath}`)

	// Ensure the "Claude Code" sender bundle exists with the bundled icon
	// BEFORE the first send fires. `notif register` is idempotent — no-op
	// when the bundle is already materialized with the same identifier +
	// display name. Failure is non-fatal : we log a warning and continue ;
	// `notif send --sender claude-code` will auto-materialize on first
	// send anyway, just without the bundled icon.
	const registerDiag = registerClaudeCodeSender()

	bus.on('send:notification', send)
	bus.on('cancelled:notification', onCancelled)

	const diag = { host, notif: notifBinPath, sender: CLAUDE_CODE_SENDER, ...registerDiag }
	return { status: 'ok', diag }
}

/**
 * Bootstrap the "Claude Code" sender bundle at daemon boot. Runs
 * `notif register --sender claude-code --name "Claude Code" --icon <path>`
 * synchronously (with a bounded timeout) so subsequent sends land under
 * the right identity + icon.
 *
 * Idempotent — `notif register` short-circuits when the bundle already
 * exists at the same display name. Failure is non-fatal : the consumer
 * falls back to auto-materialization on first send, which uses the
 * bundled default icon instead of the Claude Code one.
 *
 * @returns {object}  diag fields to fold into start()'s return
 */
function registerClaudeCodeSender() {
	const args = ['register', '--sender', CLAUDE_CODE_SENDER, '--name', CLAUDE_CODE_NAME]
	if (fs.existsSync(CLAUDE_CODE_ICON)) {
		args.push('--icon', CLAUDE_CODE_ICON)
	} else {
		log.warn(`[notify-app] Claude Code icon missing at ${CLAUDE_CODE_ICON} — sender will use the default bell`)
	}
	try {
		const r = spawnSync(notifBinPath, args, {
			stdio:   'pipe',
			timeout: 5000,
			env:     { ...process.env, NOTIF_QUIET: '1' },
		})
		if (r.status === 0) {
			log.info(`[notify-app] Claude Code sender registered (icon=${fs.existsSync(CLAUDE_CODE_ICON) ? 'bundled' : 'default'})`)
			return { register: 'ok' }
		}
		const stderr = (r.stderr || '').toString().trim()
		log.warn(`[notify-app] notif register exited ${r.status}: ${stderr || '<no stderr>'} — falling back to auto-materialization on first send`)
		return { register: `failed-${r.status}` }
	} catch (e) {
		log.warn(`[notify-app] notif register threw: ${e.message} — falling back to auto-materialization on first send`)
		return { register: 'threw' }
	}
}

// -----------------------------------------------------------------------------
// BINARY RESOLUTION
// -----------------------------------------------------------------------------

/**
 * Resolve the absolute path to the `notif` CLI binary, in priority order :
 *
 *   1. `NOTIF_BIN` env — explicit override.
 *   2. `$XDG_DATA_HOME/notif/notif` — XDG-conformant install location.
 *   3. `~/.local/bin/notif` — common user-local bin dir.
 *   4. `~/bin/notif` — the location suggested by apps/notifier/docs/install-macos.md.
 *   5. `<daemon-root>/vendor/notif` — bundled fallback (daemon ships its own copy).
 *
 * Returns `null` if none exist. Callers report `skipped` and hand the bus
 * back to basic-notif.
 *
 * @returns {string|null} absolute path, or null
 */
function getNotifPath() {
	const candidates = [
		process.env.NOTIF_BIN,
		process.env.XDG_DATA_HOME
			? path.join(process.env.XDG_DATA_HOME, 'notif', 'notif')
			: null,
		path.join(os.homedir(), '.local', 'bin', 'notif'),
		path.join(os.homedir(), 'bin', 'notif'),
		path.join(__dirname, '..', '..', 'vendor', 'notif'),
	]
	for (const p of candidates) {
		if (p && fs.existsSync(p)) return p
	}
	return null
}

// -----------------------------------------------------------------------------
// DISPATCH
// -----------------------------------------------------------------------------

/**
 * Bus handler for `send:notification`. Extracts sender + notif_id from the
 * queue line (hook.js writes them in session 8+), invokes
 * `notif send --sender X --id Y --title T --body B [...]`, then records the
 * dispatched notif so `onCancelled` can dismiss it later.
 *
 * @param {object} payload   { sid, eventType, ts, line, id }
 * @returns {void}           fire-and-forget
 */
function send(payload) {
	const sid  = payload.sid || ''
	const sid8 = sid.slice(0, 8)
	const line = payload.line || {}

	const rawId = line.notif_id
	if (!rawId && !notifIdFallbackLogged) {
		log.warn('[notify-app] queue payload missing notif_id — hook.js pre-dates v0.2 ; using local fallback id (further occurrences squelched)')
		notifIdFallbackLogged = true
	}
	// This consumer is the devcontainer notify path for Claude Code — the
	// sender identity is always "Claude Code", overriding whatever hook.js
	// wrote on line.sender (that field stays a placeholder for other
	// consumers that may implement per-event routing).
	const sender  = CLAUDE_CODE_SENDER
	const notifId = rawId || `fallback-${sid8}-${Date.now()}`
	// Elevate the interruption level on macOS for events that need the
	// user's attention now. Anything else stays at Active (the default).
	const priority = (payload.eventType === 'permission_request' || payload.eventType === 'permission_prompt') ? 'high' : null

	// Reuse the basic-notif TEMPLATES via a lazy require to avoid a load-order
	// cycle at module init. `render()` returns { title, subtitle, body } with
	// the exact same shape basic-notif spawns.
	const { render } = require('./notifier')
	const rendered = render(payload)

	const args = [
		'send',
		'--sender', sender,
		'--id',     notifId,
		'--title',  rendered.title,
		'--body',   rendered.body,
	]
	if (rendered.subtitle) args.push('--subtitle', rendered.subtitle)
	if (priority)          args.push('--priority', priority)

	log.info(`[notify-app] DISPATCH ${payload.eventType} ${sid8} — id=${notifId} sender=${sender}`)
	spawnFireAndForget(notifBinPath, args)
	rememberDispatched(sid, sender, notifId)
}

/**
 * Bus handler for `cancelled:notification`. When a banner was dispatched for
 * this sid, spawn `notif remove --sender X --id Y` to dismiss it from
 * Notification Center. Pre-fire cancels (no dispatched entry) are silent —
 * watcher.js already cleared the timer, no banner ever appeared.
 *
 * @param {object} evt
 * @param {string} [evt.sid]        session id — provided by watcher.js since session-8
 * @param {string} [evt.eventType]  logged for diagnostics
 * @param {string} [evt.reason]     logged for diagnostics
 * @returns {void}                  fire-and-forget
 */
function onCancelled({ sid, eventType, reason } = {}) {
	if (!sid) return
	const entry = dispatched.get(sid)
	if (!entry) return
	dispatched.delete(sid)
	if (entry.timeout) clearTimeout(entry.timeout)
	const sid8 = sid.slice(0, 8)
	log.info(`[notify-app] cancel ${sid8} — removing notif id=${entry.notifId} (${eventType}/${reason})`)
	spawnFireAndForget(notifBinPath, [
		'remove',
		'--sender', entry.sender,
		'--id',     entry.notifId,
	])
}

/**
 * Store the (sender, notif_id) pair for a dispatched banner so onCancelled
 * can dismiss it. Auto-evicts after DISPATCHED_TTL_MS ; the previous entry
 * (if any) is cleared so its timer never leaks.
 *
 * @param {string} sid
 * @param {string} sender
 * @param {string} notifId
 * @returns {void}
 */
function rememberDispatched(sid, sender, notifId) {
	if (!sid || !notifId) return
	const prev = dispatched.get(sid)
	if (prev?.timeout) clearTimeout(prev.timeout)
	const timeout = setTimeout(() => dispatched.delete(sid), DISPATCHED_TTL_MS)
	timeout.unref?.()
	dispatched.set(sid, { sender, notifId, timeout })
}

// -----------------------------------------------------------------------------
// SPAWN HELPER
// -----------------------------------------------------------------------------

/**
 * Fire-and-forget spawn : stdio ignored, detached + unref'd so the child
 * outlives the daemon on shutdown ; NOTIF_QUIET=1 silences the CLI's
 * progress log (daemon isn't interactive). Errors are logged and swallowed
 * — a failed notif dispatch must never crash the daemon.
 *
 * @param {string} cmd     absolute path to the `notif` binary
 * @param {string[]} args  argv
 * @returns {void}
 */
function spawnFireAndForget(cmd, args) {
	try {
		const child = spawn(cmd, args, {
			detached: true,
			stdio:    'ignore',
			env:      { ...process.env, NOTIF_QUIET: '1' },
		})
		child.unref()
		child.on('error', (err) => log.warn(`[notify-app] ${cmd} failed: ${err.message}`))
	} catch (e) {
		log.warn(`[notify-app] spawn ${cmd} threw: ${e.message}`)
	}
}

module.exports = { start, getNotifPath }

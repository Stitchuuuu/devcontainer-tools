// =============================================================================
// constants — notify-policy values shared across the daemon
// =============================================================================
//
// Single import surface for the constants that customise notify's behaviour
// (templates, OS defaults, third-party limits, regexes, per-event delays).
// Generic primitives (LF byte, ms/h, log rotation thresholds, SIGTERM grace,
// docker probe timeout, .tmp suffix) deliberately STAY in their respective
// files — extracting them would hurt readability for values that won't change.
//
// Importers :
//   index.js                       → EVENT_DELAYS_MS
//   lib/consumers/notifier.js      → BRAND_NAME, TYPE_LABELS, WINDOWS_AUMID
//   lib/consumers/flash-win.js     → FLASH_EVENT_TYPES
//   lib/consumers/sound.js         → NATIVE_SOUND_DEFAULTS, LINUX_SOUND_CANDIDATES
//   lib/consumers/discord-webhook  → DISCORD_WEBHOOK_URL_RE, DISCORD_TRUNCATION_LIMITS
// =============================================================================

// -----------------------------------------------------------------------------
// TEMPLATES — strings + per-event maps that shape user-facing output.
// -----------------------------------------------------------------------------

/**
 * Brand baked into the OS notification title. Combined with the project name
 * by notifier.brandTitle() : `Claude Code · <project>`. Discord doesn't use
 * this — its bot already shows its display name on each message.
 */
exports.BRAND_NAME = 'Claude Code'

/**
 * Body line 1 = event-type-specific label. Each event has its OWN label,
 * surfacing what KIND of attention is needed. The label is NOT shown for
 * Stop — the recap message in line 2 is already self-explanatory.
 */
exports.TYPE_LABELS = {
	permission_request:  'Permission asked',
	permission_prompt:   'Permission prompt',
	elicitation_dialog:  'Question',
	idle_prompt:         'Idle',
	stop:                null   // sentinel: no label, recap message is the body
}

/**
 * Per-event-type delay (ms) applied by the watcher before firing the
 * notification. Tuned to absorb the "user cancels seconds later" path :
 * permission events wait 30 s so a follow-up Stop can "latest-wins"-replace
 * the timer (PostToolUse does NOT fire on user-Cancel). Idle is 0 — the
 * hook binary already waited 60 s before emitting the line.
 */
exports.EVENT_DELAYS_MS = {
	stop:                30_000, // 30 s — turn finished, wait for follow-up
	permission_request:  30_000, // 30 s — Cancel path emits Stop ~3-5 s later but PostToolUse
	                             //        does NOT fire on user-Cancel ; the Stop must arrive in
	                             //        time to "latest-wins"-replace this timer. 30 s gives
	                             //        plenty of headroom for slow tools + slow Stops.
	idle_prompt:              0, //   0 s — binary already waited 60 s before firing the hook
	permission_prompt:   30_000, // 30 s — Notification variant of permission_request, same logic
	elicitation_dialog:  30_000  // 30 s — Claude asked a question (dialog with options)
}

// -----------------------------------------------------------------------------
// DEFAULTS — platform-specific defaults + third-party limits.
// -----------------------------------------------------------------------------

/**
 * AUMID used as the toast's source identity on Windows. Hardcoded to
 * the standard VS Code installer's AUMID — gives the toast VS Code's
 * icon + activation. If you run VS Code Insiders or a non-standard
 * install, override this constant (Squirrel installs use a GUID,
 * Insiders is "Microsoft.VisualStudioCodeInsiders", etc.).
 */
exports.WINDOWS_AUMID = 'Microsoft.VisualStudioCode'

/**
 * Detached PowerShell processes that hand a fire-and-forget request to a
 * Windows broker (WinRT toast via ToastNotifier.Show(), system sound via
 * SystemSounds.Asterisk.Play()) must stay alive briefly after the call.
 * Without a trailing Start-Sleep, the detached PS exits in ~50 ms and the
 * OS broker drops the registration before it can render. 600 ms is the
 * empirical value used by both consumers :
 *   - notifier.sendWindows() interpolates this constant into the PS script
 *   - sound.js bakes it into NATIVE_SOUND_DEFAULTS.windows.args (literal,
 *     since the args array is declarative — keep the two values in sync if
 *     this constant ever changes).
 */
exports.WINDOWS_DETACHED_GRACE_MS = 600

/**
 * Event types that warrant a taskbar flash. Stop / tool_started /
 * tool_finished are deliberately excluded.
 */
exports.FLASH_EVENT_TYPES = new Set([
	'permission_request',
	'permission_prompt',
	'idle_prompt',
	'elicitation_dialog'
])

/**
 * Native defaults per host kind (see lib/host.getHostKind() — 'windows'
 * covers both native win32 Node and WSL Linux Node with interop). Built-in
 * OS notification sounds — nothing bundled in the repo. Linux is resolved
 * dynamically (distros ship different sound packages) ; see
 * sound.resolveLinuxDefault().
 * `resolved` is the human-readable descriptor surfaced to the status file —
 * a path on macOS, a system-sound name on Windows (no file is played).
 */
exports.NATIVE_SOUND_DEFAULTS = {
	macos: {
		cmd:      'afplay',
		args:     ['/System/Library/Sounds/Glass.aiff'],
		resolved: '/System/Library/Sounds/Glass.aiff'
	},
	windows: {
		cmd:      'powershell.exe',
		args:     ['-NoProfile', '-Command',
			'[System.Media.SystemSounds]::Asterisk.Play(); Start-Sleep -Milliseconds 600'],
		resolved: 'SystemSounds::Asterisk'
	}
}

/**
 * Discord enforces a 2000-char hard limit per message ; render() caps at 1900
 * to leave margin for any template-side prefix growth, and truncates to 1893
 * to reserve 7 chars for a closing "\n```" fence if truncation lands inside
 * a code block. `hard_limit` is informational — the actual cap is body_cap.
 */
exports.DISCORD_TRUNCATION_LIMITS = {
	body_cap:          1900,
	body_truncate_to:  1893,
	hard_limit:        2000
}

// -----------------------------------------------------------------------------
// PATHS — filesystem locations probed at runtime.
// -----------------------------------------------------------------------------

/**
 * Probe these in order on Linux ; first existing file wins. freedesktop
 * sounds ship with most GNOME/KDE installs ; ALSA sample is the universal
 * fallback.
 */
exports.LINUX_SOUND_CANDIDATES = [
	'/usr/share/sounds/freedesktop/stereo/message-new-instant.oga',
	'/usr/share/sounds/freedesktop/stereo/bell.oga',
	'/usr/share/sounds/alsa/Front_Center.wav'
]

// -----------------------------------------------------------------------------
// REGEXES — parsing patterns kept here for centralised review.
// -----------------------------------------------------------------------------

/**
 * Discord webhook URL shape : https://discord.com/api/webhooks/<channel_id>/<token>
 * Capture group 1 = prefix incl. trailing slash (channel ID is public),
 * capture group 2 = bot token (the actual secret). Used by redactWebhook()
 * to mask only the token portion in logs.
 */
exports.DISCORD_WEBHOOK_URL_RE = /^(https:\/\/discord\.com\/api\/webhooks\/\d+\/)(.+)$/

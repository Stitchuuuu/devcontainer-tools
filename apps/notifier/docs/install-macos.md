# Install `notif` on macOS

`notif` is the Rust CLI that dispatches notifications through
`UNUserNotificationCenter`. This guide covers a fresh install on macOS
11+ (Big Sur and later — `LSMinimumSystemVersion` set in the bundle
plist).

## 1. Obtain the binary

Until v0.5 (session N-2) publishes signed release artefacts, the binary
is cross-compiled from the Linux devcontainer.

From the devcontainer :

```bash
cd apps/notifier
cargo zigbuild --target aarch64-apple-darwin --release --bin notif
# → target/aarch64-apple-darwin/release/notif   (Apple Silicon)

# For Intel Macs :
cargo zigbuild --target x86_64-apple-darwin --release --bin notif
# → target/x86_64-apple-darwin/release/notif
```

The `target/` tree is bind-mounted to the Mac host under the workspace
root — no `scp` needed. Copy the binary to a stable location, e.g.
`~/bin/notif`, and put that directory on `PATH`.

### Building from source

The cross-compile relies on a vendored subset of the **macOS 14.5 SDK
stubs** at [../vendor/macos-sdk/](../vendor/macos-sdk/) (four
`.framework` `.tbd` files + `libobjc` variants) because zig 0.14's
TAPI-v4 parser SIGSEGVs on the Xcode 26.2 stubs
([zig #24615](https://github.com/ziglang/zig/issues/24615)). The
workaround is transparent — cargo picks the stubs up via
[../.cargo/config.toml](../.cargo/config.toml). Re-vendor with
[../vendor/fetch-sdk.sh](../vendor/fetch-sdk.sh) if a symbol added post
14.5 is ever needed.

**Expected build warning.** Every cross-compile prints :

```
warning: invoking "xcrun" "--sdk" "macosx" "--show-sdk-path" failed:
No such file or directory (os error 2)
  = note: the SDK is needed by the linker to know where to find symbols
          in system libraries and for embedding the SDK version in the
          final object file
```

This is **benign**. `rustc` tries to shell out to `xcrun` for the SDK
path (used to embed the SDK version metadata in the Mach-O), and since
we're inside a Linux devcontainer with no Xcode, the probe fails. The
linker falls back to the vendored stubs referenced via `-F` / `-L`
`rustflags`, produces a valid Mach-O, and the SDK-version field in the
binary just carries an unspecified value. Setting `SDKROOT` to point at
`vendor/macos-sdk/` was tried and rejected — zig then searches that
path for a complete SDK (libcharset / libiconv) which the minimal stub
tree doesn't ship.

## 2. First run — Gatekeeper bypass

The current build carries only an **ad-hoc signature**
(`codesign -s -`) applied at bundle materialization time — not a
Developer-ID cert. macOS attaches the `com.apple.quarantine` extended
attribute to any binary that arrived from outside the local
filesystem, and refuses to launch anything quarantined that isn't
Developer-ID signed + notarized.

Strip the quarantine bit once, per binary :

```bash
xattr -d com.apple.quarantine ~/bin/notif
```

Session N-2 will produce Developer-ID-signed + notarized binaries when
the rollout matures ; this manual step will disappear then.

## 3. First `notif send` — permission dialog

`notif` dispatches through `UNUserNotificationCenter`, which requires
per-bundle user consent (TCC — "Transparency, Consent, and Control").

Run a first send :

```bash
notif send --title "Hello" --body "from notif"
```

Under the hood :

1. `notif` materializes a `.app` bundle at
   `~/.local/share/notif/senders/Notify.app` (default Tier 0 sender).
2. Ad-hoc-signs it via `codesign --sign -` — mandatory on macOS 14+
   (UN center refuses unsigned bundles with an ambiguous
   `UNErrorDomain error 1`).
3. `open -W -a Notify.app --args setup` triggers the OS notification
   permission dialog. **Click Allow.** A denial leaves `notif` in a
   dropped-silently state — re-grant via
   *System Settings → Notifications → Notify → Allow Notifications*.
4. `open -W -a Notify.app --args send …` dispatches the actual banner.

Subsequent sends skip step 3 — permission is granted for the bundle's
`CFBundleIdentifier` (defaults to `com.notify.default`) until you
`tccutil reset` it.

## 4. Registering additional senders

Each additional sender key gets its own `.app` bundle + its own TCC
grant :

```bash
notif register --sender vscode --name "Visual Studio Code"
notif send --sender vscode --title "Task done" --body "See terminal"
```

The permission dialog fires the first time each new sender's bundle is
launched, then subsequent sends run silently. Use `notif senders` to
list every materialized bundle.

## 5. Change a sender's icon after registration

```bash
notif set-icon --sender vscode --icon ~/Downloads/vscode.icns
```

Writes `icon.icns` into the bundle's `Contents/Resources/`, updates the
Info.plist to declare `CFBundleIconFile`, and re-signs the bundle
ad-hoc so LaunchServices picks up the change. Idempotent — same bytes
means no-op. The reserved `default` sender is refused (its icon is
embedded in the binary at compile time — replace
[../crates/notif-macos/assets/notify.icns](../crates/notif-macos/assets/notify.icns)
and rebuild).

## 6. Housekeeping — `notif clean`

Registered bundles accumulate under `~/.local/share/notif/senders/`
and each carries an independent TCC grant. Remove one :

```bash
notif clean --sender vscode
```

Or wipe every bundle at once (prompts unless `--yes`) :

```bash
notif clean --all
```

Both variants run `tccutil reset Notifications <bundle-id>` before
deleting the folder, so a subsequent `notif register` for the same
key gets a fresh permission prompt instead of silently inheriting a
stale grant.

## 6.5 Tier 1 identity spoof — no local bundle

`notif send --identifier <bundle-id>` dispatches the banner **under
another app's identity** — the notification appears in Notification
Center with the target app's display name + icon, without
`notif` materializing its own `.app` bundle. Useful when the desired
identity is an app that is already installed and permission-granted.

```bash
notif send --title "Ready" --body "Build complete" --identifier com.microsoft.VSCode
notif send --title "Ping"  --body "Attach here"   --identifier com.googlecode.iterm2
```

**Gate — activates on `--identifier` alone.** Tier 1 fires only when
`--identifier` is set AND neither `--sender <key>` nor `--app <hint>` is
present. Either of those falls through to the local-bundle path
(§4 Registering additional senders), where `--identifier` is written
into the materialized bundle's Info.plist instead.

**How it works.** The CLI swizzles `-[NSBundle bundleIdentifier]` at
runtime (via `objc2::runtime::method_setImplementation`) so that when the
deprecated `NSUserNotificationCenter.deliverNotification:` API asks
`[NSBundle mainBundle].bundleIdentifier`, it receives the spoofed value.
LaunchServices then resolves the display name + icon from the target
app's installed `Info.plist`. The swizzle is process-scoped and not
restored — Tier 1 is fire-and-forget within a single short-lived CLI
invocation.

**macOS version gate — Tier 1 is Sonoma-and-older territory.** The
`NSUserNotification` API was deprecated in macOS 10.14 (Mojave) and
progressively neutered in later releases. Empirically, from macOS 15
(Sequoia) onward it **silently drops delivery** for bare CLI processes
that don't have a LaunchServices `.app` registration (which our `notif`
binary does not have when Tier 1 fires). The CLI therefore gates on the
host macOS version :

| macOS version | Tier 1 behavior |
|---|---|
| 15+ (Sequoia, Tahoe, …) | **Hard refused** with an actionable error : `Tier 1 (NSUserNotification) does not deliver on macOS <N> … Add --sender <key> to switch to Tier 2 …`. No warning, no attempt — the API is known-broken here. |
| 10.14 – 14 (Mojave through Sonoma) | Warned : `NSUserNotification was deprecated in macOS 10.14 and may silently drop delivery for non-bundled CLI processes. If no banner appears, add --sender <key> …`. Delivery attempted best-effort. Suppress the warn with `--quiet` (or `NOTIF_QUIET=1`) if you're comfortable that it works on your host. |
| < 10.14 (High Sierra and older) | No gate — the API was primary back then. Best-effort delivery. |

On modern hosts the practical outcome is: **use Tier 2** (add
`--sender <key>`). Tier 2 materializes a local `.app`, gets a proper
LaunchServices registration, and delivers reliably under the identifier
you pass via `--identifier` (written into the bundle's Info.plist).

**What Tier 1 does NOT support.** The `NSUserNotification` API predates
several modern features. Combining Tier 1 with any of the flags below
is refused up-front with an actionable error:

- `--image` — no attachment support in the NS API.
- `--priority` (non-Normal) — no interruption-level support.
- `--macos-attachment`, `--macos-interruption-level`,
  `--macos-thread-identifier`, `--macos-category-identifier`,
  `--macos-sound-name` — Tier 3 overrides target the UN center, not NS.
- `--on-click`, `--on-action`, `--on-dismiss`, `--on-timeout` — the NS
  delegate protocol (`NSUserNotificationCenterDelegate`) is distinct from
  the UN delegate and out of scope for v0.2.

For any of the above, switch to Tier 2 by adding `--sender <key>`. Tier 2
materializes a local bundle with the identifier written into its Info.plist,
and the UN center path supports all the flags above.

**Guards.** Tier 1 refuses two identifier classes before dispatch :

- `com.apple.*` — SIP-tier, macOS blocks impersonation of Apple-owned
  identifiers (`kLSNoLaunchPermissionErr`).
- Identifiers that do not resolve to an installed app via Spotlight —
  NC would silently render a blank name for a typo'd identifier.

## 7. Callbacks — react to click / action / dismiss

`notif send` accepts four callback flags — `--on-click`, `--on-action`,
`--on-dismiss`, `--on-timeout` — that hook into the user's response.
Each takes a **target string** with one of three prefixes:

| Prefix | Shape | Behavior |
|---|---|---|
| `hook:<argv>` | subprocess argv | Spawned with the JSON payload on `stdin`. Fire-and-forget. `argv` is tokenized per POSIX shell rules (`shell-words`). |
| `url:<https?://…>` | HTTP endpoint | JSON payload `POST`ed as `application/json`. 5 s connect / 10 s total. Non-2xx logged, not retried. |
| `file:<abs-path>` | file path | One JSONL line (`<json>\n`) appended per fire. `O_APPEND` keeps concurrent writers atomic. |

**Auto-detect** — prefix-less targets are classified by shape :
`http://…` / `https://…` → url, `/…` → file, else → hook. So a bare
`--on-click /tmp/clicks.jsonl` is equivalent to `--on-click file:/tmp/clicks.jsonl`.

### Payload shape

All three dispatchers receive the same JSON object :

```json
{
  "notif_id": "…",
  "event": "click | action:<label> | dismiss | timeout",
  "sender": "vscode",
  "title": "Deploy done",
  "body": "staging → prod",
  "ts": "2026-07-06T12:00:00Z"
}
```

Order and keys are stable — snapshot-tested inside `notif-core`.

### Worked examples

**`hook:` — trigger a local script on click.**

```bash
cat > /tmp/on-click.sh <<'EOF'
#!/bin/sh
# Reads the payload JSON on stdin ; log it.
jq '.title + " — clicked at " + .ts' >> /tmp/click.log
EOF
chmod +x /tmp/on-click.sh

notif send --title "Deploy done" --body "staging → prod" \
    --on-click hook:/tmp/on-click.sh
```

**`url:` — post a click to a webhook.**

```bash
notif send --title "Alert" --body "CPU 92%" \
    --on-click url:https://webhook.site/<uuid>
```

**`file:` — append every dismiss to a JSONL trail.**

```bash
notif send --title "Reminder" --body "coffee" \
    --on-dismiss file:/tmp/dismissed.jsonl
```

**`--on-action` — multiple buttons, each with its own target.**

```bash
notif send --title "PR review" --body "@you" \
    --on-action "reply:hook:/usr/local/bin/reply-to-pr" \
    --on-action "ignore:file:/tmp/ignored.jsonl"
```

Buttons appear on the banner in the order of `--on-action` flags.
The first flag is the primary action.

### How dispatch works — the `notif listen` daemon

When you pass **any** `--on-*` flag, `notif send` auto-spawns a
long-lived per-sender daemon (`notif listen --sender <key>`) that owns
the notification center's delegate. Without a delegate, macOS silently
drops every click / action / dismiss ; the daemon exists specifically
to catch those events and fire the matching target.

Runtime characteristics :

- **One daemon per sender.** `Notify` (Tier 0) gets one ; each Tier 2
  sender gets its own. `pgrep -f "notif listen"` shows them.
- **Auto-spawn.** First send-with-callback for a sender materializes
  the bundle, seeds LaunchServices, and `setsid`-detaches
  `notif listen`. Subsequent sends see the socket already up (≤ 5 ms
  round-trip).
- **Idle timeout.** Daemon self-exits after `--idle-timeout` (default
  `24h`) with no activity **and** an empty callback registry. Long
  enough that ordinary sessions never bounce a running daemon.
- **Clean shutdown.** `notif listen --exit --sender <key>` sends a
  shutdown request. Sends after that will auto-respawn.

Without callbacks, `notif send` skips the daemon entirely and uses the
existing direct-spawn inner path (~2 s exit) — the daemon costs
nothing when it's not needed.

### `--on-timeout` on macOS

macOS's notification center does **not** emit a "timed out" event —
banners fade off-screen after ~5 s but linger indefinitely in
Notification Center until the user clicks or dismisses them. So
`--on-timeout` on macOS is a **no-op** ; the daemon logs one
`info: --on-timeout is a no-op on macOS …` line per sender lifetime
and drops the binding. The flag stays in the CLI for portability with
future Windows / Linux backends that may fire a real timeout event.

### `--on-dismiss` — which gestures actually fire it

macOS's `UNNotificationDismissActionIdentifier` is sent to the delegate
only when the user **explicitly** dismisses a notification. Apple
draws a fine line here :

| Gesture | Fires `--on-dismiss` ? |
|---|---|
| Click the **X** on the banner in Notification Center | ✅ yes |
| Click the **X** / "Close" on an alert-style banner | ✅ yes |
| **Swipe** a live banner off-screen while it's still visible | ❌ often no — banner just moves to Notification Center |
| Use **Clear All** in Notification Center | ❌ no (bulk clear, no per-notif delegate call) |
| Banner **auto-fades** after ~5 s | ❌ no (no user action) |

If you swipe the banner too fast, macOS treats it as "moved to
Notification Center" rather than "user dismissed". The delegate never
fires. To reliably test `--on-dismiss`, let the banner fade, open
Notification Center (top-right clock / two-finger swipe from the right
edge), and click the little `X` that appears when hovering the
notification.

Session 7b's implementation opts into `.customDismissAction` on the
UN category whenever `--on-dismiss` is set — that's the maximum macOS
lets us wire. Beyond that it's OS policy.

## Troubleshooting

- **Notifications never appear.** Check
  *System Settings → Notifications* — the sender bundle must be
  present and Allow Notifications enabled. If the bundle isn't
  listed, run `notif setup --sender <key>` to fire the dialog again.
- **`open(1) exited 1` on the first send.** The bundle isn't
  ad-hoc-signed — likely a partial materialization from an earlier
  interrupted run. `notif clean --sender <key>` then retry.
- **"bundle not signed" on subsequent sends after a system upgrade.**
  macOS occasionally invalidates ad-hoc signatures across major
  version bumps. `notif clean --sender <key>` + a fresh `send`
  re-materializes and re-signs.
- **Callbacks never fire.** Verify the daemon is up with
  `pgrep -f "notif listen"`. If it's not, the send didn't include any
  `--on-*` flag (no daemon needed) or the auto-spawn hit an error —
  the outer prints `callback daemon start failed: …` on stderr.
  Retry with `NOTIF_LOG=/tmp/notif.log …` to capture the daemon's
  startup lines. To manually verify the delegate wiring, run
  `notif listen --sender <key>` in one terminal and
  `notif send --title X --body Y --on-click hook:/tmp/x.sh` in
  another ; click the banner and confirm `/tmp/x.sh` fires.
- **`callback daemon start failed: socket … did not accept`.**
  The daemon's inner-mode `notif listen` spawned but didn't `bind()`
  the socket within 3 s. Usually a permission issue on
  `~/.local/share/notif/senders/` — check ownership + mode.

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

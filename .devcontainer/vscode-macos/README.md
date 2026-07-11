# vscode-macos — host-side VS Code core patches

Small collection of Python patches that mutate the compiled JS inside an
installed VS Code app bundle on **macOS host** (not inside the
devcontainer — `/Applications/Visual Studio Code.app` doesn't exist in
the container).

## What it fixes

**Cross-Space fullscreen focus stealing for `vscode-remote://…` URLs.**

By default, `BaseWindow.focus()` in VS Code uses `FocusMode.Transfer`,
which is a passive internal focus and doesn't invoke
`[NSApp activateIgnoringOtherApps:YES]`. On macOS, that NSApp SELF-activate
is the only reliable way to switch cross-Space when the target VS Code
window lives in a different fullscreen Space than the caller.

The primary patch flips the default to `FocusMode.Force`, which routes
through `electron.app.focus({ steal: true })` → NSApp SELF-activate.

## Prerequisites

- macOS with Xcode Command Line Tools installed (`codesign` binary).
- Python 3 (any 3.x, ships with macOS or via Homebrew).
- `sudo` if `/Applications/Visual Studio Code.app` is root-owned
  (default install path).

## Usage — primary patch

```
sudo bash .devcontainer/vscode-macos/run-all.sh
# or invoke the script directly
sudo python3 .devcontainer/vscode-macos/focus-mode-force-default.py
# or override bundle path
sudo python3 .devcontainer/vscode-macos/focus-mode-force-default.py "/Applications/Cursor.app"
```

### Full-bundle snapshot (opt-in, ENV-gated)

Set `VSCODE_MACOS_SNAPSHOT=1` to copy the entire app bundle under
`~/.devcontainer-focus-backups/` before touching anything. Uses `ditto` so
xattrs, codesign metadata and symlinks are preserved. One-shot per
(bundle, version) — re-invocations skip the copy if a snapshot already
exists for the same VS Code version.

```
sudo env VSCODE_MACOS_SNAPSHOT=1 python3 .devcontainer/vscode-macos/focus-mode-force-default.py
# or via orchestrator
sudo env VSCODE_MACOS_SNAPSHOT=1 bash .devcontainer/vscode-macos/run-all.sh
```

The `env` before the command is mandatory when using `sudo` — otherwise
`sudo` strips the variable and the snapshot skips silently.

Snapshot-only (no patch) :

```
mkdir -p ~/.devcontainer-focus-backups
ditto "/Applications/Visual Studio Code.app" \
      ~/.devcontainer-focus-backups/"Visual Studio Code.app.$(defaults read \
        /Applications/Visual\ Studio\ Code.app/Contents/Info CFBundleShortVersionString)-$(date +%Y%m%d-%H%M%S)"
```

Auto-discovers, in order: VS Code, VS Code Insiders, Cursor, Windsurf.

Expected output (successful primary run) :

```
→ Auto-discovered app bundle: /Applications/Visual Studio Code.app
→ Target : .../Contents/Resources/app/out/main.js
→ Backup written : main.js.bak-devcontainer-focus
✓ Patched (pattern=minified-0-to-2) : ...
→ Re-signing /Applications/Visual Studio Code.app (ad-hoc)…
  ✓ signed
✓ Done. Restart VS Code to pick up the patch.
```

After patching : **Cmd+Q VS Code and relaunch** so the modified
`out/main.js` gets read into the fresh electron main process.

## Empirical validation

Test that the patch actually unblocks the cross-Space fullscreen switch :

1. Make sure at least one VS Code devcontainer window is on a fullscreen
   Space (Space A).
2. From another Space (e.g. Space B with Terminal.app or Finder), run :
   ```
   open "vscode-remote://dev-container+<hash>/workspace"
   ```
   (Grab a valid `dev-container+<hash>` URI from a recent VS Code window
   title, from logs, or by right-clicking a workspace in the recent list
   and copying the URL.)
3. **Expected** : macOS switches to Space A and the target window comes
   forward.
4. **If it doesn't switch Spaces**, run the fallback (below).

## Fallback — unconditional SELF-activate in `focus()`

Only run this if the primary patch alone doesn't unblock the cross-Space
switch (some code paths pass `FocusMode.Transfer` explicitly, bypassing
the flipped default) :

```
sudo python3 .devcontainer/vscode-macos/focus-unconditional-fallback.py
```

This prepends `B && vn.app.focus({steal:true})` at the very entry of
`BaseWindow.focus()`, so every call to focus() SELF-activates on macOS
regardless of the requested `FocusMode`. It stacks on top of the
primary patch (distinct marker, distinct backup).

Restart VS Code, retest.

## Rollback

### File-level (fastest, revert JS mutation only)

```
sudo cp "/Applications/Visual Studio Code.app/Contents/Resources/app/out/main.js.bak-devcontainer-focus" \
        "/Applications/Visual Studio Code.app/Contents/Resources/app/out/main.js"

# Also rollback the fallback if applied :
sudo cp "/Applications/Visual Studio Code.app/Contents/Resources/app/out/main.js.bak-devcontainer-focus-fb" \
        "/Applications/Visual Studio Code.app/Contents/Resources/app/out/main.js"

sudo codesign --sign - --deep --force \
    --preserve-metadata=entitlements,requirements \
    "/Applications/Visual Studio Code.app"

# Re-register URL scheme handlers (see LaunchServices note below).
sudo /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f \
    "/Applications/Visual Studio Code.app"
```

If macOS refuses to launch after rollback (`code signature invalid`), try
`sudo xattr -cr "/Applications/Visual Studio Code.app"` before the
codesign step.

### Full-bundle (from snapshot — cleanest, restores Microsoft signature)

If you took a snapshot with `VSCODE_MACOS_SNAPSHOT=1` :

```
sudo rm -rf "/Applications/Visual Studio Code.app"
sudo ditto ~/.devcontainer-focus-backups/"Visual Studio Code.app.<version>-<timestamp>" \
           "/Applications/Visual Studio Code.app"
sudo /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f \
    "/Applications/Visual Studio Code.app"
```

`ditto` preserves the original Microsoft codesign metadata, so TCC
permissions (keychain, Downloads, Full Disk Access…) re-attach to the
original identity — no new authorization prompts.

## LaunchServices reset — necessary side of ad-hoc re-signing

Ad-hoc re-signing (`codesign --sign -`) changes the code directory hash
of the bundle. macOS treats it as a "new" app for two subsystems :

1. **TCC** (Transparency, Consent, Control) — one-shot re-prompt storm for
   Keychain / Downloads / Full Disk / accessibility after first launch.
   Grant once, then silent. Ugly but harmless.
2. **LaunchServices** — the URL scheme registration for `vscode://` and
   the document type associations are keyed on the previous signing
   identity. After re-sign, they can go stale. Symptom :
   ```
   open "vscode://vscode-remote/..."
   → Error Domain=NSOSStatusErrorDomain Code=-10814
     "kLSApplicationNotFoundErr: no application claims the file"
   ```

Both `focus-mode-force-default.py` and `focus-unconditional-fallback.py`
now run `lsregister -f` at the end of the codesign step to avoid this.
If a run predates that fix (or ran in an environment where lsregister
was missing), apply the fix manually :

```
sudo /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f \
    "/Applications/Visual Studio Code.app"
```

## Persistence caveat — VS Code auto-update wipes the patch

Every VS Code update replaces `out/main.js` (and re-signs the bundle).
The patch is idempotent, so re-run manually after each update :

```
sudo bash .devcontainer/vscode-macos/run-all.sh
```

**TODO — auto re-patch on update.** A `launchd` watcher on
`/Applications/Visual Studio Code.app/Contents/Info.plist` mtime could
call `run-all.sh` automatically. Out of scope for now — filed as future
work.

## Compatibility

Tested against **VS Code 1.126.0** (Apple Silicon universal build). The
minified regex anchor (`focus(e){switch(e?.mode??0){`) is validated as
uniquely occurring in the bundled `out/main.js`. Older builds with the
per-module `out/vs/platform/window*/electron-main/*.js` layout are
covered by the fallback path in `find_target()` but not empirically
tested here.

If the primary script prints `REGEX MISS` after an update, inspect the
new bundle :

```
grep -bo "focus(e){switch(e?.mode" "/Applications/Visual Studio Code.app/Contents/Resources/app/out/main.js"
```

and update the pattern in `focus-mode-force-default.py` accordingly.

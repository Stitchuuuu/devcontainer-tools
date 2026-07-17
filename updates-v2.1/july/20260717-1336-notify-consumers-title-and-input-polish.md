# 20260717-1336 — notify consumers : title `projectName` + ExitPlanMode / AskUserQuestion polish

**Upstream commit:** `10c3e59` (2026-07-17 13:38 UTC)

**Affects** : v2.1 devcontainers running the notify pipeline with the
`notify` opt-in channel active (`NOTIFY_CHANNELS=notify,…`) — the
`notify-app.js` consumer that dispatches through the standalone
`notif` binary.

## What it changes

Two files, three orthogonal render tweaks :

1. **Title `projectName` seed for `notify-app`.** `notify-app` and
   `basic-notif` are mutually exclusive on the channel mux
   (`index.js`), so when `notify-app` is active, `notifier.start()`
   never runs — `projectName` in `notifier.js` stayed empty and
   `brandTitle()` collapsed from `Claude Code · <project>` to bare
   `Claude Code`. On the banner this looked as if `--title` was
   ignored because it duplicated the bundle `CFBundleName` header
   verbatim.

   `notifier.js` gains a `setProjectName(pn)` export ; `notify-app.js`
   destructures `projectName` from its `start()` opts (already passed
   by `index.js` at line 302) and seeds `notifier` before the first
   render.

2. **`ExitPlanMode` branch in `renderPermissionInput()`.** Was falling
   through to `JSON.stringify(input)` truncated at 150 — banners
   showed escaped JSON like
   `ExitPlanMode — {"allowedPrompts":[{"tool":"Bash","prompt":"run rz-ro …`.
   New branch matches the plan markdown's first `# <title>` line and
   surfaces it (fallback : first non-empty line of the plan).
   Shared render(), so `basic-notif` benefits too.

3. **Skip `--on-action Allow` for `AskUserQuestion`.** The Allow
   action returns a blanket ack ; for `AskUserQuestion` the user is
   meant to pick one of N options, so a single-button allow is
   ambiguous (which option got allowed ?). Body-click still focuses
   VS Code where the user can answer with the right choice.

## Apply

```sh
git apply 20260717-1336-notify-consumers-title-and-input-polish.patch
```

## Verification

```sh
grep -n 'setProjectName' .devcontainer/notify/lib/consumers/notifier.js .devcontainer/notify/lib/consumers/notify-app.js
# Expect 4 matches: function def + export in notifier.js, require+call in notify-app.js.

grep -n "ExitPlanMode" .devcontainer/notify/lib/consumers/notifier.js
# Expect 2 matches: docstring entry #3 + tool_name branch.

grep -n "AskUserQuestion" .devcontainer/notify/lib/consumers/notify-app.js
# Expect 2 matches: docstring rationale + tool_name !== gate.
```

Then restart the notify daemon on the host (kill the process or
rebuild the devcontainer) so the new code is loaded.

## Base / target blob hashes

- `.devcontainer/notify/lib/consumers/notifier.js` : `94bb056` → `dad6ce0`
- `.devcontainer/notify/lib/consumers/notify-app.js` : `d988bfa` → `13251c3`

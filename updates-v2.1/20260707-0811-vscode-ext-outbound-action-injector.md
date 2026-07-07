# 20260707-0811 — Reciprocal control channel for the Claude Code VS Code extension (outbound action injector)

**Affects** : v2.1 devcontainers where the Claude Code VS Code extension
already ships with `user-action-observer.py` (observation-only — webview→ext
messages logged to `inbound.jsonl`). This update adds the **reciprocal**
channel : an external script writes a JSONL line, the extension picks it
up and injects the action as if the user had clicked, and the UI dismisses
exactly as on a real click.

**Symptom / limitation being lifted** : until now Claude Code's permission
prompts required a **human click** per tool call. An orchestrator, batch
auto-allow script, or driven test can observe every webview→ext event
(via `user-action-observer` + `inbound.jsonl`) but has no way to
**inject** a response back — every prompt still blocks on manual input.

**Design** : the injection round-trips through the canonical webview
click path :

1. An external writer appends
   `{"cmd":"tool_permission_response","sessionId","requestId","behavior","updatedInput","updatedPermissions"}`
   to `.devcontainer/logs/claude-code-vscode-ext-outbound.jsonl`.
2. The extension's file-watcher (200 ms poll, singleton across all
   panels) reads the new line, looks up the target panel in
   `this.sessionPanels`, and posts a `{type:"simulated_click", requestId,
   result}` message to `panel.webview.postMessage()`.
3. The webview-side patch intercepts `simulated_click` in the message
   listener, finds the pending `Q = new Gn(requestId, ...)` in the
   reactive signal `this.permissionRequests.value` by `Q.channelId ===
   requestId` (Gn stores the requestId under `channelId`, a mildly
   misleading Anthropic naming choice), and calls `Q.accept(input,
   perms)` or `Q.reject(msg, false)` — which is **exactly** the method
   the real Allow / Deny button invokes.
4. `Q.accept(...)` triggers the pre-existing `onResolved` callback :
   sends the `tool_permission_response` back to the ext via the standard
   webview→ext channel (captured by `user-action-observer` as a normal
   `source:"user-action"` line in `inbound.jsonl` — indistinguishable
   from a real click) AND filters `Q` out of `permissionRequests.value`,
   which dismisses the prompt UI atomically.

The whole design deliberately avoids `comms.fromClient(fake_msg)`
short-circuits — resolving from ext-side without the webview seeing
the click would leave the prompt UI orphaned. Going through
`Q.accept/reject` reuses the atomic dismiss + reply operation.

**Observability** : the extension's `Comms.sendRequest` is instrumented
to append pending / settled markers to
`pending-perms.jsonl` whenever the message is a
`tool_permission_request`. An external controller reads that file to
know which `requestId` to target (previously invisible : the
ext→webview direction wasn't observed).

## Manual how-to

Six new files + two modified files. Both trees (`.devcontainer/` dogfood
and `templates/v2/` shipped template) get the same additions.

### File 1 — `.devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py`

New patch script targeting `extension.js`. Two injections :

- Singleton file-watcher at the top of `PanelManager.setupPanel(z,K,V,N){`
  (independent anchor from user-action-observer.py which targets the
  onDidReceiveMessage call inside the body — no collision).
- Pending log instrumentation in `Comms.sendRequest(sid, req, abort)` :
  writes `settled:false` at open time, `settled:true, outcome:"allow|deny"`
  at resolve time — only when `req.type === "tool_permission_request"`.

Regex uses `\w+\(\)` for the requestId generator (`Y3()` in 2.1.145 →
`Hs()` in 2.1.202 — the surrounding sendRequest signature is the true
anchor). Splices via `content[:idx] + INJECT + content[idx:]` (never
`re.sub` with the replacement string form — `\n` escapes get processed
and produce real newlines inside JS string literals, a known trap
documented in `user-action-observer.py`).

MARKERS : `notify-queue-outbound-inject-v1`,
`notify-queue-outbound-perm-log-v1`,
`notify-queue-outbound-perm-settle-v1`.

### File 2 — `.devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py`

New patch script targeting `webview/index.js`. One injection at the
`window.addEventListener("message",...)` site — intercepts
`simulated_click` before the normal `fromHost.enqueue` dispatch. The
event listener already has `this = zn` (which owns `permissionRequests`)
in scope, so no global exposure needed.

MARKER : `notify-queue-webview-sim-click-v1`.

### File 3 — `.devcontainer/claude/outbound-tester.js`

Node CLI, zero-dep. Two sub-commands :

- `list [--json]` : reads `pending-perms.jsonl`, aggregates by
  requestId, shows unsettled entries in a tabular view.
- `send <requestId> allow|deny [--input '<json>'] [--message '<text>'] [--sid <sessionId>]` :
  appends a line to `outbound.jsonl`. Auto-resolves `sessionId` from
  the pending log unless `--sid` is passed.

### File 4 — `.devcontainer/post-start.sh` (modified)

The existing wipe block for `inbound.jsonl` at container boot is
extended to also wipe `outbound.jsonl` and `pending-perms.jsonl` —
control-channel data is per-session, no retention.

### Files 5-7 — `templates/v2/claude/vscode-ext-patchs/outbound-action-injector.py`, `templates/v2/claude/vscode-ext-patchs/webview-simulated-click.py`, `templates/v2/claude/outbound-tester.js`

Byte-for-byte mirrors of files 1-3. The template tree keeps downstream
projects generated from `templates/v2/` in sync with the dogfood tree.

### File 8 — `templates/v2/post-start.sh` (modified)

Same wipe extension as file 4.

### Bonus — `vendor/anthropic.claude-code/` + `.gitignore` + memory

Not part of the runtime, but part of the workflow that produced this
update :

- `vendor/anthropic.claude-code/README.md` : documents the layout
  (`v<VERSION>/min/` = raw VSIX extract, `v<VERSION>/pretty/` = beautified
  copy for anchor selection) and the manual regeneration commands. The
  directory itself is `.gitignore`'d (200+ MB per version).
- `.gitignore` : `vendor/anthropic.claude-code/` added.
- Auto-memory entry `vendor-claude-code-ext-workflow.md` : records the
  workflow rule "any Claude Code VS Code ext patch goes through
  `vendor/anthropic.claude-code/v<VER>/{min,pretty}/` first".

### Commit

`````bash
git add \
  .devcontainer/claude/outbound-tester.js \
  .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py \
  .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py \
  .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py \
  .devcontainer/post-start.sh \
  templates/v2/claude/outbound-tester.js \
  templates/v2/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py \
  templates/v2/claude/vscode-ext-patchs/outbound-action-injector.py \
  templates/v2/claude/vscode-ext-patchs/webview-simulated-click.py \
  templates/v2/post-start.sh \
  updates-v2.1/20260707-0811-vscode-ext-outbound-action-injector.md \
  .gitignore

git commit -m "feat(vscode-ext): reciprocal outbound control channel + icon-fix hardening for 2.1.202+"
`````

### Apply the patch (existing devcontainer, no rebuild required)

`run-all.sh` picks the new scripts automatically at the next container
boot. To apply immediately without rebuilding :

`````bash
python3 .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py
python3 .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py
`````

Then reload the VS Code window (palette → `Developer: Reload Window`)
so both `extension.js` and `webview/index.js` re-load from disk.

## Verify

- [ ] `python3 .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py`
      → GREEN "injected at setupPanel top" + "instrumented sendRequest"
      first run ; YELLOW "already patched" thereafter, exit 0 both times.
- [ ] `python3 .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py`
      → GREEN "injected (event var: …)" first run ; YELLOW "already
      patched" thereafter.
- [ ] `node --check ~/.vscode-server/extensions/anthropic.claude-code-*/extension.js`
      → no output (valid JS).
- [ ] `node --check ~/.vscode-server/extensions/anthropic.claude-code-*/webview/index.js`
      → no output.
- [ ] After reload, trigger a Claude Code tool that needs permission
      (e.g. ask Claude to run `whoami`) :
      - `.devcontainer/logs/claude-code-vscode-ext-pending-perms.jsonl`
        gets a `settled:false` line with `requestId`.
      - `node .devcontainer/claude/outbound-tester.js list` shows the
        pending request.
- [ ] `node .devcontainer/claude/outbound-tester.js send <requestId> allow`
      → within 200 ms :
      - The Allow/Deny prompt disappears from the webview (real dismiss,
        not orphan).
      - Claude proceeds with `whoami`.
      - `inbound.jsonl` gets a `source:"user-action"` line with the
        tool_permission_response (indistinguishable from a real click).
      - `pending-perms.jsonl` gets a matching `settled:true,
        outcome:"allow"` line.
- [ ] `node .devcontainer/claude/outbound-tester.js send <requestId> deny`
      → same, `outcome:"deny"`, tool NOT executed.
- [ ] `bash .devcontainer/claude/vscode-ext-patchs/run-all.sh` → summary
      lists `outbound-action-injector.py` and `webview-simulated-click.py`
      as OK alongside existing scripts.
- [ ] `diff .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py
      templates/v2/claude/vscode-ext-patchs/outbound-action-injector.py`
      → no output. Same for the other 3 mirrored files.

## Rollback

`````bash
rm .devcontainer/claude/outbound-tester.js
rm .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py
rm .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py
rm templates/v2/claude/outbound-tester.js
rm templates/v2/claude/vscode-ext-patchs/outbound-action-injector.py
rm templates/v2/claude/vscode-ext-patchs/webview-simulated-click.py

# Revert the post-start.sh JSONL wipe extension
sed -i 's|rm -f /workspace/.devcontainer/logs/claude-code-vscode-ext-inbound.jsonl \\|rm -f /workspace/.devcontainer/logs/claude-code-vscode-ext-inbound.jsonl|' \
  .devcontainer/post-start.sh templates/v2/post-start.sh
# Then hand-remove the outbound.jsonl + pending-perms.jsonl lines that follow.

# Remove injection sites from the live extension (best-effort — a fresh
# VSIX re-extract is cleaner if you're going to rebuild anyway) :
LIVE=~/.vscode-server/extensions/anthropic.claude-code-*
node -e "
  const fs=require('fs'),path=require('path'),glob=require('glob');
  for (const dir of glob.sync(process.argv[1])) {
    for (const f of ['extension.js', 'webview/index.js']) {
      const p=path.join(dir,f);
      let s=fs.readFileSync(p,'utf8');
      // Simplest revert: remove everything between MARKER and next matching brace.
      // In practice, re-extract from vendor/ is safer.
      console.log('re-extract',dir,'from vendor/');
    }
  }
" "$LIVE"

git revert <commit-hash>
`````

## Bundled — icon-fix `_tg` rename + version-adaptive enum name

While validating this update against the latest marketplace release
(2.1.202), two pre-existing issues were found in
`icon-fix-open-in-current-panel.py` and fixed in the same commit.

**Fix 1 — `let g=` shadow collision.** Step [4/6] hard-coded a `let g=…`
injection inside the `primaryEditor.open` arrow function. On 2.1.202 the
minifier assigned `(g, b)` as arrow params → `async(g,b)=>{let g=…}` →
`SyntaxError: Identifier 'g' has already been declared`. Renamed to
`let _tg=…` (underscore-prefixed, guaranteed no collision with
one-letter-lowercase minified names).

**Fix 2 — version-adaptive enum name.** Verified against the raw VSIX
files : Anthropic has NOT added `"primary"` to the `preferredLocation`
enum in any released version (2.1.145 / 2.1.201 / 2.1.202 all ship with
`enum: ["sidebar", "panel"]`). The `primaryEditor.open` command IS
registered in all versions but is dead-code (only invoked by the
`vscode://anthropic.claude-code/open?…` deep-link handler; the
`getPreferredLocation()` getter collapses everything non-`"sidebar"` to
`"panel"`).

However, `primaryEditor.open` **exists as a command** in the ext bundle
which suggests Anthropic may plug the wiring in a future release. To
avoid a semantic conflict if that ever ships :

- **v2.1.201-** : patch keeps using `"primary"` (matches current
  `devcontainer.json` and `vscode-settings.jsonc` configs, no
  migration needed on the shipped devcontainer).
- **v2.1.202+** : patch uses `"active-panel"` — reserves `"primary"`
  for whatever Anthropic might do with it.

The choice is version-based (parsed from `package.json`'s `version`
field), not enum-inspection based (which gets contaminated by our own
previous patches once they've run).

**Migration** : when the base image bumps to 2.1.202+, the two config
files above must switch from `"primary"` to `"active-panel"`. That's a
one-line change in each file, tracked as a follow-up update recipe.

## Extensibility

The `outbound.jsonl` `cmd` field is deliberately extensible. Follow-up
updates can add `cmd:"askuser_response"` (for AskUserQuestion),
`cmd:"mode_change"` (for set_permission_mode), or `cmd:"prompt"` (drive
the input field) with a matching case in the ext-side watcher and, for
the last two, a matching webview-side intercept in the
`simulated_click` patch.

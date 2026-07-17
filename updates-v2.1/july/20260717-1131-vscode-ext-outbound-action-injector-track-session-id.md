# outbound-action-injector — track SDK sessionId + rename channelId (v1 → v2)

**Fix** : notify-app's Allow button was never emitted because
`pending-perms.jsonl` wrote the extension's short chatId under the
`sessionId` key, while the container-side hook (`notify-queue/hook.js`)
matches against Claude Code CLI's session UUID. The two ID spaces are
disjoint by construction, so `readPendingPermsTail(sid)`'s
`evt.sessionId === sid` filter never matched, leaving `line.tool_use_id`
unpopulated and the notify-app gate skipped the `--on-action Allow` arg.

Symptom, before this patch :
- `.devcontainer/logs/notif-actions.jsonl` stays 0 bytes despite
  many `permission_request` dispatches
- No queue jsonl carries a `"tool_use_id"` field
- macOS banner shows only the body (click-to-focus), no Allow button

## Why this recipe exists

The earlier `20260707-0811-vscode-ext-outbound-action-injector` recipe
shipped a v1 shape that misnamed `channelId` as `sessionId`. This delta
migrates the patcher to a v2 shape that :

1. Tracks the real SDK session UUID on `this._currentSessionId` via a
   new `patch_track_session_id` hook on `update_session_state`
   (same UUID PanelManager uses to key `sessionPanels`).
2. Writes records as
   `{sessionId: this._currentSessionId ?? null, channelId: <sid_var>, ...}`,
   restoring accurate field names.
3. Adds `strip_v1_injection()` — self-healing: strips pre-existing v1
   injections from `extension.js` before applying v2 blocks on rerun.
4. Emits a `{ev:"session_boot"}` marker in `pending-perms.jsonl` at
   watcher init (session-boundary detection for the outbound tester).
5. Captures `focused` / `active` from `vscode.window.state` in every
   record (was previously only written by the container-side hook).
6. Adds a `DEBUG=1` / `CLAUDE_OUTBOUND_DEBUG`-gated debug JSONL sibling
   for tailable watcher diagnostics.

## What it changes

`.devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py` —
one file, three logical additions :

- Rename markers `MARKER_PENDING` / `MARKER_SETTLE` from `-v1` to `-v2`,
  add `MARKER_TRACK_SID`, keep `MARKER_PENDING_V1` / `MARKER_SETTLE_V1`
  as strip anchors.
- New `strip_v1_injection(content, marker_v1)` helper — brace-counter
  walk to locate and remove `/*<marker>*/try{...}catch(_){};` blocks
  cleanly.
- New `patch_track_session_id(content)` — regex-anchored injection
  before `this.onSessionStateChanged` that stores
  `this._currentSessionId = <msg>.request.sessionId || this._currentSessionId`
  (comma-operator guard against transient empty sessionIds).
- Enhanced `WATCHER_BLOCK` — dbg helper, `pos=0` startup (reprocesses
  pre-existing lines), `sessionPanels.keys()` diagnostics in
  `lookup_miss`, `session_boot` marker append.
- `patch_sendrequest` — calls `strip_v1_injection` upfront, records
  now include `sessionId: this._currentSessionId ?? null`, `channelId`,
  `focused`, `active`.
- `main()` wires `patch_track_session_id` between `patch_watcher` and
  `patch_sendrequest`.

## Apply

```sh
git apply 20260717-1131-vscode-ext-outbound-action-injector-track-session-id.patch
```

Then re-run the patcher against the installed extension to migrate the
already-injected v1 blocks to v2 :

```sh
python3 .devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py
```

Expected output includes :
```
[outbound-perm-log] extension.js — stripped v1 injections (pending=1, settle=1)
[outbound-session-track] extension.js — tracking this._currentSessionId at update_session_state (msg=...)
[outbound-perm-log] extension.js — instrumented sendRequest (sid=..., rid=...)
```

Reload the VS Code window (Command Palette → *Developer: Reload
Window*) so the patched `extension.js` takes effect, then restart the
notify daemon.

## Verification

1. First line appended to `pending-perms.jsonl` after reload : has a
   UUID under `sessionId` and a short id under `channelId`. In tail :
   ```sh
   tail -1 /workspace/.devcontainer/logs/claude-code-vscode-ext-pending-perms.jsonl \
     | python3 -m json.tool
   ```
2. Next `permission_request` in a queue file carries `tool_use_id` :
   ```sh
   grep '"event":"permission_request"' \
     /workspace/.devcontainer/notify/queue/<sid>.jsonl | tail -1 \
     | grep -o '"tool_use_id":"[^"]*"'
   ```
3. The macOS banner for a permission_request shows an **Allow** button
   directly on the notif surface.
4. Clicking Allow appends a line to
   `/workspace/.devcontainer/logs/notif-actions.jsonl` (was 0 bytes
   forever before the fix) and produces a matching
   `tool_permission_response` entry in
   `claude-code-vscode-ext-outbound.jsonl`.

## Base / target blob hashes

- `.devcontainer/claude/vscode-ext-patchs/outbound-action-injector.py`
  : `859289c` → `79e010f`

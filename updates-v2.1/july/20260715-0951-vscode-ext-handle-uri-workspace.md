# 20260715-0951 — Route `claude-code://` URIs across workspaces

**Affects** : v2.1 devcontainers where clicking a `claude-code://` link
(from notifications, notify banners, `focus:open` payloads, etc.) never
routes to the intended workspace when it isn't the current focused
window.

**Symptom** : URIs targeting a workspace other than the one currently
focused are dropped or opened in the wrong window. Notify banner
click-to-focus / notif-daemon `focus:open` dispatches feel unreliable
across projects.

**Cause** : Claude Code's built-in `/open` handler ignores the source
workspace — VS Code's global `UriHandlerService` delivers the URI to
whatever window happens to be focused when the OS activates the URI
scheme.

**Resolution** : new patcher `handle-uri-workspace.py` extends the
`/open` handler to accept a `workspace=<absolute-path>` query param.
When the URI targets a workspace **other than** the current window, the
patch :

1. Calls `vscode.commands.executeCommand("vscode.openFolder", <target>)`
   to focus the target workspace.
2. Optionally `sleep`s a beat (covers macOS Space transitions where
   `openFolder` resolves before the WM focus swap completes).
3. Re-dispatches the URI without the `workspace` param, letting VS
   Code's global `UriHandlerService` deliver it to the now-focused
   window.

Idempotent via marker `notify-queue-uri-workspace-v1` so `run-all.sh`
re-runs stay clean.

**Upstream commit** : `6929dee` — `feat(vscode-ext-patchs): route claude-code:// URIs across workspaces`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260715-0951-vscode-ext-handle-uri-workspace.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260715-0951-vscode-ext-handle-uri-workspace.patch

git add .devcontainer/claude/vscode-ext-patchs/handle-uri-workspace.py
git commit -m "feat(vscode-ext-patchs): route claude-code:// URIs across workspaces"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] `test -f .devcontainer/claude/vscode-ext-patchs/handle-uri-workspace.py`
      → present.
- [ ] Open two projects side by side in VS Code. From project A, emit a
      URI targeting project B :
      `claude-code://open?session_id=<sid>&workspace=/absolute/path/to/B`
      → the window for B focuses and receives the URI.
- [ ] Optional : `grep -c "notify-queue-uri-workspace-v1" <extension-dir>/dist/extension.js`
      → 1 (idempotent marker injected).
- [ ] `run-all.sh` twice → second pass reports idempotent-skip on the
      workspace-routing patcher.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

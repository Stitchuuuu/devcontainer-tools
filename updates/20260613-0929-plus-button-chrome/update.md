# 20260613-0929 — Plus button chrome regression

**Affects** : v2.x devcontainers running `cc7cf11` or later with
`claudeCode.preferredLocation = "primary"`.

**Symptom** : clicking the `+` in the Claude webview header opens a
new tab without the session-tabs strip and right-side menu.

**Cause** : step [5/6] of `icon-fix-open-in-current-panel.py`
(`patch_plus_button_active_column`) passed `vscode.ViewColumn.Active`
literally as the 4th arg to `claude-vscode.editor.open`. Inside
`PanelManager.createPanel(z, K, V)` this sets
`Z = V === ViewColumn.Active → true`, and `setupPanel` then renders
the panel in "embedded mode" (no chrome, no right menu). Routes the
`+` handler through `claude-vscode.primaryEditor.open` when
`preferredLocation === "primary"`, mirroring step [3/6]'s branch on
`editor.openLast` so both entry-points (icon and `+`) share the same
code path. Also tightens the idempotence anchors of steps [4/6] and
[6/6] so `run-all.sh` re-runs stay clean after [5/6] introduces a
second occurrence of `"claude-vscode.primaryEditor.open"` and
`preferredLocation === "primary"` in `extension.js`.

**Upstream commit** : `54cd8b4` — `fix(vscode-ext-patchs): + button — route via primaryEditor.open in primary mode`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname)
populates `.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates/20260613-0929-plus-button-chrome/update.patch
git apply        .tmp/devcontainer-updates/updates/20260613-0929-plus-button-chrome/update.patch

git add .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py
git commit -m "fix(vscode-ext-patchs): + button chrome"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] Click `+` in a Claude webview header → new tab opens in the
      active column **with** session-tabs strip + right menu.
- [ ] Click the Claude icon (top-right) → unchanged behavior.
- [ ] Optional : flip `claudeCode.preferredLocation = "panel"`,
      reload window, click `+` → original split-column behavior with
      full chrome (fall-through branch preserved).

## Rollback

```bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
```

# 20260711-0701 — Create `disable-webview-auth-redirect.py` patcher (v1)

**Affects** : v2.1 devcontainers that don't yet have the
`disable-webview-auth-redirect.py` patcher installed.

**What it ships** : creates
`.devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py`
(246 lines). The patcher plumbs a new `claudeCode.disableWebviewAuthRedirect`
boolean into the Claude Code VS Code extension. When `true`, the chat
webview no longer redirects to the login screen on
`authentication_failed` errors — the error surfaces inline, panel
stays on chat view.

**Upstream commit** : `47b5365` — `feat(vscode-ext-patch): add claudeCode.disableWebviewAuthRedirect setting`

## Apply (patcher file only)

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260711-0701-vscode-ext-disable-webview-auth-redirect-create.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260711-0701-vscode-ext-disable-webview-auth-redirect-create.patch

git add .devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py
git commit -m "feat(vscode-ext-patchs): create disable-webview-auth-redirect.py patcher"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

## Enable the setting (manual — one-line edit)

The patcher only activates its runtime behavior when the setting is
declared as `true` in `.devcontainer/devcontainer.json`. Add it
alongside your existing `claudeCode.*` block. Example (add the last
line) :

```jsonc
"claudeCode.preferredLocation": "primary",
"claudeCode.disableWebviewAuthRedirect": true
```

The JSON edit is left manual — forks often customize the surrounding
`window.title` / `preferredLocation` lines and a `.patch` hunk would
fail on any drift.

## Verify

- [ ] `test -f .devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py`
- [ ] After adding the setting and reloading VS Code : trigger a 401
      (invalidate OAuth token) → chat panel stays on chat view with
      inline error, no redirect to login screen.

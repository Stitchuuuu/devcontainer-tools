# 20260712-0531 — Gate `isAuthenticated` getter (cover OAuth-expired redirect)

**Affects** : v2.1 devcontainers running
`updates-v2.1/20260707-0739-vscode-ext-disable-login-prompt` (the v1
`disable-webview-auth-redirect.py`).

**Symptom** : despite `claudeCode.disableLoginPrompt = true`, the
login screen still surfaces when `claude auth status --json` reports
no auth — e.g. after an OAuth session expires + refresh fails. The
chat panel silently swaps to the login component mid-session.

**Cause** : v1 gated the *synthetic-message → showLogin()* path only.
On OAuth expiry the extension pushes `update_state` with
`authStatus:null` ; the webview's `isAuthenticated` getter falls
through to the tail `return!1`, which flips the panel to the login
component. The v1 injection point sat too far upstream to catch this
route.

**Resolution** : add a bypass **inside the `isAuthenticated` getter**,
right before the tail `return!1` fallback :

- `forceLogin` (user-initiated `/login` command) short-circuits earlier
  in the getter → `/login` still opens the login screen normally.
- Legitimate auth (`authStatus.authenticated === true`) also
  short-circuits earlier → no change to the happy path.
- The new bypass only kicks in when **both signals are absent**, which
  is exactly the OAuth-expired case.

Anchor verified on v2.1.145 / v2.1.205 / v2.1.207 (helper minifier
names drift `l2` → `kn` → `On`, structure intact). Idempotent marker
so `run-all.sh` re-runs stay clean.

**Upstream commit** : `d8d2c37` — `feat(vscode-ext-patchs): gate isAuthenticated getter — cover OAuth expired redirect`

## Prerequisites

Apply `updates-v2.1/20260707-0739-vscode-ext-disable-login-prompt` first
— this patch is the delta on top of the v1 auth-redirect fix.

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260712-0531-vscode-ext-disable-webview-auth-redirect-oauth-gate.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260712-0531-vscode-ext-disable-webview-auth-redirect-oauth-gate.patch

git add .devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py
git commit -m "feat(vscode-ext-patchs): gate isAuthenticated getter (OAuth-expired redirect)"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] `grep -c "isAuthenticated" .devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py`
      → ≥ 2 hits (v1 gate + new getter bypass).
- [ ] Simulate OAuth expiry (e.g. `mv ~/.claude/oauth ~/.claude/oauth.bak && Cmd+Shift+P → Developer: Reload Window`)
      → chat panel keeps the current view, does **not** swap to login.
- [ ] `/login` from the chat input → login screen still opens
      normally.
- [ ] `run-all.sh` twice → second pass reports idempotent-skip on
      the disable-webview-auth-redirect patcher.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

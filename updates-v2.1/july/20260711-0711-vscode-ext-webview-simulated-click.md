# 20260711-0711 — Rewrite `webview-simulated-click` with reqId back-ref (v2.1.205+ support)

**Affects** : v2.1 devcontainers with the v1 `webview-simulated-click.py`
patcher already installed. Downstream extension version 2.1.145 → 2.1.207.

**Symptom** : outbound tool-permission responses (from Claude Code CLI
back to the webview) resolve the extension-side promise but never
dismiss the tool-permission prompt in the webview. The user still sees
the blocked prompt even though the tool has been auto-approved by a
`floating-perms` grant or a `notif`-daemon `Allow` click.

**Cause** : v1 intercept looked up pending permission-request
instances by `Q.channelId === requestId`. This never matched — `Q`
(the perm-request class instance) stores the **chat channelId** as
`this.channelId` (per-launch, ~10 chars), while the **wire requestId**
is a UUID living only in the `processRequestInner` closure. Two
different values, comparing them always failed.

**Resolution** : coordinated 3-point rewrite in `webview/index.js` :

1. **Callsite** (`processRequestInner`) — pass `$.requestId` as a 4th
   arg to `handleToolPermissionRequest`.
2. **Tag** (`handleToolPermissionRequest`) — stash `arguments[3]` on
   the new perm-request instance as `_reqId`, right after the
   constructor call.
3. **Intercept** (`window.addEventListener("message")`) — match
   pending instances by `q?._reqId === _rid` (instead of `channelId`).
   When found, call `.accept(...)` or `.reject(...)` — same code path
   a real click takes.

Constructor regex is widened to survive class-name drift : on v2.1.145
the class is `Gn` and sanitizer is `eV`; on v2.1.205+ both are
renamed (`LG` / `A1`). Anchor is the stable `.toolName / .inputs /
.suggestions` field triplet, with a backref forcing the same
sanitizer on all three — rules out unrelated `new Gn(...)` sites in
the token-model code.

Validated on v2.1.145, v2.1.205, v2.1.207 : 1 match per site, both
`node --check` pass, all three markers land as expected.

**Upstream commit** : `a410057` — `feat(vscode-ext-patch): rewrite sim-click with reqId back-ref, support v2.1.205+`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260711-0711-vscode-ext-webview-simulated-click.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260711-0711-vscode-ext-webview-simulated-click.patch

git add .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py
git commit -m "feat(vscode-ext-patchs): rewrite sim-click with reqId back-ref (v2.1.205+)"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] `grep -c "_reqId" .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py`
      → ≥ 3 hits (callsite + tag + intercept).
- [ ] Trigger a tool-permission prompt that a `floating-perms` grant
      auto-allows → the webview prompt dismisses (Simulated click)
      instead of staying blocked.
- [ ] Same test on both v2.1.145 and v2.1.205+ → same behavior on
      both versions.
- [ ] Optional : `run-all.sh` twice → second pass reports
      idempotent-skip on the sim-click patcher.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

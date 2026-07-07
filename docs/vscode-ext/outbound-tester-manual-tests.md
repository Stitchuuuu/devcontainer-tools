# Outbound control channel — manual test plan

Post-reload validation of the outbound action injector patch set
(`outbound-action-injector.py` + `webview-simulated-click.py` +
`outbound-tester.js`). Run these after `Developer: Reload Window` on
the Claude Code VS Code panel.

**Prerequisites**
- LIVE ext already patched (`bash .devcontainer/claude/vscode-ext-patchs/run-all.sh`)
- VS Code window reloaded so the patched `extension.js` and
  `webview/index.js` are actually loaded
- A terminal open in the workspace to run `outbound-tester.js` + tail
  the JSONL logs

## Baseline sanity — no session yet

```bash
node .devcontainer/claude/outbound-tester.js list
# expected: "(no pending perm requests)"
ls -l .devcontainer/logs/claude-code-vscode-ext-{outbound,pending-perms}.jsonl
# expected: both files exist (created by the ext watcher at startup),
# both empty (0 bytes) or absent-then-touched on first perm request
```

## T1 — Pending perm request appears in the observability log

1. In Claude, ask : "run `whoami`".
2. Wait for the permission prompt UI to appear.
3. In the terminal :
   ```bash
   tail -f .devcontainer/logs/claude-code-vscode-ext-pending-perms.jsonl
   ```
   Expected : a JSON line with `"settled":false`, `"toolName":"Bash"`,
   a valid `"requestId"` (short id).

4. Simultaneously :
   ```bash
   node .devcontainer/claude/outbound-tester.js list
   ```
   Expected : tabular row showing sid8, requestId, "Bash",
   inputs (starts with `{"command":"whoami"`), age.

## T2 — Inject allow via CLI, tool runs

Grab the `requestId` from T1. Then :

```bash
node .devcontainer/claude/outbound-tester.js send <requestId> allow
```

**Within 200 ms, expected** :

- ✅ The Allow / Deny prompt in the webview **disappears** (real
  dismiss, not orphan) — same visual as if a human had clicked Allow.
- ✅ Claude proceeds with `whoami` — you see its output land in the
  panel.
- ✅ New line in `.devcontainer/logs/claude-code-vscode-ext-inbound.jsonl`
  with `"source":"user-action"` (not `"user-action-simulated"` —
  because we go through the canonical webview→ext roundtrip, not a
  synthetic side-channel) and payload matching a normal
  `tool_permission_response`.
- ✅ New line in `pending-perms.jsonl` with `"settled":true,
  "outcome":"allow"` for the same requestId.
- ✅ « Claude VSCode » output channel logs
  `[outbound-inject] posted simulated_click sid=... rid=... behavior=allow`.

## T3 — Inject deny, tool does NOT run

Ask Claude to run another Bash command (e.g., `ls /tmp`). Wait for
prompt. Then :

```bash
node .devcontainer/claude/outbound-tester.js send <requestId> deny
```

Expected :
- ✅ Prompt dismisses in the webview.
- ✅ Claude gets the deny — the tool does NOT execute, Claude either
  stops or reroutes.
- ✅ `pending-perms.jsonl` gets `"settled":true, "outcome":"deny"`.

## T4 — Multi-session isolation (skip if only one Claude panel open)

1. Open a SECOND Claude session (icon → New Session or the `+` button
   in the panel header).
2. In session A, ask Claude to run a Bash command → prompt appears.
3. In session B, ask Claude to run a DIFFERENT Bash command → prompt
   appears.
4. `node .devcontainer/claude/outbound-tester.js list` → both should
   appear with different `sid8` values.
5. `send <requestId-A> allow` → **only session A's prompt dismisses**,
   session B is untouched.
6. `send <requestId-B> deny` → session B's prompt dismisses, session A
   already back to normal.

Confirms the sessionId lookup in `this.sessionPanels?.get(sid)` routes
to the right panel per session.

## T5 — Error handling : unknown requestId, bad sessionId

```bash
# Unknown requestId
node .devcontainer/claude/outbound-tester.js send nonexistent-id allow
# expected: exit 1 with error "no pending request with requestId=nonexistent-id"

# Force with a bogus sid — patch should log a warning but not crash
echo '{"cmd":"tool_permission_response","sessionId":"bogus","requestId":"x","behavior":"allow"}' \
  >> .devcontainer/logs/claude-code-vscode-ext-outbound.jsonl
# expected in « Claude VSCode » channel:
#   [outbound-inject] no panel for sid=bogus
```

The extension keeps running normally. No panels affected.

## T6 — Malformed JSONL line handling

```bash
echo 'not json at all' \
  >> .devcontainer/logs/claude-code-vscode-ext-outbound.jsonl
```

Expected : warning in « Claude VSCode » channel
`[outbound-inject] parse failed: Unexpected token ...`. Next valid
line still gets processed.

## T7 — Boot-time state — files touched on VS Code startup

1. Close VS Code entirely (Cmd+Q).
2. Reopen. Wait for the Claude panel to initialize.
3. Check the logs directory :
   ```bash
   ls -la .devcontainer/logs/claude-code-vscode-ext-*.jsonl
   ```
   Expected :
   - `inbound.jsonl`, `outbound.jsonl`, `pending-perms.jsonl` all
     present, wiped clean (0 bytes) by `post-start.sh`.

## T8 — Watcher robustness — file removed mid-session

```bash
# With Claude panel open and no perm request pending :
rm .devcontainer/logs/claude-code-vscode-ext-outbound.jsonl
# The watcher's stat() will fail silently on the next tick, retry on
# subsequent ticks. No crash, no ext restart needed.
# Then :
node .devcontainer/claude/outbound-tester.js send <newRequestId> allow
# The tester itself re-creates the file on append. Injection should
# work the next time a perm request comes up.
```

## Rollback check (if any test fails badly)

```bash
# Revert to unpatched extension.js + webview/index.js by re-extracting
# the VSIX. The Dockerfile is set up for this at every image rebuild.
docker compose --project-directory .devcontainer down
docker compose --project-directory .devcontainer build claude
docker compose --project-directory .devcontainer up
```

Or, if you want to test without a full rebuild : delete the marker
strings from the ext bundle and reload. But the cleanest reset is a
container rebuild — that's why we test on `vendor/anthropic.claude-code/v<VER>/min/`
extracts before touching LIVE.

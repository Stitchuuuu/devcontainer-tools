# Prompt 1 — Bash mix (allow + non-allow)

**Categories**: A1, A2, A3, E1, E2

**Goal**: verify the hook counts real popups correctly, skips
auto-allowed calls, and that grep on a path outside cwd may or may
not trigger a spike depending on Claude Code behavior.

## Prompt to paste (fresh chat)

```
Petite recon système, tool call séparé à chaque étape. Pas de batch,
pas de boucle, pas de subagent.

1. Bash : `ls -la /workspace/.devcontainer`
2. Bash : `git status`
3. Bash : `grep -c linux /etc/os-release`
4. Bash : `grep -c linux /etc/hostname`
5. Bash : `python3 -c "print('one')"`
6. Bash : `python3 -c "print('two')"`

Rapide, séquentiel. Note ce qui a demandé une autorisation et ce qui
est passé silencieusement.
```

## Expected

- Steps 1-2 (`/workspace/**`) → auto-allow silent, no `permission_seen`.
- Steps 3-4 (`grep` allow-listed, path `/etc`) → observe: silent or
  popup? Signal to look for in audit.
- Steps 5-6 (`python3` non-allow) → 2 popups → spike at 2nd or 3rd
  following tool call → floating-perms deny → workflow
  ANALYZE → ASK → EXECUTE → RETRY.
- After grant, `python3` retry passes without re-prompt.

## Verify

```bash
tail -40 .devcontainer/notify/floating-perms-audit.jsonl \
  | jq -c 'select(.event | test("permission_|spike|grant"))
           | {ts, event, pattern, sid: (.sid // .session_id | .[0:8])}'
```
- Zero `permission_seen` for steps 1-2 expected.
- For steps 3-4: either nothing (silent), or `permission_seen`
  (Claude Code prompts despite allow — important signal to document).
- 2× `permission_seen Bash(python3:*)` then `spike_detected` then
  `grant`.

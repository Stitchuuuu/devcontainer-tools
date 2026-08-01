# Prompt 2 — File tool outside cwd (central additionalDirectories test)

**Categories**: B1, B3, D1, D2, D3, G1

**Goal**: verify the full additionalDirectories flow: injection at
grant, disclosure in AskUserQuestion, silent retry, cleanup on
revoke.

## Prompt to paste (fresh chat)

```
Prépare un dossier /var/tmp/fp-test-$(date +%s)/ (retiens le chemin) et
écris-y 4 fichiers de configuration JSON courts :
- app.json (2 clés)
- db.json (2 clés)
- logging.json (2 clés)
- health.json (2 clés)

Ensuite lis app.json pour vérifier son contenu.

Un tool call à la fois, séquentiel. Pas de batch, pas de subagent.
```

## Expected

- 2 Write outside cwd → spike → workflow.
- **CRITICAL (D1)**: the agent's AskUserQuestion MUST mention
  `additionalDirectories: /var/tmp/fp-test-XXXX` in the description
  of the "Allow all" option. If absent → DISCLOSE doc not followed.
- **CRITICAL (D2)**: the agent must single-quote patterns in the
  `apply.js batch` (or `Skill batch`) call. Unquoted → risk of
  garbage patterns.
- **CRITICAL (D3)**: the `apply.js batch` output must contain the
  line `(+ additionalDirectories: /var/tmp/fp-test-XXXX)`. Absent →
  apply.js didn't extract the dir correctly.
- After grant, the remaining 2 Writes + the Read must pass without
  re-prompt (silent retry = fix confirmed).

## Verify (right after the grant)

```bash
cat /workspace/.claude/settings.local.json \
  | jq '.permissions | {allow, additionalDirectories}'
```
Expected:
- `/var/tmp/fp-test-XXXX` (absolute, no `/**`) in additionalDirectories.
- **NO new `Write(...)` entry in allow** — redundant, the dir gate
  already covers Read+Write.
- The apply.js batch output must contain `dir → /var/tmp/fp-test-XXXX
  (additionalDirectories only; allow entry skipped as redundant)`.

## Cleanup + G1 test (revoke removes dir)

```bash
node .devcontainer/skills/floating-perms/apply.js revoke \
  'Write(/var/tmp/fp-test-XXXX/**)'
cat /workspace/.claude/settings.local.json \
  | jq '.permissions.additionalDirectories'
```
Expected: no more `/var/tmp/fp-test-XXXX`. User-authored dirs preserved.

# Prompt 5 — Subpath of an existing additionalDirectories

**Categories**: B2

**Goal**: verify that a Write/Read in a sub-directory of a dir
already present in additionalDirectories passes silently (auto-allow
guard works for file tools on subpaths).

## Setup (before fresh chat) — manual grant of a test dir

```bash
mkdir -p /var/tmp/fp-parent-42/
node .devcontainer/skills/floating-perms/apply.js batch \
  'Write(/var/tmp/fp-parent-42/**)' \
  sid=$(cat ~/.claude/current-sid 2>/dev/null || echo test) ttl=15m
```

## Prompt to paste (fresh chat)

```
Petit test : crée le dossier /var/tmp/fp-parent-42/nested/inner/ puis
écris-y un fichier probe.txt avec le contenu "hello". Utilise
l'outil Write, un tool call à la fois.
```

## Expected

- No popup — the path is a sub-directory of a dir in
  additionalDirectories, so Claude Code auto-resolves.
- Audit: `permission_request_auto_allowed` or nothing at all for
  this Write.

## Verify

```bash
tail -15 .devcontainer/notify/floating-perms-audit.jsonl \
  | jq -c 'select(.event | test("permission_")) | {ts, event, pattern}'
```

## Cleanup

```bash
node .devcontainer/skills/floating-perms/apply.js revoke 'Write(/var/tmp/fp-parent-42/**)'
rm -rf /var/tmp/fp-parent-42/
```

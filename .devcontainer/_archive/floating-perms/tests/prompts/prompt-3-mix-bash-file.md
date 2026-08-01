# Prompt 3 — Mix Bash + file tool in the same batch

**Categories**: C1

**Goal**: verify that when a spike contains both Bash and file-tool
patterns, a single batch grants both correctly (allow for Bash,
allow+additionalDirectories for file tool).

## Prompt to paste (fresh chat)

```
Prépare un rapport système dans /var/tmp/fp-report-$(date +%s)/ :
1. Bash : `python3 --version` (capture output)
2. Prépare le dossier avec mkdir.
3. Utilise Write pour créer summary.txt avec la version dans un texte
   court (2 lignes max).
4. Utilise Bash pour ajouter la date (bash `date -u` capturé) à la
   fin du fichier via echo >>.

Un tool call à la fois, pas de batch shell, pas de boucle.
```

## Expected

- `python3` (non-allow) + Write outside cwd → mixed spike.
- Single batch granting `Bash(python3:*)` AND
  `Write(/var/tmp/fp-report-XXXX/**)`.
- AskUserQuestion mentions additionalDirectories only for the Write
  (not for python3).
- After grant: `Bash(python3:*)` in allow (no dir), `Write(...)` in
  allow with `/var/tmp/fp-report-XXXX` in additionalDirectories.

## Verify

Same as Prompt 2 + check `Bash(python3:*)` in allow.

## Cleanup

```bash
node .devcontainer/skills/floating-perms/apply.js revoke 'Bash(python3:*)'
node .devcontainer/skills/floating-perms/apply.js revoke 'Write(/var/tmp/fp-report-XXXX/**)'
rm -rf /var/tmp/fp-report-*
```

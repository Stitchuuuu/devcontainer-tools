# Session 1 — skeleton & wiring

> **Effort** : ~60-90 min | **Dependencies** : parent rollout's sessions 1+2 ✅ (need the converged session-2 process + final `CLAUDE-local-dev.md` to archive as `CLAUDE-local-mac-light-dev.md`)

## Prompt to paste

`````
Je démarre la session 1 du rollout `tune-claude-local-skill`.

Entry point : `/workspace/plans/tune-claude-local-skill/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are — 0/3 delivered, session 1 = skeleton)
- `LOG.md` (empty journal, session 1 will be the first append)
- `EXISTING.md` (technical inventory + architecture target)
- `sessions/session-1-skeleton.md` (this spec)
- Parent rollout context :
  - `../uniclaudeproxy-integration-local-opti/LOG.md` (sessions 1+2 — especially session 2's converged process)
  - `../uniclaudeproxy-integration-local-opti/sessions/session-3-tune-claude-local-skill.md` (the original spec that got extracted here — historical context)
- Relevant existing code (DO NOT MODIFY in this session except where noted) :
  - `.devcontainer/host-helpers/claude-switch` (MODIFY in this session — add CLAUDE_LOCAL_PROFILE env support)
  - `.devcontainer/host-helpers/mitm-capture` (reference for host-helper conventions)
  - `.devcontainer/skills/watch-log/watch-log.skill.md` (reference for skill .md frontmatter)
  - `.devcontainer/firewall/domains.d/` (reference for ecosystem allowlist convention)
  - `.devcontainer/claude/CLAUDE-local-dev.md` (read-only — will be cp'd to mac-light archive)
  - `.devcontainer/docker-compose.yml` (verify/add bind-mount for .devcontainer/tmp/)

Goal : pose la fondation du skill `tune-claude-local` sans encore
implémenter sa logique métier interactive. À la fin de cette session,
le CLI répond à `--help` avec un synopsis complet, `claude-switch
local-proxy` accepte le toggle via `CLAUDE_LOCAL_PROFILE`, le firewall
laisse passer ollama.com en GET, et le profil session-2 est archivé
sous son nom définitif. Sessions 2 (discovery) et 3 (tuning loop) impl
le reste.

Concrete deliverables :

### 1. CLI skeleton `.devcontainer/host-helpers/tune-claude-local` (NEW)

Bash script ~150-250 lignes max pour cette session. Pattern aligné avec
les autres host-helpers (`#!/usr/bin/env bash`, `set -euo pipefail`,
header doc, host-only guard si pertinent).

Structure :
- Header standard + `usage()` function dumpant le synopsis COMPLET des
  9 sous-commandes (même celles non encore implémentées) avec exemples
- Dispatcher sur `$1` vers `cmd_<name>`
- `cmd_status <profile>` : implémenté complètement — lit
  `.devcontainer/tmp/tune-claude-local/<profile>/state.json` si existe,
  dump human-readable avec phase courante + next step suggéré ; si pas
  encore initialisé, affiche "profile <X> not yet started — run
  `tune-claude-local ask <X>` to begin"
- `cmd_resume <profile>` : alias de `cmd_status` + ligne explicite "Next
  command : tune-claude-local <next-step> <profile>"
- `cmd_ask` / `cmd_research` / `cmd_propose` / `cmd_verify` /
  `cmd_baseline` / `cmd_apply` / `cmd_measure` / `cmd_finalize` :
  stubs qui :
  - Valident l'argument `<profile>` (kebab-case `[a-z][a-z0-9-]*`)
  - Appellent `init_state_dir <profile>` si pas encore créé (utile
    pour status/resume après n'importe quel step)
  - Affichent `[TODO session 2|3] subcommand <name> not yet implemented
    — see plans/tune-claude-local-skill/sessions/session-<2|3>-*.md`
  - Exit code 99 (distinct de 0 succès et de 1+ erreurs réelles)
- Helper interne `init_state_dir <profile>` : crée
  `.devcontainer/tmp/tune-claude-local/<profile>/` + un `state.json`
  minimal `{"profile":"<X>","current_step":"init","created_at":"<ISO>"}`
  via `jq -n` ou `printf`
- `--help` ou pas d'arg → usage(), exit 0
- Validation entrée : profile invalide → exit 2, message clair

Validation impl :
- `bash .devcontainer/host-helpers/tune-claude-local --help` → synopsis
  complet 9 sous-commandes + exemples
- `bash .devcontainer/host-helpers/tune-claude-local status demo` →
  init state dir + dump JSON minimal
- `bash .devcontainer/host-helpers/tune-claude-local ask demo` →
  message TODO session 2, exit 99
- `bash .devcontainer/host-helpers/tune-claude-local status demo` (re-run) →
  dump le state.json existant, ne re-init pas

### 2. `scenarios.json` statique (NEW)

`.devcontainer/host-helpers/tune-claude-local-internals/scenarios.json`
(le dossier internals est créé en session 1 mais ne contient que ce
JSON pour l'instant — les .sh suivront en sessions 2+3).

Schéma final (statique, consommé en session 3 par `replay-scenarios.sh`) :

```json
{
  "scenarios": [
    {
      "id": "ping",
      "system": "",
      "user": "ping",
      "max_latency_s": 30,
      "success": {"kind": "regex", "pattern": "^pong[.!]?\\s*$", "on": "text", "max_len": 100}
    },
    {
      "id": "reasoning",
      "system": "",
      "user": "Calcule 17 × 23 et explique brièvement.",
      "max_latency_s": 90,
      "success": {"kind": "all", "checks": [
        {"regex": "391", "on": "text"},
        {"jq": ".content | map(select(.type==\"thinking\")) | length > 0"}
      ]}
    },
    {
      "id": "file-extract",
      "system": "Read the embedded file content and answer in exactly one line.",
      "user_template": "Voici le contenu de robrowser/INDEX.md :\n\n<file>\n{{INDEX_MD_CONTENT}}\n</file>\n\nListe les plugins existants sur une seule ligne, noms séparés par des virgules, sans préambule.",
      "max_latency_s": 60,
      "success": {"kind": "regex", "pattern": "^[A-Za-z0-9_-]+(,\\s*[A-Za-z0-9_-]+){2,}\\s*$", "on": "text"}
    },
    {
      "id": "tool-discipline",
      "system": "Answer with exactly Y or N. No preamble, no tools, no thinking.",
      "user": "Should a Tampermonkey userscript be reloaded after a code edit?",
      "max_latency_s": 30,
      "success": {"kind": "all", "checks": [
        {"regex": "^[YN]$", "on": "text"},
        {"jq": ".content | map(select(.type==\"tool_use\")) | length == 0"}
      ]}
    }
  ]
}
```

`{{INDEX_MD_CONTENT}}` sera remplacé runtime par session 3 (lecture
fichier + substitution). En session 1 c'est juste le template statique.

Validation : `jq . scenarios.json` passe sans erreur, 4 scénarios listés.

### 3. claude-switch patch — `CLAUDE_LOCAL_PROFILE` env (MODIFY)

Modifier `.devcontainer/host-helpers/claude-switch` :

- Dans la section `local-proxy` (autour de la ligne où
  `ln -sfn ".devcontainer/claude/CLAUDE-local-dev.md" "$CLAUDE_MD"` est
  appelé) :
  - Lire `CLAUDE_LOCAL_PROFILE` depuis l'environnement ; si vide, parser
    `.devcontainer/.env` (`grep '^CLAUDE_LOCAL_PROFILE=' | cut -d= -f2-`)
  - Si défini et non-vide : cible
    `.devcontainer/claude/CLAUDE-local-${CLAUDE_LOCAL_PROFILE}-dev.md`
  - Vérifier que le fichier cible existe ; sinon erreur claire
    "profile <X> not found at <path>, run tune-claude-local <X> first
    or unset CLAUDE_LOCAL_PROFILE" et exit 2 SANS toucher le symlink
    existant
  - Sinon (unset/vide) : cible `.devcontainer/claude/CLAUDE-local-dev.md`
    (comportement actuel, inchangé)
- Le `status` subcommand de claude-switch affiche aussi le profile actif
  (lit la cible du symlink + reporte le profile name dérivé du filename
  s'il diffère du default)

Validation :
- Sans `CLAUDE_LOCAL_PROFILE` → symlink reste `CLAUDE-local-dev.md`
  (régression test : comportement actuel intact)
- Avec `CLAUDE_LOCAL_PROFILE=mac-light` + fichier mac-light existe (cf.
  livrable 7) → symlink sur `CLAUDE-local-mac-light-dev.md`
- Avec `CLAUDE_LOCAL_PROFILE=nonexistent` → erreur claire, symlink pas
  modifié, exit 2

### 4. Firewall — `domains.d/ollama.txt` (NEW)

`.devcontainer/firewall/domains.d/ollama.txt` :

```
# GET-only — Ollama public registry & library for model lookup during
# tune-claude-local research phase. NO POST. Required by
# plans/tune-claude-local-skill/ sessions 2+3.
ollama.com
registry.ollama.ai
```

L'user doit relancer `sudo /usr/local/bin/init-firewall.sh` après le
commit pour que la règle prenne effet — à mentionner explicitement dans
la wrap-up.

Validation :
- `bash .devcontainer/tests/diagnose.sh` passe sans regression sur la
  suite firewall après ajout (la suite doit accepter le nouveau fichier).
- Après `sudo init-firewall.sh` : `curl -sI https://ollama.com/library |
  head -1` → HTTP 200.

### 5. Bind-mount `.devcontainer/tmp/` dans docker-compose

Vérifier `.devcontainer/docker-compose.yml` :
- Si `.devcontainer/tmp/` n'est pas déjà bind-mounté sur le service
  principal → ajouter le mount.
- Si déjà OK → no-op, juste noter en LOG.md "déjà configuré".

Test : créer un fichier côté host dans `.devcontainer/tmp/` ; le voir
depuis le conteneur (et inversement).

### 6. Documentation root

#### CLAUDE.md (workspace root) — sous-section "Local mode tuning"

Insérer une sous-section dans la section appropriée (probablement après
le bloc DevContainer Phase 3) :

```
### Local mode tuning — generating a per-hardware CLAUDE-local-<profile>-dev.md

Pour optimiser le mode local-proxy pour un combo matos + modèle Ollama
spécifique, lancer côté host (workflow guidé par skill) :

  bash .devcontainer/host-helpers/tune-claude-local ask <profile-name>

Le skill `.devcontainer/skills/tune-claude-local/` (impl. sessions 2+3
du rollout `plans/tune-claude-local-skill/`) guide la session Claude
étape par étape : ask → research → propose → user-setup-ollama →
verify → baseline (4 scénarios) → iterate variants → finalize.
Resumable via `tune-claude-local status <profile>` ou
`tune-claude-local resume <profile>`.

Activer un profile pour `claude-switch local-proxy` :
  echo 'CLAUDE_LOCAL_PROFILE=<profile-name>' >> .devcontainer/.env
  bash .devcontainer/host-helpers/claude-switch local-proxy

Le symlink CLAUDE.md pointera sur CLAUDE-local-<profile>-dev.md.
Unset → fallback CLAUDE-local-dev.md (default).

Cf. plans/tune-claude-local-skill/ROLLOUT.md pour le détail complet.
```

#### `.devcontainer/.env.example` — doc commentée

Ajouter ligne commentée :

```
# Active un profile spécifique pour `claude-switch local-proxy` :
# le symlink CLAUDE.md pointera sur CLAUDE-local-<value>-dev.md.
# Unset → fallback sur CLAUDE-local-dev.md (default).
# Générer un profile via : bash .devcontainer/host-helpers/tune-claude-local ask <name>
# Cf. plans/tune-claude-local-skill/ROLLOUT.md
# CLAUDE_LOCAL_PROFILE=mac-light
```

#### `.gitignore` — ajouter tmp

Vérifier que `.devcontainer/tmp/` est ignoré ; sinon ajouter une ligne.

### 7. Archive profile session-2

`cp .devcontainer/claude/CLAUDE-local-dev.md .devcontainer/claude/CLAUDE-local-mac-light-dev.md`

Convention : `CLAUDE-local-dev.md` reste comme cible par défaut quand
`CLAUDE_LOCAL_PROFILE` unset. `CLAUDE-local-mac-light-dev.md` est
l'archive nommée du profile session-2 actuel (Mac unifié, backing
qwen3.5:9b aliasé en claude-opus-4-7).

Validation : diff -q entre les deux fichiers → 0 (identiques).

Validation end-to-end de la session :

1. `bash .devcontainer/host-helpers/tune-claude-local --help` → synopsis 9 sous-commandes
2. `bash .devcontainer/host-helpers/tune-claude-local status demo` → init state, dump JSON
3. `bash .devcontainer/host-helpers/tune-claude-local ask demo` → message TODO session 2, exit 99
4. `jq . .devcontainer/host-helpers/tune-claude-local-internals/scenarios.json` → no error, 4 scénarios
5. `CLAUDE_LOCAL_PROFILE=mac-light bash .devcontainer/host-helpers/claude-switch local-proxy` → symlink CLAUDE-local-mac-light-dev.md
6. `unset CLAUDE_LOCAL_PROFILE && bash .devcontainer/host-helpers/claude-switch local-proxy` → symlink CLAUDE-local-dev.md (régression OK)
7. `CLAUDE_LOCAL_PROFILE=nonexistent bash .devcontainer/host-helpers/claude-switch local-proxy` → erreur claire, exit 2, symlink intact
8. `bash .devcontainer/tests/diagnose.sh` → all green après ajout firewall

DoD at the end of this session :

1. **STATUS.md (ce rollout)** : flip session 1 row 📋 → ✅, prompt link
   → —, bump Delivered 0→1, set "Next focus" → session 2 (discovery).
2. **LOG.md (ce rollout)** : append `## 1 — skeleton-and-wiring` section
   avec files touched + What / Why / Decisions / Gotchas / Tests / Commit.
   Inclure explicitement les 8 décisions verrouillées (cf. ROLLOUT.md
   section Decisions) comme rappel non-régression.
3. **EXISTING.md (ce rollout)** : ajouter les fichiers nouveaux session 1
   (tune-claude-local CLI skeleton, scenarios.json, domains.d/ollama.txt,
   CLAUDE-local-mac-light-dev.md).
4. **Single commit** proposé (NOT exécuté sans confirmation user).
   Message :
   ```
   Scaffold tune-claude-local skeleton + claude-switch profile env

   - Add host-helpers/tune-claude-local: 9-subcommand CLI skeleton with
     status/resume implemented, other commands stubbed for sessions 2+3.
     Initializes state dir at .devcontainer/tmp/tune-claude-local/<profile>/
   - Add tune-claude-local-internals/scenarios.json: 4-test battery spec
     (ping, reasoning, file-extract, tool-discipline) consumed in session 3
   - Extend claude-switch local-proxy with CLAUDE_LOCAL_PROFILE env support
     for per-profile CLAUDE-local-<name>-dev.md symlink selection
   - Add firewall/domains.d/ollama.txt for ollama.com + registry.ollama.ai
     GET allowlist (consumed in session 2 research phase)
   - Archive session-2 converged profile as CLAUDE-local-mac-light-dev.md
   - Document the workflow in CLAUDE.md and .env.example
   ```
5. **Reminder à l'user après commit** : `sudo /usr/local/bin/init-firewall.sh`
   pour activer la règle ollama.com (le firewall ne re-lit pas `domains.d/`
   automatiquement).
6. **Hand-off à session 2** : afficher en wrap-up le path du prompt
   session 2 (`plans/tune-claude-local-skill/sessions/session-2-discovery.md`)
   et un rappel court de son scope (`ask` / `research` / `propose` /
   `verify` impl + skill .md phases 1-5).
`````

## Next session

Session 2 — discovery workflow : impl `ask` / `research` / `propose` /
`verify` subcommands + skill .md phases 1-5 + AskUserQuestion templates
pour HW + modèle + queries ollama.com.

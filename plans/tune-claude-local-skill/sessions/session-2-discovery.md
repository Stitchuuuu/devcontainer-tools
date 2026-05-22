# Session 2 — discovery workflow

> **Effort** : ~60-90 min | **Dependencies** : session 1 ✅ (CLI skeleton + state dir layout + firewall ollama.com must exist before implementing the interactive subcommands)

## Prompt to paste

`````
Je démarre la session 2 du rollout `tune-claude-local-skill`.

Entry point : `/workspace/plans/tune-claude-local-skill/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are — 1/3 delivered, session 2 = discovery)
- `LOG.md` (session 1 entry — what's already wired up)
- `EXISTING.md` (updated state after session 1)
- `sessions/session-2-discovery.md` (this spec)
- `sessions/session-3-tuning.md` (next — to anticipate the contract for `inventory.json` / `research.json` / `proposal.json` consumed by session 3)
- `.devcontainer/host-helpers/tune-claude-local` (session 1 skeleton — extend the stubs of `ask` / `research` / `propose` / `verify`)
- `.devcontainer/skills/watch-log/watch-log.skill.md` (reference for skill .md format)
- `.devcontainer/skills/prepare-pr/prepare-pr.skill.md` (reference for multi-step skill workflow)
- `.devcontainer/firewall/domains.d/ollama.txt` (firewall allowance for ollama.com — already added in session 1)
- Parent rollout's session 2 LOG entry for the Phase 0 gate streaming pattern (will be re-used in `verify`).

Goal : impl phases 1-5 du workflow (ask / research / propose / verify)
de façon interactive et resumable. Crée le fichier skill .md qui guide
la session Claude à travers ces 4 sous-commandes (avec resume support).
À la fin, on peut faire `tune-claude-local ask demo` → répondre aux
questions → `research demo` → query ollama.com → `propose demo` →
afficher recommandations → user setup Ollama manuellement → `verify
demo` → green light pour le baseline (session 3).

Concrete deliverables :

### 1. `cmd_ask <profile>` impl (MODIFY tune-claude-local CLI)

Comportement :
- Si `state.json.current_step` ∉ {`init`, `ask`} → refuser ("ask already
  done at <ts> ; use `tune-claude-local status <profile>` to see the
  current phase, or pass --restart to wipe state — TODO add --restart
  flag if needed")
- Sinon : appel à `tune-claude-local-internals/ask-interactive.sh
  <profile>` qui pose les questions et écrit `inventory.json`
- Met à jour `state.json.current_step = "ask"` au succès

`inventory.json` schéma :
```json
{
  "profile": "mac-pro",
  "model_intent": "named" | "recommend",
  "model_name": "qwen3:14b" | null,
  "hw": {
    "ram_gb": 64,
    "vram_gb": 48,
    "unified_memory_gb": null,
    "cpu": "Apple M3 Max",
    "gpu": "Apple M3 Max integrated",
    "os": "macOS 14.5"
  },
  "asked_at": "2026-05-21T12:34:56Z"
}
```

NOTE : la HW probe est purement question-ouverte côté user (décision
verrouillée ROLLOUT.md). PAS de `system_profiler` / `lscpu` /
`nvidia-smi` auto-call.

### 2. `tune-claude-local-internals/ask-interactive.sh` (NEW)

Script bash qui pose les questions interactives via `read -r -p` (le
helper tourne sur le HOST en TTY interactif via `docker exec -it` ou
direct).

Questions à poser (kebab-case ou réponse libre selon) :
- "Profile name (kebab-case)" — déjà reçu en arg, juste confirmer
- "Do you already have a target Ollama model in mind, or do you want
  recommendations? [named/recommend]"
- Si named : "Model name as it appears in Ollama (e.g. qwen3:14b,
  llama3.1:8b)"
- "RAM (GB)" — entier
- "VRAM dédiée (GB), ou laisser vide si Mac à mémoire unifiée"
- "Mémoire unifiée (GB), ou laisser vide si VRAM dédiée renseignée"
- "CPU model (free text, e.g. 'Apple M3 Max', 'Intel i9-13900K')"
- "GPU (free text, e.g. 'Apple M3 Max integrated', 'NVIDIA RTX 4090')"
- "OS + version (e.g. 'macOS 14.5', 'Ubuntu 22.04')"

Validation des réponses :
- Numerics : refuse si non-int
- VRAM XOR unified_memory : un des deux doit être renseigné (sinon
  re-ask)
- Tous les autres champs : non-vide

À la fin, dump `inventory.json` via `jq -n --arg ... '{}'`.

### 3. `cmd_research <profile>` impl + `query-ollama.sh` (NEW)

Comportement :
- Prerequisite : `state.json.current_step == "ask"`
- Lit `inventory.json`
- Appelle `tune-claude-local-internals/query-ollama.sh <profile>` qui :
  - Tente `curl -sS --max-time 5 http://ollama.internal:11434/api/tags`
    pour lister modèles déjà pullés localement (utile pour proposer ou
    pour confirmer existence du modèle named)
  - Si timeout / 404 / NXDOMAIN → exit code spécial (3) avec message
    explicite "Ollama local unreachable" — `cmd_research` catch ce code
    et propose à l'user via `AskUserQuestion` (dans le skill .md) de
    démarrer Ollama, puis retry. PAS de `/watch-log`, PAS d'auto-start.
  - Si `inventory.json.model_intent == "named"` :
    - `curl -sS https://ollama.com/library/<model-base>` (HTML page)
    - Extraire params + variants disponibles (regex sur le HTML — les
      modèles Ollama ont des pages avec une table de variants visible)
    - Ou alternative : `curl -sS https://ollama.com/api/library/<model-base>`
      si endpoint JSON existe — préférer JSON si dispo, scraping HTML
      en fallback
  - Si `inventory.json.model_intent == "recommend"` :
    - `curl -sS https://ollama.com/library` (page listing tous les
      modèles populaires) — extraire un set de candidates avec leurs
      tailles
    - Filtrer par compatibilité matos (taille modèle Q4 vs VRAM/unified
      memory dispo) — heuristique simple : modèle quantifié Q4 ≈
      params_b * 0.5 GB, ajouter buffer ctx 32k ≈ 2 GB
- Écrit `research.json`

`research.json` schéma :
```json
{
  "ollama_local_reachable": true,
  "ollama_local_models": ["qwen3:14b", "llama3.1:8b", "claude-opus-4-7"],
  "research_source": "ollama.com",
  "candidates": [
    {
      "name": "qwen3:14b",
      "url": "https://ollama.com/library/qwen3",
      "params_b": 14,
      "variants": [
        {"tag": "14b-q4_0", "size_gb": 8.5, "ctx_default": 32768, "ctx_max": 131072},
        {"tag": "14b-q5_K_M", "size_gb": 10.2, "ctx_default": 32768, "ctx_max": 131072}
      ],
      "description": "Latest Qwen3 family. Reasoning model. Multi-lingual."
    }
  ],
  "researched_at": "2026-05-21T12:35:00Z"
}
```

Met à jour `state.json.current_step = "research"`.

### 4. `cmd_propose <profile>` impl

Comportement :
- Prerequisite : `state.json.current_step == "research"`
- Lit `inventory.json` + `research.json`
- **N'écrit PAS de recommendation directement** — le skill .md instruit
  la session Claude à raisonner sur les facts JSON et proposer un setup.
- Le helper se contente d'écrire `proposal.json` avec la STRUCTURE
  attendue (vide pour Claude à remplir) + d'afficher à l'écran le
  contenu de `inventory.json` + `research.json` formaté pour lecture
  humaine, comme prompt pour la session Claude.
- Met à jour `state.json.current_step = "propose"`

`proposal.json` schéma (rempli par Claude session dans le skill workflow,
PAS par le helper) :
```json
{
  "recommended_model": "qwen3:14b",
  "recommended_variant": "14b-q4_0",
  "recommended_ctx": 32768,
  "gpu_offload": {"num_gpu": 999, "num_thread": 8},
  "rationale": "free-text 2-3 lines : pourquoi ce variant, vs alternatives",
  "cloud_comparison": "Cloud Opus 4.7 has 1M ctx (Claude Max sub). This local setup has 32k ctx — adequate for code edits but not for full repo dump. Reasoning quality expected to lag cloud by ~30-40% on multi-step tasks.",
  "user_setup_commands": [
    "ollama pull qwen3:14b",
    "ollama cp qwen3:14b claude-opus-4-7"
  ],
  "proposed_at": "2026-05-21T12:36:00Z"
}
```

Le skill .md guide la session Claude à remplir ces champs (la session
fait `cat .devcontainer/tmp/.../inventory.json
.devcontainer/tmp/.../research.json` puis raisonne + écrit
`proposal.json` via un sub-helper `tune-claude-local propose-write`
TBD si utile, ou directement via Write tool sur le fichier).

### 5. Phase 4 manuelle (l'user setup Ollama lui-même)

PAS un sous-commande du helper. Le skill .md instruit Claude à :
- Afficher les commandes de `proposal.json.user_setup_commands` à l'user
- `AskUserQuestion` : "Done? Y/N"
- Sur Y : passer à `verify`. Sur N : afficher une fois encore les
  commandes, attendre.

### 6. `cmd_verify <profile>` impl + `run-gate.sh` (NEW)

Comportement :
- Prerequisite : `state.json.current_step == "propose"`
- Vérifie 3 conditions :
  - `curl -sS http://ollama.internal:11434/api/show -d '{"name":"claude-opus-4-7"}'`
    → retourne 200 avec un body qui contient les facts du modèle
    backing. Confirme que l'alias `ollama cp` a été fait.
  - `curl -sS http://claude-bridge:9223/health` → 200 OK
  - Lance `tune-claude-local-internals/run-gate.sh` (script extrait de
    parent session 2 — le streaming `<think>` curl direct sur le bridge
    avec un prompt de raisonnement, vérifie que la traduction
    `reasoning → thinking` est fonctionnelle pour le NOUVEAU backing
    model). Critère pass : au moins 1 `content_block_start[thinking]`
    SSE event reçu.
- Écrit `verify.json` :
```json
{
  "ollama_alias_ok": true,
  "bridge_health_ok": true,
  "phase0_gate_ok": true,
  "errors": [],
  "verified_at": "2026-05-21T12:37:00Z"
}
```
- Si une vérif foire : `errors[]` populé + exit code spécifique
- Met à jour `state.json.current_step = "verify"`

### 7. Skill .md `.devcontainer/skills/tune-claude-local/tune-claude-local.skill.md` (NEW)

Frontmatter :
```yaml
---
description: Generate or refresh CLAUDE-local-<profile>-dev.md by guiding the user step-by-step through model selection, Ollama setup, and CLAUDE.md fine-tuning for a specific hardware profile. Resumable at any phase. Outputs are activated via CLAUDE_LOCAL_PROFILE env in .devcontainer/.env.
argument-hint: "<profile-name>"
---
```

Body — workflow phases 1-5 (phases 6-9 ajoutées en session 3) :

1. **Bootstrap** — `bash .devcontainer/host-helpers/tune-claude-local
   status <profile>`. Si state absent → profile nouveau, va step 2.
   Sinon → lit `current_step`, saute au step approprié.
2. **Ask (Phase 1)** — Si current_step ∉ {init, ask} → skip. Sinon :
   `bash .devcontainer/host-helpers/tune-claude-local ask <profile>`
   (script interactif côté terminal pose les questions, l'user répond).
   Une fois fini, lit `inventory.json` et confirme à l'user.
3. **Research (Phase 2)** — `bash .devcontainer/host-helpers/tune-claude-local
   research <profile>`. Si exit code 3 ("Ollama local unreachable"),
   `AskUserQuestion` "Ollama doesn't respond on `ollama.internal:11434`.
   Please start it on the host (`ollama serve` or `brew services start
   ollama`), then click 'Done'." Sur 'Done' → retry `research`. PAS de
   /watch-log.
4. **Propose (Phase 3)** — `bash .devcontainer/host-helpers/tune-claude-local
   propose <profile>`. Lit `inventory.json` + `research.json` affichés,
   raisonne sur le combo, écrit `proposal.json` (champs ci-dessus +
   rationale 2-3 lignes + cloud_comparison explicite). Affiche le
   contenu de proposal.json à l'user pour validation, ajuste si user
   demande.
5. **User Ollama setup (Phase 4 manuelle)** — Affiche les commandes de
   `proposal.json.user_setup_commands` à l'user. `AskUserQuestion`
   "Done?". Sur Done → step suivant.
6. **Verify (Phase 5)** — `bash .devcontainer/host-helpers/tune-claude-local
   verify <profile>`. Lit `verify.json`. Si tous greens → annonce
   "Ready for baseline (next session 3 phases 6-9)". Si erreurs →
   affiche remediation, AskUserQuestion pour retry après fix.

(Phases 6-9 TBA en session 3.)

Validation impl session 2 :

1. `bash .devcontainer/host-helpers/tune-claude-local ask demo` →
   pose les questions, écrit `inventory.json` valide (`jq .`)
2. `bash .devcontainer/host-helpers/tune-claude-local research demo`
   (Ollama local up) → écrit `research.json` avec au moins 1 candidate,
   GET https://ollama.com/library/<X> réussit
3. `bash .devcontainer/host-helpers/tune-claude-local research demo`
   (Ollama local down — `sudo systemctl stop ollama` ou équivalent) →
   exit code 3, message clair pour skill .md
4. `bash .devcontainer/host-helpers/tune-claude-local propose demo` →
   affiche inventory+research, crée proposal.json stub vide
5. `bash .devcontainer/host-helpers/tune-claude-local verify demo`
   (sans Ollama setup complet) → exit non-zéro, `verify.json` montre
   les erreurs
6. Skill discoverable : `ls ~/.claude/commands/tune-claude-local.md`
   après `sync-skills.sh`
7. Invocation naturelle "tune local mode for my new mac" → skill se
   déclenche

DoD at the end of this session :

1. **STATUS.md** : flip session 2 row 📋 → ✅, Delivered 1→2, Next
   focus → session 3 (tuning).
2. **LOG.md** : append `## 2 — discovery-workflow` section.
3. **EXISTING.md** : ajouter les internals `.sh` + skill .md + le
   shape des JSON state files.
4. **Single commit** proposé (NOT exécuté) :
   ```
   Implement tune-claude-local discovery workflow (phases 1-5)

   - cmd_ask + ask-interactive.sh: collect HW + model intent via TTY
     prompts, write inventory.json
   - cmd_research + query-ollama.sh: query ollama.internal + ollama.com,
     write research.json (candidates + variants)
   - cmd_propose: dump structured proposal.json scaffold for the skill
     workflow's Claude session to fill via reasoning
   - cmd_verify + run-gate.sh: check alias + bridge health + Phase 0
     streaming <think> gate, write verify.json
   - skills/tune-claude-local/tune-claude-local.skill.md: phases 1-5
     workflow with resume support and Ollama-unreachable AskUserQuestion
     fallback
   ```
`````

## Next session

Session 3 — tuning loop & demo : impl `baseline` / `apply` / `measure` /
`finalize` + refactor des internals depuis parent rollout session 2 +
skill .md phases 6-9 + demo end-to-end.

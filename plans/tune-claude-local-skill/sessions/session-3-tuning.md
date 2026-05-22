# Session 3 — tuning loop & demo

> **Effort** : ~60-90 min | **Dependencies** : session 2 ✅ (need ask/research/propose/verify subcommands + `inventory.json` / `proposal.json` / `verify.json` schemas to chain into baseline + iterate)

## Prompt to paste

`````
Je démarre la session 3 du rollout `tune-claude-local-skill`.

Entry point : `/workspace/plans/tune-claude-local-skill/ROLLOUT.md`
Read also :
- `STATUS.md` (où on en est — 2/3 delivered, session 3 = tuning loop & demo)
- `LOG.md` (sessions 1+2 outcomes — état actuel du CLI + skill .md)
- `EXISTING.md` (updated state après sessions 1+2)
- `sessions/session-3-tuning.md` (CE spec)
- `.devcontainer/host-helpers/tune-claude-local` (extend avec baseline/apply/measure/finalize)
- `.devcontainer/skills/tune-claude-local/tune-claude-local.skill.md` (extend avec phases 6-9)
- Parent rollout (référence — sera refactoré ici) :
  - `../uniclaudeproxy-integration-local-opti/LOG.md` (sections 2 + F2 — la tuning loop manuelle, qui définit le comportement à automatiser)
  - `.devcontainer/tests/diag-bridge-translation.sh` (268 lignes — source pour `replay-scenarios.sh`)
  - `.devcontainer/tests/tweak-claude-md-for-local.sh` (207 lignes — source pour `apply-variant.sh`)
- `.devcontainer/host-helpers/tune-claude-local-internals/scenarios.json` (session 1 — les 4 scénarios à consommer ici)
- `.devcontainer/host-helpers/mitm-capture` (re-used pour auto-`on` en pre-flight baseline)

Goal : impl phases 6-9 du workflow (baseline / iterate / stop / finalize)
en factorisant les 2 scripts ad-hoc du parent rollout session-2 dans
les internals du skill. À la fin, on peut faire un end-to-end run sur
le profile `demo` (ou `mac-light` si besoin) :
ask → research → propose → user setup → verify → baseline → iterate (≥1
variant) → finalize → CLAUDE-local-demo-dev.md généré + session log
écrit + reminder pour activer le profile via env.

Concrete deliverables :

### 1. `cmd_baseline <profile>` impl (MODIFY tune-claude-local CLI)

Comportement :
- Prerequisite : `state.json.current_step == "verify"` (verify a passé)
- Pre-flight : check `mitm-capture status` ; si off → `mitm-capture on`
  avec trap pour restore prior state au exit (trap '`mitm-capture
  $PRIOR_STATE`' EXIT)
- Lit `scenarios.json` ; pour chaque scénario :
  - Substitute `{{INDEX_MD_CONTENT}}` (lit `/workspace/robrowser/INDEX.md`)
    si le scénario a un `user_template` au lieu d'un `user` (cas
    file-extract)
  - Capture cloud SEND via `claude --print` (cloud-as-oracle) + content-match
    new capture from `/tmp/claude-capture/` (réutilise `new_send_matching`
    pattern de parent session-2)
  - Replay contre `claude-bridge:9223` avec model rewrite +
    `stream:false`
  - Mesure : latence (`curl -w '%{time_total}'`), token counts, content
    blocks structure, thinking-block presence
  - Évalue success predicate (`regex`/`jq`/`all`) défini dans `scenarios.json`
- Écrit `baseline.json` :
```json
{
  "scenarios": [
    {
      "id": "ping",
      "pass": true,
      "latency_s": 3.9,
      "tokens_in": 1234,
      "tokens_out": 12,
      "content_structure": [{"type":"text"}],
      "preview": "pong"
    },
    ...
  ],
  "all_pass": false,
  "failed_ids": ["file-extract"],
  "measured_at": "..."
}
```
- Met à jour `state.json.current_step = "baseline"`

### 2. `cmd_apply <profile> --variant <label> --body-file <path>` impl

Comportement :
- Prerequisite : `state.json.current_step ∈ {"baseline", "iterate"}`
- Lit le file `<path>` (body markdown entre les markers
  `<!-- 1B-LIGHT-MODEL-DIRECTIVES-START/END -->`)
- Appelle `tune-claude-local-internals/apply-variant.sh
  --variant-label <label> --body-file <path>` qui :
  - Lit le fichier `CLAUDE-local-<profile>-dev.md` (existe déjà OU
    bootstrap depuis `CLAUDE-local-dev.md` si profil neuf)
  - Remplace le bloc entre markers idempotemment (awk-based, comme
    `tweak-claude-md-for-local.sh`)
  - Affiche un diff résumé
- Mets à jour `state.json.current_step = "iterate"` + log le variant
  label dans `variants.md` du state dir

### 3. `cmd_measure <profile> --label <V<n>-<slug>>` impl

Comportement :
- Prerequisite : `state.json.current_step == "iterate"`
- Identique à `baseline` mais écrit `iter-NNN.json` (auto-increment N
  selon les fichiers existants dans le state dir) au lieu de
  `baseline.json`
- Le `--label` est embarqué dans le JSON pour traçabilité
- Compare implicitement avec baseline.json (delta latence / pass-rate)
  et l'inclut dans le JSON :
```json
{
  "label": "V1-explicit-tiny-model",
  "scenarios": [...],
  "all_pass": true,
  "delta_vs_baseline": {
    "ping_latency_s": -13.5,
    "reasoning_latency_s": -27.1,
    "new_passes": ["file-extract"],
    "new_failures": []
  },
  "measured_at": "..."
}
```

### 4. `cmd_finalize <profile>` impl

Comportement :
- Prerequisite : `state.json.current_step ∈ {"baseline", "iterate"}` ET
  au moins un fichier `iter-NNN.json` OU baseline.json all_pass
- Pick la meilleure itération (all_pass ET min latence cumulée), OU
  baseline si elle pass déjà tout
- Écrit `.devcontainer/claude/CLAUDE-local-<profile>-dev.md` (rename de
  `final.md` dans le state dir)
- Append session log à `plans/tune-claude-local-skill/sessions/finalized/tune-local-<profile>.md`
  (nouveau dossier `finalized/` pour distinguer des spec sessions) avec :
  - HW inventory (depuis inventory.json)
  - Modèle Ollama backing (depuis proposal.json)
  - Variant ledger (toutes les iter avec leurs mesures)
  - Final pick (le variant retenu + mesures)
  - Date + profile name
- Met à jour `state.json.current_step = "done"`
- Affiche à l'écran :
  ```
  ✅ Profile <X> finalized.
  Archived : .devcontainer/claude/CLAUDE-local-<X>-dev.md
  Session log : plans/tune-claude-local-skill/sessions/finalized/tune-local-<X>.md

  To activate this profile :
    echo 'CLAUDE_LOCAL_PROFILE=<X>' >> .devcontainer/.env
    bash .devcontainer/host-helpers/claude-switch local-proxy
  ```

### 5. `tune-claude-local-internals/replay-scenarios.sh` (NEW)

Factoring de `parent_rollout/.devcontainer/tests/diag-bridge-translation.sh`
(268 lignes) :

- Fonctions à conserver telles quelles (battle-tested session-2) :
  - `new_send_matching()` : content-match nouvelle capture (protège
    contre sessions concurrentes)
  - `run_cloud_capture()` : `claude --print` dans env minimal pour
    forcer cloud
  - `replay_bridge()` : warmup + curl mesuré contre `:9223`
- Nouveautés à ajouter :
  - Param `--scenario-id <id>` : lit le scénario depuis `scenarios.json`
    (au lieu du prompt hard-codé "ping" + "reasoning")
  - Param `--label <label>` : nom de la mesure pour le fichier sortie
  - Param `--output <path>` : où écrire le JSON résultat (baseline.json
    ou iter-NNN.json selon caller)
  - `--substitute-index-md` : si présent, lit `robrowser/INDEX.md` et
    substitue `{{INDEX_MD_CONTENT}}` dans le user_template
  - Évaluation du `success` predicate (regex/jq/all) défini dans le
    scénario
- L'API : `bash replay-scenarios.sh --profile <X> --scenario-id <id>
  --output <path> [--label <label>] [--substitute-index-md]`

### 6. `tune-claude-local-internals/apply-variant.sh` (NEW)

Factoring de `parent_rollout/.devcontainer/tests/tweak-claude-md-for-local.sh`
(207 lignes) :

- Garde la logique idempotente awk-based (battle-tested) entre les
  markers `<!-- 1B-LIGHT-MODEL-DIRECTIVES-START/END -->`
- Supprime les HEREDOC V0/V1 hard-codés (le helper passe le body via
  `--body-file`)
- API : `bash apply-variant.sh --profile <X> --variant-label <L>
  --body-file <path>`
- Comportement :
  - Cible `.devcontainer/claude/CLAUDE-local-<X>-dev.md` (NOT
    `CLAUDE-local-dev.md` ; chaque profile a son fichier)
  - Si fichier absent : bootstrap depuis `CLAUDE-local-dev.md` + ajoute
    les markers s'ils manquent
  - Remplace le bloc entre markers par le contenu de `--body-file`
  - Affiche diff résumé

### 7. Skill .md `.devcontainer/skills/tune-claude-local/tune-claude-local.skill.md` (EXTEND)

Ajouter phases 6-9 au workflow Claude :

6. **Baseline (Phase 6a)** — Si current_step == "verify" :
   `bash .devcontainer/host-helpers/tune-claude-local baseline <profile>`.
   Lit `baseline.json`. Report pass/fail par scénario avec latence +
   token counts. Affiche la comparaison cloud Opus 4.7 (1M ctx, Claude
   Max) vs ce setup local pour situer le tradeoff.

7. **Iterate (Phase 6b)** — boucle ≤ `--max-iterations` (default 5) :
   - Si `baseline.json.all_pass == true` → skip iterate, go finalize
   - Sinon, Claude propose UNE variante body (markdown entre markers),
     justifié par les failed scenarios. Montre inline.
   - `AskUserQuestion` Y/n
   - Sur Y : écrit body dans
     `.devcontainer/tmp/tune-claude-local/<profile>/pending-variant-V<n>.md`,
     appelle `apply` + `measure --label V<n>-<slug>`
   - Lit `iter-NNN.json`, compare deltas
   - Stop conditions :
     - Tous scénarios pass → finalize
     - Max iterations atteint → ask user (continue / finalize / abort)
     - 2 iter consécutives sans amélioration → ask user
     - User dit stop

8. **Finalize (Phase 7)** —
   `bash .devcontainer/host-helpers/tune-claude-local finalize <profile>`.
   Helper écrit le profile + session log. Claude affiche un wrap message
   avec liens cliquables et la commande pour activer
   `CLAUDE_LOCAL_PROFILE`.

9. **Done** — Reminder à l'user que :
   - Le profile est archivé sous `CLAUDE-local-<X>-dev.md`
   - Pour réactiver plus tard sans re-tuner : `CLAUDE_LOCAL_PROFILE=<X>`
   - Pour re-tuner (nouveau matos) : nouveau profile name, new flow
   - Tu peux toujours visualiser : `tune-claude-local status <X>`

### 8. End-to-end demo

Sur un profile (ex: `demo` ou recyclage de `mac-light`) :

```bash
# Démarrer le skill via une session Claude — il guide à travers
# les 9 phases. À la fin, vérifier :

# 1. Le profile est archivé
test -f /workspace/.devcontainer/claude/CLAUDE-local-demo-dev.md && echo OK

# 2. Le session log existe
test -f /workspace/plans/tune-claude-local-skill/sessions/finalized/tune-local-demo.md && echo OK

# 3. claude-switch active bien le profile
echo 'CLAUDE_LOCAL_PROFILE=demo' >> /workspace/.devcontainer/.env
bash /workspace/.devcontainer/host-helpers/claude-switch local-proxy
readlink /workspace/CLAUDE.md
# → .devcontainer/claude/CLAUDE-local-demo-dev.md

# 4. Le fichier final est sain (markers présents, body cohérent)
grep -c '1B-LIGHT-MODEL-DIRECTIVES-START' /workspace/.devcontainer/claude/CLAUDE-local-demo-dev.md
# → 1
```

### 9. Annoter les scripts session-2 du parent comme superseded

Header comment dans :
- `parent_rollout/.devcontainer/tests/diag-bridge-translation.sh`
- `parent_rollout/.devcontainer/tests/tweak-claude-md-for-local.sh`

Texte :
```
# NOTE: Superseded by tune-claude-local skill (plans/tune-claude-local-skill/).
# Refactored into .devcontainer/host-helpers/tune-claude-local-internals/
# replay-scenarios.sh + apply-variant.sh. Kept here for ad-hoc debugging.
```

Validation end-to-end session 3 :

1. `bash .devcontainer/host-helpers/tune-claude-local baseline <profile>`
   → écrit `baseline.json` avec 4 scénarios mesurés ; ping + reasoning
   passent au minimum (déjà validé en parent session-2)
2. `bash .devcontainer/host-helpers/tune-claude-local apply <profile>
   --variant V1-test --body-file /tmp/test-body.md`
   → modifie `.devcontainer/claude/CLAUDE-local-<profile>-dev.md` entre
   les markers, idempotent (re-run → no change)
3. `bash .devcontainer/host-helpers/tune-claude-local measure <profile>
   --label V1-test`
   → écrit `iter-001.json` avec delta vs baseline
4. `bash .devcontainer/host-helpers/tune-claude-local finalize <profile>`
   → écrit `CLAUDE-local-<profile>-dev.md` + session log finalized
5. Activation via env : `CLAUDE_LOCAL_PROFILE=<profile>` puis
   `claude-switch local-proxy` → symlink correct
6. Full flow Claude-driven : invocation "tune local mode for my mac"
   déclenche le skill ; les 9 phases roulent end-to-end ; resume après
   interruption (ex: kill au phase 5) repart proprement via
   `tune-claude-local status <profile>`

DoD at the end of this session :

1. **STATUS.md (ce rollout)** : flip session 3 row 📋 → ✅, prompt link
   → —, bump Delivered 2→3, set "Next focus" → "rollout complete (skill
   shipped)".
2. **LOG.md (ce rollout)** : append `## 3 — tuning-loop-and-demo` section
   avec files touched + What / Why / Decisions / Gotchas / Tests / Commit.
   Inclure :
   - Le profile généré pour la demo
   - Tout deviation découverte vs le process manuel session-2 parent
   - Le diff entre `diag-bridge-translation.sh` (268 LOC) et
     `replay-scenarios.sh` (LOC final) — montre la valeur de la
     généralisation
3. **EXISTING.md (ce rollout)** : ajouter les nouveaux internals .sh +
   le profile demo finalisé + le session log finalized/.
4. **Parent rollout** :
   - Mettre à jour le header de
     `parent_rollout/.devcontainer/tests/diag-bridge-translation.sh` et
     `tweak-claude-md-for-local.sh` avec note "superseded" pointant vers
     les internals.
   - **N'enlève PAS le statut 📦 deferred de la session 3 dans
     STATUS.md parent** — c'est correct historiquement.
   - Optionnel : append une ligne dans
     `parent_rollout/LOG.md` "## 3 — extracted (deferred)" disant que
     le skill rollout a été complété (1 ligne, pas de full entry).
5. **Single commit** proposé (NOT exécuté) :
   ```
   Implement tune-claude-local tuning loop + finalize the skill rollout

   - cmd_baseline + cmd_measure: run 4-scenario battery via
     replay-scenarios.sh (factored from parent rollout session-2's
     diag-bridge-translation.sh). Reuses new_send_matching content-match,
     run_cloud_capture, replay_bridge functions; adds scenarios.json
     consumption + INDEX.md content substitution + success predicate
     evaluation
   - cmd_apply: idempotent variant applier via apply-variant.sh (factored
     from parent rollout session-2's tweak-claude-md-for-local.sh);
     bootstraps per-profile CLAUDE-local-<X>-dev.md from default on
     first apply
   - cmd_finalize: writes per-profile archive + appends finalized session
     log + prompts user to set CLAUDE_LOCAL_PROFILE
   - skills/tune-claude-local/tune-claude-local.skill.md: phases 6-9
     workflow (baseline + iterate + stop conditions + finalize)
   - Annotate parent rollout's session-2 scripts as superseded
   - End-to-end demo on profile `demo` (or rerun on mac-light)
   ```
`````

## Next session

None — the skill rollout completes after this. The shipped artifacts :

- `.devcontainer/host-helpers/tune-claude-local` (9-subcommand CLI complete)
- `.devcontainer/host-helpers/tune-claude-local-internals/` (6 files :
  scenarios.json + ask-interactive.sh + query-ollama.sh + run-gate.sh +
  replay-scenarios.sh + apply-variant.sh)
- `.devcontainer/skills/tune-claude-local/tune-claude-local.skill.md`
- At least one finalized profile in `.devcontainer/claude/`
- Session log in `plans/tune-claude-local-skill/sessions/finalized/`

Future work (hors scope ce rollout, à scaffolder via /prepare-plan si
besoin) :
- `claude-switch local-proxy <profile>` positional arg (alternative à
  l'env var, listé en parking lot ROLLOUT.md)
- Auto-detect HW (system_profiler/lscpu) — explicit out of scope per
  Decisions
- CI/regression suite re-evaluating per-profile files on every UCP /
  Claude Code release
- Publication upstream (claude-code-skills repo)

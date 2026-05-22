# Part 1 — session 3b — gitignore-architecture-refactor

> **Effort** : ~45 min — 1 h | **Dependencies** : Part 1 session 2
> (install-redesign) delivered. Indépendante de session 3
> (firewall-layer-split) — peuvent partir en parallèle ou dans n'importe
> quel ordre. Découverte pendant fresh-install validation.

## Why this session

Trois problèmes surfacés pendant la validation fresh-install :

1. **`update_gitignore()` incomplet** ([install.sh:324-356](install.sh#L324-L356))
   — écrit 13 entrées mais en oublie ~9 par rapport au `.gitignore` du
   repo template lui-même. Manquent : `.env.dev`, `.DS_Store`,
   `firewall-blocks.*.log`, `__pycache__/`, `tests/diag*.log`,
   `*.local.skill.md`, `claude-bridge/config.json`, `LESSONS.local.md`.

2. **Conflit `.vscode/`** — install.sh écrit `.vscode/` (full ignore)
   dans le projet cible, mais [post-start.sh:32](templates/v2/post-start.sh#L32)
   fait `update-index --skip-worktree .vscode/settings.json` qui n'a
   effet que sur fichier **tracké**. Le bind-mount de
   [docker-compose.yml:49](templates/v2/docker-compose.yml#L49) bénéficie
   d'un `vscode-settings.json` commité (visible aux autres devs), pas
   d'un fichier full-ignored.

3. **LESSONS au root** — `LESSONS.md` / `LESSONS.local.md` au root sont
   l'exception dans un écosystème AI sinon entièrement contenu dans
   `.devcontainer/` (`claude/`, `knowledge/`, `skills/`, `claude-bridge/`).
   `CLAUDE.md` au root est déjà un symlink vers
   `.devcontainer/claude/CLAUDE-<mode>.md` (vérifié : `git ls-files -s`
   montre mode 120000 — le symlink commit OK).

**Le fix** : split du gitignore entre `.devcontainer/.gitignore` (shipped
par le template, scoped aux entrées internes) et `update_gitignore()`
(root-only entries). Relocate LESSONS dans `.devcontainer/` avec symlink
root pour visibilité (même pattern que CLAUDE.md). Whitelist
`.vscode/settings.json` au gitignore root pour que le skip-worktree
fonctionne.

## Where this session runs

Édit canonique : `templates/v2/` + `install.sh` au root du repo
`devcontainer-tools` (ou son alias host `devcontainer-tools-v2/`). Si
le repo a son propre `.devcontainer/` pour dogfooding, mirror les
changements applicables (drop l'ancien `.devcontainer/.gitignore` si
divergent, ajouter le `LESSONS.md` + symlink).

Projets adoptants (cyro-live, portal42, etc.) inherit le fix au prochain
`install.sh` rerun. **MAIS** ré-installer écrase les fichiers existants —
si un projet a déjà customisé son `.devcontainer/.gitignore` ou son
`LESSONS.md`, le rerun perd ces customisations (le `LESSONS.md` cp
n'overwrite que si absent ; `.gitignore` peut être plus violent). Pas
pertinent pour cette session, juste à flagger pour session 5 CHANGELOG.

## Prompt to paste

`````
Je démarre la Part 1 session 3b (gitignore-architecture-refactor) du
rollout `devcontainer-tools-v2-migration`.

Entry point : `plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `STATUS.md` (Part 1 progress)
- `LOG.md` § P1-S2 (session 2 context)
- `sessions/part-1-session-3b-gitignore-architecture-refactor.md` (this spec)

Goal : isoler les règles gitignore en deux étages —
- `.devcontainer/.gitignore` shipped par le template (entrées scoped à
  `.devcontainer/`)
- `update_gitignore()` réduit aux entrées **root-scope** uniquement
Relocate LESSONS dans `.devcontainer/` avec symlink root pour visibilité.
Whitelister `.vscode/settings.json` pour que le skip-worktree de
post-start.sh fonctionne.

Session 3b scope :

1. **Créer `templates/v2/.devcontainer-gitignore.template`** (le naming
   `.template` évite que le `.gitignore` du repo template ignore son
   propre artefact). Contenu :

   ```
   # DevContainer (v2) — internes scoped à .devcontainer/

   # Runtime artefacts (auto-generated, per-rebuild)
   .env
   .configured-*
   logs/

   # Skill scratch dirs (keep dir via .keep, ignore content)
   pending/*
   !pending/.keep
   pr-drafts/*
   !pr-drafts/.keep
   research-bundles/*
   !research-bundles/.keep
   scan-deps/*
   !scan-deps/.keep

   # Firewall — local overrides, caches, test logs
   firewall/domains.local.txt
   firewall/policy.local.d/
   firewall/addons/__pycache__/
   tests/diagnose.log
   tests/diag-a2-*.log

   # Skills — local personal variants
   skills/**/*.local.skill.md
   skills/**/*.local/

   # claude-bridge sidecar runtime config (bootstrapped from .example.json)
   claude-bridge/config.json

   # Lessons — local / not-yet-generalisable (committed sibling is LESSONS.md)
   LESSONS.local.md
   ```

2. **Créer `templates/v2/LESSONS.md`** (baseline shipped, ~10 lignes) :

   ```markdown
   # LESSONS — project-wide patterns (committed)

   > Sister files : `LESSONS.local.md` (gitignored) for personal /
   > not-yet-generalisable lessons. Cross-project preferences live in
   > `~/.claude/memory/` (auto-memory).

   <!-- Entries : one bullet per lesson. Rule first, then *Why* and
        *How to apply* on the same or following line. -->

   - _No lessons yet._
   ```

3. **Modifier `install.sh`** :
   - `install_files()` (après les copies existantes, avant `success`) :
     ```bash
     cp "$TEMPLATE_DIR/.devcontainer-gitignore.template" "$DEST/.gitignore"
     [ -f "$DEST/LESSONS.md" ] || cp "$TEMPLATE_DIR/LESSONS.md" "$DEST/LESSONS.md"
     ```
     Le `[ -f ... ]` preserve les lessons accumulées sur ré-install.

   - **Nouvelle fonction** `link_lessons_root()` :
     ```bash
     link_lessons_root() {
         ln -sf ".devcontainer/LESSONS.md" "$TARGET_DIR/LESSONS.md"
         success "LESSONS.md → .devcontainer/LESSONS.md (symlink)"
     }
     ```
     Appelée dans `main()` après `set_exec_perms`, avant `write_v2_marker`.
     Pattern identique à `post-create.sh:36` pour CLAUDE.md.

   - **Réduire** `update_gitignore()` aux entrées root-scope uniquement :
     ```bash
     local entries=(
         "# DevContainer (v2) — root-scope"
         ".claude/"
         ".vscode/*"
         "!.vscode/settings.json"
         "!.vscode/extensions.json"
         ".env.dev"
         ".DS_Store"
         "firewall-blocks.*.log"
     )
     ```
     `.devcontainer/` entièrement géré par son propre `.gitignore` ;
     `LESSONS.local.md` aussi (dans `.devcontainer/.gitignore`).
     `.vscode/*` + whitelist `settings.json` + `extensions.json` débloque
     le `skip-worktree` de post-start.sh.

4. **Décider du contenu de `templates/v2/vscode-settings.json`**.
   Actuellement `{}`. Deux options :
   - Garder `{}` : le bind-mount existe, le `skip-worktree` masque les
     diffs. Suffit fonctionnellement. Recommandé si pas de baseline
     partagé à imposer.
   - Mettre une baseline minimale alignée sur `devcontainer.json:22-41`
     (extensions.autoUpdate=false, editor.formatOnSave=false, terminal
     defaultProfile=zsh). Plus opinionated mais explicite.

   Recommandation : **garder `{}`** pour cette session — pas dans le
   scope. Si on veut une baseline, c'est une décision UX séparée.

5. **Optionnel — déposer baseline `.vscode/settings.json` au root du
   projet** depuis install.sh. Idem § 4 : pas dans le scope si on garde
   `vscode-settings.json` à `{}`. Skip.

6. **Amender [CLAUDE.md § 8](CLAUDE.md)** (Self-Improvement Loop) :
   - `.devcontainer/LESSONS.md` (committed, root symlink for visibility)
   - `.devcontainer/LESSONS.local.md` (gitignored via `.devcontainer/.gitignore`)
   - Auto-memory `MEMORY.md` (cross-project, inchangé)

7. **Si le repo dogfood son propre `.devcontainer/`** :
   - Pose le symlink dogfood : `ln -sf .devcontainer/LESSONS.md LESSONS.md`
   - Si `.devcontainer/LESSONS.md` n'existe pas, le créer depuis le template
   - Si un `LESSONS.md` existait déjà au root (fichier réel), `git mv`
     vers `.devcontainer/LESSONS.md`, puis poser le symlink à sa place
   - Confirmer `git ls-files -s LESSONS.md` retourne mode `120000` au stage

Validation (manuelle, end of session) :

```bash
# 1. Le template a les nouveaux fichiers
ls templates/v2/.devcontainer-gitignore.template templates/v2/LESSONS.md

# 2. install.sh référence les nouveaux fichiers
grep -E "(.devcontainer-gitignore.template|LESSONS.md|link_lessons_root)" install.sh

# 3. update_gitignore() ne contient plus les entrées .devcontainer/*
! grep -E '"\.devcontainer/' install.sh \
  | grep -v '\.devcontainer/\.env\|TARGET_DIR\|DEST'
# (les seules occurrences acceptables sont des paths internes au script)

# 4. update_gitignore() entries cible root-scope uniquement
grep -A 10 "local entries=(" install.sh | head -15

# 5. Smoke install dans /tmp pour vérifier le résultat
TMPDIR=$(mktemp -d)
bash install.sh "$TMPDIR" <<< "
testproj
Test Proj


n
y
"
test -f "$TMPDIR/.devcontainer/.gitignore"           && echo "✓ .devcontainer/.gitignore shipped"
test -f "$TMPDIR/.devcontainer/LESSONS.md"           && echo "✓ .devcontainer/LESSONS.md shipped"
test -L "$TMPDIR/LESSONS.md"                         && echo "✓ root LESSONS.md is a symlink"
readlink "$TMPDIR/LESSONS.md" | grep -q "^\.devcontainer/LESSONS\.md$" && echo "✓ symlink target correct"
grep -q "^\.vscode/\*$" "$TMPDIR/.gitignore"         && echo "✓ root .gitignore has .vscode/*"
grep -q "^!\.vscode/settings\.json$" "$TMPDIR/.gitignore" && echo "✓ root .gitignore whitelists settings.json"
! grep -q "^\.devcontainer/logs/$" "$TMPDIR/.gitignore" && echo "✓ root .gitignore no longer duplicates .devcontainer/* entries"
rm -rf "$TMPDIR"
```

6. **git check-ignore sanity** depuis le dogfood :
```bash
git check-ignore -v .devcontainer/logs/foo.log    # → .devcontainer/.gitignore
git check-ignore -v .devcontainer/LESSONS.local.md # → .devcontainer/.gitignore
git check-ignore -v .claude/foo                   # → root .gitignore
git check-ignore -v .vscode/keybindings.json      # → root .gitignore (matched by .vscode/*)
! git check-ignore .vscode/settings.json          # → NOT ignored (whitelisted)
```

DoD at end of this session :
1. STATUS.md : ajouter row "3b — gitignore-architecture-refactor" entre
   les rows 3 et 4, flip 📋 → ✅, prompt link → —, bump "Delivered"
   counter (2/5 → 3/6 ou 3/7 selon où on en est avec S3).
2. LOG.md : append `## P1-S3b — gitignore-architecture-refactor`
   section avec files touched, rationale architectural, install smoke
   output, gotchas (en particulier si symlink LESSONS.md sur Windows
   nécessite des privs spéciaux — flagger).
3. ROADMAP.md : insérer row 3b dans la status table, mention dans
   "Part 1 — what's left", bump compteurs.
4. CLAUDE.md § 8 amendée (LESSONS location convention).
5. Propose UN commit (DO NOT commit sans user confirmation) :
   ```
   Split gitignore + relocate LESSONS into .devcontainer/

   - templates/v2/.devcontainer-gitignore.template : new shipped file
     for .devcontainer-scoped rules ; install.sh copies it to
     <target>/.devcontainer/.gitignore at install
   - install.sh update_gitignore() reduced to root-scope entries only
     (.claude/, .vscode/*, .env.dev, .DS_Store, firewall-blocks.*.log)
     with .vscode/settings.json + extensions.json whitelisted so the
     skip-worktree trick in post-start.sh actually has a tracked file
   - templates/v2/LESSONS.md : baseline shipped ; install.sh poses a
     symlink at <target>/LESSONS.md → .devcontainer/LESSONS.md (same
     pattern as CLAUDE.md → .devcontainer/claude/CLAUDE-dev.md)
   - CLAUDE.md § 8 : LESSONS convention amended (root → .devcontainer/
     with root symlink for visibility)
   ```
`````

## Next session

`part-1-session-4-fresh-install-test.md` — full Reopen-in-Container
cycle that validates S3 + S3b end-to-end on a sandbox project.

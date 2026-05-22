# Roadmap — devcontainer-tools v2

> Self-contained handover doc so work can resume in this repo without
> needing to cd back to the originating project (where the formal
> rollout plan lives at `plans/devcontainer-tools-v2-migration/`).
> Last sync'd : 2026-05-22.

## Status

**v2.0.0 in flight.** `install.sh` rewritten (796 → 456 LoC, 4 prompts,
2 placeholders). `templates/v2/` populated (~95 files, scrubbed of
project-specific identity). `update.sh` removed (v2 ships one installer
only). Image layer split landed (P1-S3) — base image now stable
across projects pinning the same `CLAUDE_CODE_VERSION`. Not yet
CHANGELOG'd, not yet released.

**Part 1 progress** : 3 / 6 delivered (S1 scope-audit, S2 install-redesign,
S3 firewall-layer-split). Next focus : **S4 fresh-install-test** (depends
on S3 ✅ + S3b) ; S3b gitignore-architecture-refactor still parallelisable.

| Phase | Status | What it ships |
|---|---|---|
| Part 1 / S1 — scope-audit | ✅ delivered | Frozen file list (84-95 files), templating model (shell expansion + 2 `{{...}}` placeholders), drop list |
| Part 1 / S2 — install-redesign | ✅ delivered | This commit batch : `install.sh` v2 + `templates/v2/` sync + all scrubs |
| Part 1 / S3 — firewall-layer-split | ✅ delivered | Moved 4 project-specific firewall COPYs (domains.txt, domains.local.txt.example, policy.d/, policy.local.d.example/) from `Dockerfile.base` to project layer (`Dockerfile` + `Dockerfile.php`) so `claude-devcontainer-base:${VERSION}` stays project-agnostic and truly shared. Mirror cp'd into dogfooding `.devcontainer/` (Dockerfile.base + Dockerfile ; no PHP dogfood). Grep verification : 5/5 gates pass. |
| Part 1 / S3b — gitignore-architecture-refactor | 📋 parallel | Split gitignore : ship `.devcontainer/.gitignore` for internal rules, slim `update_gitignore()` to root-scope only ; relocate `LESSONS.md` into `.devcontainer/` with root symlink (same pattern as `CLAUDE.md`) ; whitelist `.vscode/settings.json` for the `skip-worktree` trick. Independent of S3 — can run in parallel. |
| Part 1 / S4 — fresh-install-test | 📋 | Run installer against a sandbox project, full Reopen-in-Container cycle, validate lifecycle + firewall + sync-creds + sync-skills runtime, gitignore split + LESSONS symlink behave correctly |
| Part 1 / S5 — bump-changelog | 📋 | Write `CHANGELOG.md` v2.0.0 entry, document breaking drops, refresh root `README.md` for v2 flow, tag `v2.0.0` locally |
| Part 2 — 1.3 → 2.0 migration prompt | ⏸️ deferred | Paste-into-Claude session prompt that walks an existing 1.3 project through the upgrade (replaces the dropped `update.sh`) |

## Part 1 — what's left (~3-4 h total)

### Session 3 — firewall-layer-split — ✅ DELIVERED (2026-05-22)

Done : moved the 4 project-specific firewall COPYs out of `Dockerfile.base`
into the project layer. Base image now stable across projects. See
[LOG.md § P1-S3](plans/devcontainer-tools-v2-migration/LOG.md) and
[KNOWLEDGE.md § Image layer split](plans/devcontainer-tools-v2-migration/KNOWLEDGE.md#image-layer-split-p1-s3-2026-05-22)
for the rationale + verification. Migration recipe for already-deployed
v2-beta instances : see the session spec.

### Session 3b — gitignore-architecture-refactor (~45 min — 1 h)

Trois problèmes surfacés pendant fresh-install validation : (1)
`update_gitignore()` incomplet (~9 entrées manquantes), (2) `.vscode/`
full-ignore en conflit avec le `skip-worktree` de `post-start.sh`
(opération inopérante sur fichier non tracké), (3) `LESSONS.md`/
`LESSONS.local.md` au root, seule exception dans un écosystème AI sinon
contenu dans `.devcontainer/`.

Le fix : ship un `.devcontainer/.gitignore` shipped depuis le template
(scoped aux entrées internes), réduire `update_gitignore()` aux entrées
root-only, relocate LESSONS dans `.devcontainer/` avec symlink root
(même pattern que `CLAUDE.md` — `git ls-files -s` montre déjà mode 120000
sur `CLAUDE.md`, le symlink commit OK), whitelist `.vscode/settings.json`
+ `.vscode/extensions.json` au root gitignore.

Spec : [plans/devcontainer-tools-v2-migration/sessions/part-1-session-3b-gitignore-architecture-refactor.md](plans/devcontainer-tools-v2-migration/sessions/part-1-session-3b-gitignore-architecture-refactor.md).

Touches ~5 files (install.sh + 2 nouveaux fichiers shipped dans
templates/v2/ + CLAUDE.md amendement + optionnel mirror dogfood).
**Indépendante de session 3** — peut partir en parallèle.

### Session 4 — fresh-install-test (~2-3 h)

Full runbook : look for `HOWTO-test-install-host.md` in the originating
project's `plans/devcontainer-tools-v2-migration/` directory (if you
copied it over, it'll be at `docs/HOWTO-test-install-host.md` ; if not,
the workflow boils down to) :

1. Sandbox : `mkdir -p ~/sandbox/dctest-v2-$(date +%s) && cd $_`
2. Install : `bash /path/to/devcontainer-tools/install.sh $(pwd)`
3. Defaults wizard : Enter / Enter / Enter / volume name or `n` / Enter
4. `code .` → Reopen in Container
5. Inside : `claude --version`, `wtf foo`, `bash .devcontainer/test-firewall.sh`,
   inspect `~/.claude/settings.json` + `~/.claude/commands/`
6. Test the v1.3 abort path (plant a fake `.configured-setup VERSION="1.3.0"` in a second sandbox)
7. Capture timings + log highlights for CHANGELOG (session 4)

Key things to watch for :
- Base image build : ~5-8 min cold cache, ~10s warm
- Lifecycle phase logs in `.devcontainer/logs/<phase>-<ts>.log`
- Firewall up : `bash test-firewall.sh` from inside container
- Creds carry-over : if you used the same `CLAUDE_CREDS_VOLUME` as
  your daily devcontainer, OAuth should be transparent

### Session 5 — bump-changelog

1. **`CHANGELOG.md`** — write a v2.0.0 entry. Suggested sections :
   - **Breaking changes (drops)** :
     - `gh-secure/` (6 scripts) — superseded by `/prepare-pr` skill
     - `Dockerfile.node` — generic `Dockerfile` covers Node projects
     - `templates/master-review/` skill — superseded by `/prepare-pr`
     - `templates/KNOWLEDGE.md` (single file) — superseded by `knowledge/` dir (6 files)
     - `templates/test-db.php` — unused
     - `templates/gitignore-entries.txt` — install.sh embeds inline now
     - `update.sh` — full-resync script too fragile, replaced by Part 2 Claude prompt
   - **Renames** :
     - `templates/Dockerfile.custom` → `templates/v2/Dockerfile` (default project layer)
     - `templates/` → `templates/v2/` (versioned scheme, allows future flavours)
   - **New** :
     - 4-prompt wizard (down from 13)
     - Shell expansion (`${VAR:-default}`) replaces 11 sed placeholders ; only `{{PROJECT_ID}}` + `{{PROJECT_DISPLAY_NAME}}` survive
     - Detect-existing : v1.3 marker → abort with Part 2 pointer ; v2 → reinstall/abort
     - `Dockerfile.php` ships as a first-class variant (PROJECT_TYPE=php)
     - `claude-bridge/` sidecar (UniClaudeProxy for local Ollama)
     - `host-helpers/` (12 host-side utilities incl. `claude-switch`, `claude-bridge`, `verify-slim-base`, etc.)
     - Full `knowledge/` directory ships (6 files : INDEX, firewall, wtf, extension-points, docker-base-image, ollama-local)
     - 4 docs ship (README + RUNBOOK + SECURITY + RESEARCH)
     - 5 generic skills ship + `sync-skills.sh` (excludes `.local` skills)
     - 13 baseline L7 policies in `firewall/policy.d/`
     - Ollama online registry domains (`ollama.com`, `docs.ollama.com`, `registry.ollama.ai`) added to firewall allowlist for debug/model-query from inside container

2. **Root `README.md`** — refresh for v2 flow :
   - Quick start : `bash install.sh <project-dir>` → 4 prompts → done
   - Mention `templates/v2/` (and the variant pattern for future)
   - Document `TEMPLATE_VARIANT=` env var (currently only `v2` exists)
   - Drop any v1.3 / `update.sh` references
   - Add a "Migrating from v1.3" section pointing to Part 2 (when delivered)

3. **Bump `TEMPLATE_VERSION`** — already done in session 2 (`install.sh:6 → "2.0.0"`) ; just confirm.

4. **Tag** : `git tag v2.0.0` (don't push tag until validation passes).

## Part 2 — 1.3 → 2.0 migration prompt (deferred)

**Why deferred** : `install.sh v2` doesn't auto-migrate. v1.3 detect
path aborts with a pointer to "Part 2 migration prompt — not yet
specced".

**What it should be** : a paste-into-Claude session prompt that an
existing 1.3 project owner pastes into a fresh Claude Code session.
The prompt walks Claude through the upgrade : diff each file, reconcile
with the v2 baseline, ask the user for the project-specific bits
(`.env` values, custom firewall domains, `policy.local.d/` overrides),
preserve them across the upgrade. Human-in-the-loop validation per
reconciled file — no blind overwrites.

**Why a prompt, not a script** : the original v1.3 `update.sh` (full
resync with diff) was deemed too fragile for a major bump. Per-file
reconciliation with judgment calls fits Claude's strength.

**When to spec it** : after Part 1 ships (v2.0.0 tagged + a few new
projects installed via v2). Real upgrade requests will surface the
edge cases worth handling.

## Backlog (low priority, flagged during session 2)

- **`.devcontainer/tests/diag-*.sh`** — hardcoded default container
  name. Parameterise or drop (out-of-scope per SCOPE § "Out — debugging only").
- **`templates/v2/skills/prepare-research/templates/*.yaml`** — prompt
  content is in French (the skill `.md` is English). EN translation
  pass when touching that skill next.
- **`tests/diag-*.sh` paths** — they reference `/Volumes/Data/dev/...` in
  comments. Generic-ify or drop.

## Future ideas (not committed, not specced)

- **`templates/v3/`** — next major break. Triggers : v2 baseline gets
  too crufty, or a major Docker-Compose / DevContainer spec change.
- **Variant flavours under `templates/v2/`** — e.g. `templates/v2/minimal/`
  (no claude-bridge, no host-helpers) for lighter installs. Would need
  a 5th wizard prompt.
- **`sync-templates.sh`** maintainer helper — currently the refresh
  from upstream baseline is a manual procedure documented in the
  source project's `LOG.md`. If sync churn becomes painful, formalise.
- **CI smoke test** — GitHub Actions job that runs `install.sh` against
  a temp dir + asserts file count + exec perms + no leftover placeholders.

## How this file is maintained

- Lives in repo root of `devcontainer-tools/` (this file).
- Updated at the end of each delivered session (mirrors the LOG.md
  entries in the source project's rollout dir).
- Rsync'd from the in-container working copy at
  `<host-workspace>/devcontainer-tools/` to this repo via the
  procedure in the source project's `HOWTO-test-install-host.md`.

## Cross-references

- Originating rollout plan (in source project) :
  `plans/devcontainer-tools-v2-migration/`
  - `ROLLOUT.md` — strategic context, decisions
  - `STATUS.md` — actionable session table
  - `LOG.md` — append-only delivered-session journal
  - `SCOPE.md` — file inventory + templating model
  - `HOWTO-test-install-host.md` — session 3 runbook + container→host sync
  - `sessions/part-1-session-3-fresh-install-test.md` — session 3 paste-prompt

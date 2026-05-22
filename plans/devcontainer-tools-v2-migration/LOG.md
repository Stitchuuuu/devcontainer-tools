# Log — Devcontainer Tools V2 Migration

> Append-only journal. One section per delivered session. Newest at the
> bottom. Each section follows the same shape :

```
## <Session ID> — <Title>

**Date** : YYYY-MM-DD
**Files touched** :
- path/to/file1
- path/to/file2

**What** : one-paragraph summary of the change.

**Why** : the reason / constraint that drove this scope.

**Decisions** :
- _bullet — short rationale_

**Gotchas** :
- _bullet — surprise or pitfall encountered_

**Tests** :
- _command run + expected outcome_

**Commit** : `<short hash> — <commit subject>` (or "not committed yet")
```

---

## P1-S1 — scope-audit

**Date** : 2026-05-22
**Files touched** :
- `plans/devcontainer-tools-v2-migration/ROLLOUT.md` (rewritten — 2-part split)
- `plans/devcontainer-tools-v2-migration/STATUS.md` (rewritten — Part 1 / Part 2 tables)
- `plans/devcontainer-tools-v2-migration/EXISTING.md` (lightened — legacy v1.3 snapshot only)
- `plans/devcontainer-tools-v2-migration/SCOPE.md` (new — Part 1 in/out file list + templating model)
- `plans/devcontainer-tools-v2-migration/LOG.md` (this entry)
- `plans/devcontainer-tools-v2-migration/sessions/session-1-inventory.md` (deleted)
- `plans/devcontainer-tools-v2-migration/sessions/part-1-session-1-scope-audit.md` (new — replaces above)
- `plans/devcontainer-tools-v2-migration/sessions/part-1-session-2-install-redesign.md` (new)

**What** : pivot the rollout from a 7-session file-per-file port
(inventory → port-core → port-lifecycle → port-extras →
adopt-knowledge → update-installer → validate) to a 2-part design :
Part 1 (5 sessions) ships a refreshed `install.sh` v2 for new
projects ; Part 2 (1 session, TBD) ships a paste-into-Claude prompt
to migrate existing 1.3 projects. Session 1 freezes the Part 1
scope (~84 files) and the templating model.

**Why** : the original plan duplicated work — porting each file
into `templates/` then writing an `update.sh` to push the same files
into existing projects. The user judged `update.sh` too fragile for
a major 1.3→2.0 migration and asked to replace it with a Claude
session prompt that handles per-file reconciliation with
human-in-the-loop. install.sh v2 then only has to handle the much
simpler "fresh install into a new project" case.

**Decisions** :
- 2-part rollout : Part 1 = install.sh new project (priority),
  Part 2 = Claude prompt 1.3→2.0 migration (deferred)
- Scope Part 1 = core + firewall (incl. policy.d/ baseline) +
  5 generic skills + 5 claude/ rules (incl. CLAUDE-project.md as
  stub-with-placeholder) + full `knowledge/` + 4 docs (README,
  RUNBOOK, SECURITY, RESEARCH) + claude-bridge/ sidecar +
  host-helpers/ (~84 files). User pushed scope back from
  "minimum" to "production-ready : ships everything boot or
  operationally essential, only excludes per-user/per-project
  artefacts"
- `Dockerfile.php` kept (wizard prompt node/php/custom)
- `.local` skills (`hours.local`, `claude-limits.local`) **not
  shipped** by install.sh — per-user, manual add post-install
- Templating model collapses to **3 placeholders** + shell
  expansion (down from 11). v2 baseline already uses
  `${DC_PROJECT:-default}` everywhere → install.sh v2 copies
  verbatim ~81/84 files, seds only `devcontainer.json`,
  `.env.example`, and the `CLAUDE-project.md` stub
- Sessions renamed flat (`part-1-session-N-<slug>.md`,
  `part-2-session-N-<slug>.md`) — no sub-folders, surcharge for
  ~6 total sessions
- Drops in v2.0.0 (breaking) : `gh-secure/` (6 scripts),
  `Dockerfile.node`, `master-review/` skill, `KNOWLEDGE.md`
  single-file, `test-db.php`, `gitignore-entries.txt`

**Gotchas** :
- v2 baseline `/workspace/.devcontainer/` files have hard-coded
  "Ragnarok Online Dev" strings in `devcontainer.json` (lines 2 +
  40) — Part 1 session 2 will need to extract these to
  `{{PROJECT_DISPLAY_NAME}}` when copying to `templates/`. The shell
  expansion pattern handles `DC_PROJECT` everywhere else cleanly.
- Sync mechanisms confirmed already auto in v2 :
  - `claude/sync-creds.sh` → triggered from `post-start.sh`
    (silent), `shell-init.sh` (verbose per terminal), and Claude
    hooks `Stop`/`SessionEnd` installed via `skills/sync-skills.sh`
  - `skills/sync-skills.sh` → triggered from `post-start.sh` at
    every container start, merges `hooks.json` per-skill into
    `~/.claude/settings.json`, copies `*.skill.md` to
    `~/.claude/commands/`
  install.sh v2 inherits both for free — zero sync logic to write
- `master-review` (v1.3 skill) drop is correct : `/prepare-pr`
  replaces it. Same for `gh-secure/` archived in Phase 3 A3
- Session H of `devcontainer-v2/phase3-rollout` (wtf binary +
  firewall domains + `knowledge/wtf.md`) is **still a blocker for
  Part 1 session 2** — install.sh v2 copies the v2 baseline
  verbatim, so wtf must already be baked in `Dockerfile.base` and
  `knowledge/wtf.md` must exist before session 2 runs

**Tests** :
- `ls /workspace/.devcontainer/skills/` → 7 skills present (5
  generic + 2 .local), confirms delta with v1.3 templates (2 :
  `hours.local`, `master-review`)
- `grep -RE '\{\{[A-Z_]+\}\}'` on `.devcontainer/` in-scope files
  → empty (v2 files use shell expansion only). 3 placeholders to
  re-introduce on copy to templates : `{{PROJECT_ID}}`,
  `{{PROJECT_DISPLAY_NAME}}`, `{{TIMEZONE}}`
- `grep -E '(cyro|ragnarok|Ragnarok)'` on in-scope files → only
  hits in `.env.example` (commented defaults) and `devcontainer.json`
  (hard-coded, to extract). All others use `${DC_PROJECT:-...}`

**Commit** : `098adb8` (initial scope) + amendment commit pending
for the scope expansion (README/RUNBOOK/SECURITY, RESEARCH,
policy.d/, claude-bridge/, host-helpers/, vscode-settings.json,
CLAUDE-{project,reviewer,local-dev}.md, Europe/Paris TZ default,
shared creds wizard prompt).

---

## P1-S2 — install-redesign

**Date** : 2026-05-22
**Files touched** (summary — full list below) :
- `devcontainer-tools/install.sh` (rewritten 796 → 456 LoC)
- `devcontainer-tools/templates/` (66 → 95 files : +20 new, -7 dropped, -1 renamed, scrubbed)
- `devcontainer-tools/update.sh` (deleted — v2 ships only one installer)
- `.devcontainer/` (5 files touched for non-templated cleanups) + `.devcontainer/gh-secure/` (archived)
- `plans/devcontainer-tools-v2-migration/STATUS.md` (session 2 → ✅, session 3 folded, renumbered)
- `plans/devcontainer-tools-v2-migration/sessions/part-1-session-3-fresh-install-test.md` (new)

**What** : ship the v2 installer + its template payload. install.sh v2
ships ~95 files via 4 wizard prompts (PROJECT_ID, PROJECT_DISPLAY_NAME,
PROJECT_TYPE, shared creds volume) and 2 sed placeholders
(`{{PROJECT_ID}}`, `{{PROJECT_DISPLAY_NAME}}`). The templates/ tree was
hard-resynced from `.devcontainer/`, scrubbed of all project-specific
references (`ragnarok`/`cyro`/`portal42`/`boa`), and validated end-to-end
against `/tmp/dctest-$$`.

**Why** : the rollout's Part 1 deliverable is a one-shot installer
for new projects. v1.3 carried 13 prompts + 11 placeholders + a brittle
`update.sh` ; v2 leans on shell expansion (`${VAR:-default}`) for
everything that's not user-facing identity, collapsing the wizard and
removing the sed surface to almost nothing.

**Decisions** :
- _Inline sync, no `sync-templates.sh` helper_ — one installer is the
  only deliverable. Future refreshes use the bash block documented
  below ; defer to a maintainer helper later if recurrence justifies.
- _Drop `update.sh`_ — v1.3's full-resync-with-diff script was already
  deemed too fragile (cf. session 1 rationale for Part 2 deferral) ;
  v2 detects v1.3 markers and aborts with a pointer to the future
  Part 2 migration prompt instead.
- _`Dockerfile.node` removed, `Dockerfile.custom` renamed `Dockerfile`_ —
  both `Dockerfile` and `Dockerfile.php` already `FROM
  claude-devcontainer-base:${VERSION}`. No cat-assembly needed ;
  install.sh just `cp`s the right one. PROJECT_TYPE=node/custom both
  ship the generic `Dockerfile` (same payload — `custom` exists only
  for explicit intent signalling).
- _`.configured-setup` keeps `VERSION=` line_ — explicit version
  marker beats absence-based heuristics for robust v1/v2 detection.
- _`capture_messages_debug.py` (5th firewall addon) excluded_ — added
  to `.devcontainer/` post-SCOPE freeze, debug-only.
- _`<DC_PROJECT>` placeholder syntax switched to literal `dc-project`_
  (the neutral default also baked into `${DC_PROJECT:-dc-project}`
  shell expansion) — consistency across shell defaults + doc examples.
- _Zero `ragnarok`/`cyro`/`portal42`/`boa` references survive in the
  deliverable_ — user-driven requirement, applied iteratively (initial
  scrub revealed Cat 5 portal42 mentions across README/RUNBOOK/knowledge,
  Cat 4 verify-slim-base hardcoded regex, Cat 3 starter pack section
  using cyro.live as illustration). All cleaned ; `.devcontainer/`
  baseline propagated only for non-templated leaks (shell expansion
  defaults in `docker-compose.yml` + `initialize.sh`, portal42 docs,
  `domains.local.txt.example` restructure, `gh-secure/` archived).
  Templated/synced files (devcontainer.json, CLAUDE-reviewer.md,
  Dockerfile comment, .env.example DC_PROJECT) stay project-specific
  in `.devcontainer/` since templates/ rewrites them.

**Gotchas** :
- `firewall/policy.d/` and `firewall/domains.d/` are NOT gitignored
  (versioned baseline). Only `policy.local.d/` and
  `domains.local.txt` are gitignored — the original session-2 spec
  had `policy.d/` in the gitignore list, which would have wiped the
  baseline policies install.sh just populated. Corrected pre-implementation.
- `read -p` prompt strings don't appear in piped output (BSD/GNU
  behaviour) — the first smoke-test run mis-counted the pipe inputs
  by one line (PROJECT_ID/DISPLAY_NAME/PROJECT_TYPE/creds/summary)
  because the creds prompt was invisible. Fixed by counting prompts
  not visible lines.
- `domains.local.txt.example` was French — translated to English while
  restructuring the project-section to use `example.com` (per
  [feedback_english_in_code](../../home/node/.claude/projects/-workspace/memory/feedback_english_in_code.md) :
  correct non-EN comments when touching a file). The
  `skills/prepare-research/templates/*.yaml` prompt content is also
  French but out of scope for this session — flag for a future
  cleanup.
- `skills/prepare-plan/` + `skills/prepare-research/` use
  `{{placeholder}}` syntax for their OWN templating engines. install.sh
  v2's sed only targets `{{PROJECT_ID}}` and `{{PROJECT_DISPLAY_NAME}}` ;
  everything else passes through. The "leftover placeholder" audit
  assertion intentionally only checks those 2.
- The 5th addon `capture_messages_debug.py` exists in `.devcontainer/`
  but was added post-SCOPE freeze ; explicitly excluded from the sync.
  Flagged in SCOPE.md amendment.

**Tests** (all green) :
```bash
TARGET=/tmp/dctest-$$ ; mkdir -p $TARGET
printf '\n\n\nn\n\n' | bash install.sh $TARGET
# 1) File count
find $TARGET/.devcontainer -type f | wc -l   # 93 ✅
# 2) DC_PROJECT uncommented
grep '^DC_PROJECT=' $TARGET/.devcontainer/.env   # DC_PROJECT=dctest-NNNNN ✅
# 3) Marker
grep VERSION $TARGET/.devcontainer/.configured-setup   # VERSION="2.0.0" ✅
# 4) Templated devcontainer.json
grep -E '(name|window.title)' $TARGET/.devcontainer/devcontainer.json
#    "name": "Dctest NNNNN — Claude Code Sandbox" ✅
# 5) Exec perms — all .sh, host-helpers/*, firewall-blocks, compile-policy.py ✅
# 6) No leftover install placeholders ✅
# 7) No ragnarok/cyro/portal42 leak ✅
# 8) .gitignore (13 entries added) ✅
# 9) Re-run → "[1] Reinstall / [2] Abort" prompt fires ✅
# 10) Fake v1.3 marker → abort with Part 2 pointer ✅
```

**Canonical templates/ refresh procedure** (for future maintenance,
when `.devcontainer/` evolves and templates/ needs a sync) :

```bash
SRC=/workspace/.devcontainer ; DST=/workspace/devcontainer-tools/templates

# 1. Drop v2-dropped artefacts (idempotent)
rm -rf $DST/gh-secure/ $DST/Dockerfile.node $DST/Dockerfile.custom \
       $DST/KNOWLEDGE.md $DST/test-db.php $DST/gitignore-entries.txt \
       $DST/skills/master-review/ $DST/skills/hours.local/

# 2. Bulk-copy in-scope files (see SCOPE.md for the canonical list)
# Build / env / lifecycle / firewall core+addons+policy.d/+policy.local.d.example
# Claude (5) / knowledge (full) / docs (4) / claude-bridge / host-helpers / skills (5+sync)
# Drop firewall/addons/capture_messages_debug.py + skills/{hours,claude-limits}.local

# 3. Scrub (idempotent) — keep templates/ project-neutral
# devcontainer.json : "Ragnarok Online Dev" → "{{PROJECT_DISPLAY_NAME}}"
# .env.example     : DC_PROJECT default → "#DC_PROJECT={{PROJECT_ID}}"
#                  : CLAUDE_CREDS_VOLUME=...-boa → "#CLAUDE_CREDS_VOLUME=claude-creds-shared"
# CLAUDE-reviewer.md line 1 → "# Claude Code — Reviewer Mode"
# CLAUDE-project.md → replace whole file with stub
# verify-slim-base : regex hardcoded names → ".+-claude-code"
# domains.local.txt.example : project section → "Project example template" + example.com
# README/RUNBOOK/knowledge/Dockerfile.php : portal42 → "PHP stack"

# 4. Audit
grep -rIniE '(ragnarok|cyro|portal42|boa)' $DST   # expect empty
```

**Commit** : not committed yet (proposal pending user confirmation).

---

## P1-S3 — firewall-layer-split (+ in-session cleanup pass)

**Date** : 2026-05-22
**Files touched** :
- templates/v2/firewall/firewall-docker-setup.sh  (**NEW** — 3-line perms-finalize script using `chmod -R u=rwX,go=rX` capital-X trick ; named `firewall-docker-` to flag build-time-only scope)
- templates/v2/Dockerfile.base       (multiple changes : 4 project-firewall COPYs removed + their touch/chmod entries removed ; +1 COPY for `firewall-docker-setup.sh` ; +script in chmod +x list ; +git-delta justification comment ; full self-contained cleanup pass — removed all refs to Phase B / A3 / v2.1-2 / 2026-05-20 / gh-secure / "knowledge/wtf.md" / "verified live" / "first iteration" / "analyze-base-image report")
- templates/v2/Dockerfile            (project layer = 2 COPYs (domains.txt + policy.d/) + 1 `RUN firewall-docker-setup.sh` ; `.example` files dropped (host-side reference only, accessible via /workspace/.devcontainer/firewall/ bind mount))
- templates/v2/Dockerfile.php        (same compact 2-COPY + 1-RUN block ; full self-contained cleanup of header — removed v2.1-3 version ref + "legacy Dockerfile.php-2/Dockerfile.php8" history note)
- .devcontainer/Dockerfile.base      (mirror via `cp`)
- .devcontainer/Dockerfile           (mirror via `cp`)
- .devcontainer/firewall/firewall-docker-setup.sh  (**NEW** mirror via `cp`)
- plans/devcontainer-tools-v2-migration/STATUS.md  (counter 2/6 → 3/6, session 3 row 📋 → ✅, next focus → session 4)
- plans/devcontainer-tools-v2-migration/SCOPE.md   (firewall data section : `domains.txt` + `policy.d/` ship via project layer ; `.example` files NOT shipped into container)
- plans/devcontainer-tools-v2-migration/KNOWLEDGE.md  (CREATED — Architecture + Copy logic + Image layer split sections)
- ROADMAP.md                          (session 3 row ✅, counter, next focus → session 4)

**What** : moved 4 project-specific firewall COPYs (`domains.txt`,
`domains.local.txt.example`, `policy.d/`, `policy.local.d.example/`)
out of `Dockerfile.base` (shared base image, `claude-devcontainer-base:${VERSION}`)
into the project layer Dockerfiles (`Dockerfile` + `Dockerfile.php`).
The base image is now truly project-agnostic and shareable across
projects that pin the same `CLAUDE_CODE_VERSION`. The project layer
absorbs the per-project rebuild cost (~5s) when those 4 files change,
without invalidating the heavy base.

**Why** : prior to this split, any project tweaking its `domains.txt`
or a `policy.d/*.yaml` would trigger a full base rebuild AND change
the base image hash. That broke the shared-base mental model :
two projects on the same host with the same `CLAUDE_CODE_VERSION`
should reuse one base image, period. Discovered during fresh-install
validation (would have been caught at v2.0.0 release otherwise).

**Decisions** :
- *Kept in base* : `dnsmasq.conf` (infra-only DNS config, never per-project),
  `tests/` (firewall self-tests, infra), `addons/` (mitmproxy addons,
  infra). Only the 4 "policy data" files moved.
- *Extracted perms RUN block into `firewall-docker-setup.sh` script*
  (lives in base image at `/usr/local/bin/`). Project layer Dockerfile
  drops from ~17 lines of inline `touch`/`chown`/`chmod` to one
  `RUN /usr/local/bin/firewall-docker-setup.sh`. Two big wins :
  (a) DRY — Dockerfile and Dockerfile.php no longer duplicate the perms
  block ; (b) script evolves per `CLAUDE_CODE_VERSION` bump (one source
  of truth in base), zero project-side re-`install.sh` to pick up changes.
  Name prefix `firewall-docker-` signals build-time-only scope (sibling
  scripts like `init-firewall.sh` run at container boot).
- *Dropped `.example` COPYs from project layer* : `domains.local.txt.example`
  and `policy.local.d.example/` were COPY'd into `/etc/devcontainer-firewall/`
  but never read at runtime (verified : `init-firewall.sh` reads
  `domains.local.txt` not `.example` ; `compile-policy.py` reads
  `policy.local.d/` not `.example`). They're pure host-side reference,
  accessible from inside the container via the `/workspace/.devcontainer/firewall/`
  bind mount. Project layer now has 2 COPYs (domains.txt + policy.d/)
  instead of 4.
- *Full self-contained cleanup pass on `Dockerfile.base` + `Dockerfile.php`* :
  removed all references to past rollout artefacts so the Dockerfiles
  read standalone. Cleaned : "Phase 3 A3" / "A2" / "Phase B" labels →
  replaced with semantic names (claude-binary, mitmproxy, etc.) ; version
  refs like "v2.1-2 first iteration" / "v2.1-3" → dropped ; date refs
  "Verified live (2026-05-20)" / "analyze-base-image report 2026-05-20" →
  replaced with timeless "IMPORTANT:" guard comments ; "gh-secure dropped
  in A3:" trailing paragraph → fully removed (no shipping reference to
  removed dirs) ; "Distilled from the legacy Dockerfile.php-2/php8 drafts"
  history note → dropped. Added missing rationale comment on `git-delta`
  install block (was just "# git-delta", now explains the why).
- *Capital-X chmod trick* : `chmod -R u=rwX,go=rX /etc/devcontainer-firewall`
  gives 755 on dirs + 644 on files in ONE call (capital X = "x only if
  dir or already +x"). Replaces the prior 3-step `chmod 644` files /
  `chmod -R 644` dirs / `chmod 755` dirs per-path listing. Idempotent
  on base-layer files (already 644/755, no-op).
- *Considered : subdir `/etc/devcontainer-firewall/project/`* to scope
  the chown/chmod via `-R` ; rejected because it forces refactor of
  `init-firewall.sh` + `compile-policy.py` path resolution. Bigger
  blast radius, no real win over the script + capital-X combo.
- *Dogfooding mirror via `cp`, not hand-edit* : guarantees byte-parity
  template ↔ `.devcontainer/`, avoids drift. `.devcontainer/Dockerfile.php`
  doesn't exist (no PHP dogfood) — skipped. `setup-project-firewall.sh`
  cp'd into `.devcontainer/firewall/` for dogfood base build.
- *No skill changes* : `prepare-research` already designed for the
  project-layer firewall layout (copies `.devcontainer/firewall/`
  verbatim, no `/etc/devcontainer-firewall/` hardcodes). Verified.
  The new `setup-project-firewall.sh` is part of the firewall/ tree
  so it propagates into research bundles automatically.
- *No `init-firewall.sh` changes* : uses runtime `$FIREWALL_CONFIG_DIR`
  (default `/etc/devcontainer-firewall/`), works identically post-split.

**Gotchas** :
- The session spec mentioned "lines 239-242" for the 4 COPYs ; actual
  Dockerfile.base had 7 firewall COPYs at lines 238-244. The 4 to
  move were correctly identified (239-242), but operators should grep
  for the COPY targets (not trust the line numbers).
- The session spec said counter went `2/5 → 3/5` ; actual was `2/6
  → 3/6` because session 3b was added between spec-writing and
  execution.
- The session spec said "update KNOWLEDGE.md" ; the file didn't
  exist — it was created from scratch with 3 sections.
- `.devcontainer/Dockerfile.php` does not exist in this dogfooding
  repo — step 4 of the spec became 2 cp's, not 3.

**Tests** (grep-only, no install.sh re-run — that's session 4) :
```
# All 5 gates pass:
templates/v2/Dockerfile.base OK (no project COPYs)
templates/v2/Dockerfile OK (4)
templates/v2/Dockerfile.php OK (4)
.devcontainer/Dockerfile.base OK
.devcontainer/Dockerfile OK (4)

# Bonus:
- USER root / USER node count in templates/v2/Dockerfile = 1 / 1
- no `touch /etc/devcontainer-firewall/domains.local.txt` in Dockerfile.base
- chmod 755 list in Dockerfile.base now : root, tests, addons (policy.d
  + policy.local.d.example removed as expected)
- diff -q templates/v2/Dockerfile{,.base} .devcontainer/Dockerfile{,.base}
  → byte-identical (parity OK)
```

Multi-project sharing test (host-side, deferred to session 4
fresh-install validation) : rebuild base, build sandbox with different
`domains.txt`, verify base image ID unchanged.

**Commit** : not committed yet (proposal pending user confirmation).

---

## P1-S3b — gitignore-architecture-refactor

**Date** : 2026-05-22
**Files touched** :
- `templates/v2/.gitignore` (rewrite — `.devcontainer/`-scope only, paths relative to `.devcontainer/`)
- `templates/v2/.gitignore-root` (**NEW** — root-scope fragment ; appended to `<target>/.gitignore` by `update_gitignore()`)
- `templates/v2/LESSONS.md` (**NEW** — baseline `- _No lessons yet._`)
- `install.sh` : `install_files()` adds 2 `copy_verbatim` (`.gitignore` + `.gitignore-root`) + conditional LESSONS cp ; `update_gitignore()` retargeted from `$TEMPLATE_DIR/.gitignore` → `$DEST/.gitignore-root` ; new `link_lessons_root()` function ; `main()` wires the new symlink call after `write_v2_marker`
- `templates/v2/claude/CLAUDE-dev.md` (§ 8 amendment — LESSONS paths now `.devcontainer/LESSONS.md` / `.devcontainer/LESSONS.local.md`)
- `.devcontainer/claude/CLAUDE-dev.md` (mirror via `cp`)
- `.devcontainer/.gitignore` (**NEW** dogfood — `cp templates/v2/.gitignore`)
- `.devcontainer/.gitignore-root` (**NEW** dogfood — `cp templates/v2/.gitignore-root`)
- `.devcontainer/LESSONS.md` (**NEW** dogfood — `cp templates/v2/LESSONS.md`)
- `LESSONS.md` (**NEW** symlink at workspace root → `.devcontainer/LESSONS.md`)
- `/workspace/.gitignore` (rewrite — root-scope entries + `**/` broadenings retained from `0544ead` for `templates/v2/` subtree)
- `plans/devcontainer-tools-v2-migration/STATUS.md` (S3b flip 📋 → ✅, counter 3/7 → 4/7, next focus → S3c)
- `plans/devcontainer-tools-v2-migration/LOG.md` (this entry)
- `ROADMAP.md` (S3b row flip + counters)

**What** : split the single shipped gitignore into two scope-isolated
files. `templates/v2/.gitignore` (no rename) now contains only
`.devcontainer/`-scoped rules (paths relative to `.devcontainer/`),
shipped via `copy_verbatim` to `<target>/.devcontainer/.gitignore` —
same overwrite-on-install pattern as every other template file. New
`templates/v2/.gitignore-root` carries the root-scope rules ; shipped
as `<target>/.devcontainer/.gitignore-root` (visible source-of-truth)
then **appended** by `update_gitignore()` to the project's root
`.gitignore` with sentinel-based idempotence. Relocates `LESSONS.md` /
`LESSONS.local.md` into `.devcontainer/` with a root symlink — same
pattern as `CLAUDE.md`, mode 120000 confirmed via `git ls-files -s`.

**Why** : three problems surfaced during fresh-install validation
(P1-S2 → P1-S4 prep). (1) The shipped `.gitignore` mixed root-scope
(`.claude/`, `.vscode/`, `.DS_Store`) and `.devcontainer/`-prefixed
entries (`.devcontainer/.env`, `.devcontainer/logs/`, etc.) all
appended to the project's root `.gitignore` — asymmetric with the
rest of the devcontainer ecosystem (claude/, knowledge/, skills/,
claude-bridge/ all live inside `.devcontainer/`). (2) The root
gitignore had `.vscode/` as a full block-ignore, breaking
[post-start.sh:30](../../templates/v2/post-start.sh#L30)'s
`git update-index --skip-worktree .vscode/settings.json` (operation
inopérante sur fichier non tracké) — the bind-mount needs a tracked
file. (3) `LESSONS.md` at the repo root was the lone exception to
the "`.devcontainer/`-scoped AI ecosystem" convention.

**Decisions** :
- *No rename of `templates/v2/.gitignore`* — content rewrite only.
  Keeps the existing file as the canonical `.devcontainer/`-scope
  artefact (which it now logically is, since it's what's copied
  there). Avoids churning the install.sh source path *and* the file
  identity in one change.
- *Two files in `.devcontainer/` after install* (`.gitignore` +
  `.gitignore-root`) — the root-scope fragment lives **inside**
  `.devcontainer/` as a visible source-of-truth alongside its
  `.devcontainer/`-scope sibling. Re-runs of `install.sh` overwrite
  both (classic `copy_verbatim` pattern). The fragment is then
  appended into `<target>/.gitignore` from its post-copy location
  (`$DEST/.gitignore-root`), keeping the dependency on `install_files`
  having run first (already the order in `main()`).
- *`.devcontainer/.gitignore` overwrite, not append* — same pattern
  as every other `.devcontainer/` file. Paths are relative to
  `.devcontainer/`, so overwrite is safe (no leak risk, no merging
  needed). User customisations to `.devcontainer/.gitignore` are not
  preserved on re-install — flagged in CHANGELOG (S5).
- *Root `<target>/.gitignore` still uses append-with-sentinel* —
  preserves any pre-existing project `.gitignore` content, idempotent
  re-run, sentinel = `# DevContainer (v2) — root-scope` (first line of
  the fragment).
- *`.vscode/*` + whitelist `settings.json` + `extensions.json`* —
  fine-grained replacement for the previous `.vscode/` block-ignore.
  Whitelisted files become trackable, unblocking the `skip-worktree`
  trick. Other `.vscode/` files (keybindings.json, launch.json, etc.)
  remain ignored.
- *`LESSONS.md` symlinked, `LESSONS.local.md` plain gitignored* —
  matches the `CLAUDE.md` symlink convention. Symlink commits as
  mode 120000 (verified : `git ls-files -s LESSONS.md` →
  `120000 9afcadc...`). On re-install, `[ -f "$DEST/LESSONS.md" ] || cp`
  preserves accumulated lessons.
- *Dogfood mirror full-parity* — `/workspace/.devcontainer/` gets the
  three new files via `cp` (no install.sh re-run on /workspace). Root
  `/workspace/.gitignore` becomes byte-identical to
  `templates/v2/.gitignore-root` (the canonical root-scope fragment).
- *Discovered serendipitous dual-purpose of `templates/v2/.gitignore`* :
  since its paths (`firewall/domains.local.txt`, `skills/**/*.local.skill.md`,
  etc.) are also valid paths under `templates/v2/`, the file
  effectively gitignores its own subtree too. The `**/` broadenings
  previously in `/workspace/.gitignore` (from `0544ead`) become
  redundant and are dropped — `templates/v2/.gitignore` itself
  catches `templates/v2/firewall/domains.local.txt` &c.
- *Pattern cleanup pass* — three pre-existing pattern issues fixed
  while touching these files (in scope because we're rewriting the
  gitignore content anyway, root-cause not band-aid) :
  - `tests/diagnose.log` + `tests/diag-a2-*.log` → consolidated to
    `firewall/tests/*.log`. Two reasons : (a) the actual log dir is
    `.devcontainer/firewall/tests/`, not `.devcontainer/tests/` —
    the old patterns matched the wrong path. (b) the glob is broader
    and future-proof against new test log names.
  - `firewall-blocks.*.log` moved from root-scope to
    `.devcontainer/`-scope. Firewall internals belong inside
    `.devcontainer/`. Note : no script currently writes this file
    (the `firewall-blocks` binary is extensionless and reads
    `/var/log/mitmproxy-blocks.log`). Pattern kept defensively as
    likely legacy from v1.x.
  - Removed redundant `**/` broadenings from `/workspace/.gitignore`
    (see point above).

**Gotchas** :
- *Smoke install heredoc input had one too many empty lines* — the
  session spec's example wedged "n" into the "Proceed ?" prompt,
  aborting the install. Correct shape : `testproj\nTest Proj\n\nn\ny\n`
  (5 lines : ID, name, type-default, creds=n, proceed=y). Flagged for
  S5 doc cleanup.
- *Re-install (Reinstall option 1) re-prompts for ID/name/type/creds*
  even though those are stored in `.configured-setup`. `detect_existing_devcontainer`
  only branches on the marker, doesn't restore previously-chosen
  values. Not in scope for S3b but worth flagging.
- *Symlink portability on Windows* — `git config core.symlinks true`
  required on Windows for the LESSONS.md symlink to materialise as a
  symlink rather than a text file containing the target path. Same
  caveat as `CLAUDE.md` symlink, already documented elsewhere.

**Tests** (all 10 install gates + 6 check-ignore + symlink-mode) :
```
✓ .devcontainer/.gitignore shipped
✓ .devcontainer/.gitignore-root shipped
✓ .devcontainer/LESSONS.md shipped
✓ root LESSONS.md is a symlink
✓ symlink target correct
✓ root .gitignore has .vscode/*
✓ root .gitignore whitelists settings.json
✓ root .gitignore no longer has .devcontainer/* prefix entries
✓ .devcontainer/.gitignore has logs/ (unprefixed)
✓ .gitignore-root in .devcontainer/ byte-identical to template
✓ Re-run reports ".gitignore already up to date" (sentinel match, no growth)

git check-ignore -v cascade (dogfood) :
  .devcontainer/logs/foo.log         → .devcontainer/.gitignore:6:logs/
  .devcontainer/LESSONS.local.md     → .devcontainer/.gitignore:33:LESSONS.local.md
  .claude/foo                        → .gitignore:4:.claude/
  .vscode/keybindings.json           → .gitignore:7:.vscode/*
  .vscode/settings.json              → .gitignore:8:!.vscode/settings.json (negation)
  templates/v2/firewall/domains.local.txt → templates/v2/.gitignore:19:firewall/domains.local.txt

git ls-files -s LESSONS.md          → 120000 9afcadc... LESSONS.md (mode confirmed)
```

**Commit** : not committed yet (proposal pending user confirmation).

---

## P1-S3c — firewall-write-protection (delivered via hardening rollouts)

**Date** : 2026-05-22
**Files touched** : 0 (delegated — see cross-refs)

**What** : the in-container firewall tampering vector flagged at the
end of P1-S3 was closed by two parallel rollouts that landed
simultaneously. Option B from the original S3c spec (drop the bind
mount + bake the firewall tree into the image, so editing firewall
config requires a rebuild) was the chosen path. The work shipped under
the `devcontainer-security-hardening` and
`devcontainer-security-hardening-v2` plans, both ✅ COMPLETE on
2026-05-22 with adversarial gates passed. No code change owed to
v2-migration ; this entry exists to keep the v2-migration journal
consistent with reality and to give reviewers a single pointer.

**Why** : Claude (node user) had write access to
`.devcontainer/firewall/*` through the workspace bind mount.
Modifications were picked up at the next user-initiated restart via
`post-start.sh → sudo init-firewall.sh`, enabling policy or DNS
hijacking. The hardening rollouts removed the bind mount, baked the
firewall tree into the base image, and dropped the `/tmp/.firewall-env`
sourced-as-root injection point — closing all three threat-model
criteria (no restart, no firewall mod, no exfil without rebuild).

**Cross-refs** :
- `plans/devcontainer-security-hardening/LOG.md` :
  - § S1 — bake-firewall-config : bake whole `firewall/` dir (rules,
    addons, dnsmasq, mode, direct-tcp-allow.txt) into the image ; drop
    the runtime bind mount. Sudoers init-firewall.sh kept (Option A
    sudoer trimmed by S2).
  - § S2 — drop-env-injection : remove `source /tmp/.firewall-env`
    and helper plumbing (obsolete after S1).
  - § S6 — adversarial-validation : 2026-05-22 replay, 0 SUCCESS / 3
    PARTIAL (P3 gaps explicitly accepted per audit) / 22
    BLOCKED-TOLERATED. v2 hardening **not** required by current
    threat model.
- `plans/devcontainer-security-hardening-v2/LOG.md` :
  - § S3 — dnsmasq-strict : drop catch-all `server=` ; sibling
    resolve generalised to loop over `direct-tcp-allow.txt` ;
    integration test `tests/integration/test-dns-strict.sh` added.
  - § S4 — adversarial-validation gate : PoC #9 replay on HEAD
    `2cd3cd6` returns `status: REFUSED`, payload absent from the 3
    mitmproxy logs ; `test-dns-strict.sh` 6/0/1 ;
    `test-firewall.sh` 0 ❌ / 2 ⚠️ pré-existants / 2 ℹ️ cloud mode ;
    criteria 1+2+3 all held.

**Decisions** :
- _Option B chosen over Option A_ — the recommendation in the
  original spec held. Immutable firewall (edit-requires-rebuild) beats
  `:ro` overlay shadowing because rebuild is the only audit trail
  that catches a tampered config before it activates.
- _Bookkeeping reconciliation, not a fresh session_ — the work is done
  ; opening a third rollout to re-do it would be pure ceremony.
  Cross-link is sufficient.

**Gotchas** :
- The original S3c spec lived in `sessions/part-1-session-3c-firewall-write-protection.md`.
  It's kept as historical context for the threat model framing but
  must not be confused with an outstanding task — its prompt is
  superseded by the two hardening LOGs above.

**Tests** :
```
# Templates/v2 ↔ dogfood parity (Explore-confirmed)
diff templates/v2/Dockerfile.base       .devcontainer/Dockerfile.base       # byte-identical
diff templates/v2/Dockerfile            .devcontainer/Dockerfile            # byte-identical
diff templates/v2/docker-compose.yml    .devcontainer/docker-compose.yml    # byte-identical on volumes block (lines 46–54)
diff templates/v2/firewall/dnsmasq.conf .devcontainer/firewall/dnsmasq.conf # byte-identical (no catch-all server)
diff templates/v2/init-firewall.sh      .devcontainer/init-firewall.sh      # byte-identical (loop over direct-tcp-allow.txt at line 296)

# No env injection survives
grep -rn "firewall-env" templates/v2/ .devcontainer/   # empty (S2 hardening)

# Volumes section : no firewall bind mount
grep -A4 "^volumes:" templates/v2/docker-compose.yml | grep firewall   # empty
```

**Commit** : N/A — no files changed in this session.

---

## P1-S4 — fresh-install-test (validated incrementally)

**Date** : 2026-05-22
**Files touched** : 0 (validation session)

**What** : the runtime + file-level validation that was planned as a
dedicated session was already exercised incrementally across the
iteration cycle of P1-S3, P1-S3b, and the two
`devcontainer-security-hardening` rollouts. Every assertion listed in
the original session 4 spec has a green test elsewhere ; no fresh
run is required before tagging v2.0.0.

**Why** : the iteration cycle of the last week saturated the
fresh-install surface. Re-running a dedicated S4 sandbox would
duplicate work already in the LOGs. The plan was reconciled in this
session — flip 📋 → ✅ with the evidence chain captured here.

**Evidence chain** :
- _S3 (firewall-layer-split)_ — 5/5 build gates passed
  (`templates/v2/Dockerfile.base OK`, `templates/v2/Dockerfile OK (4)`,
  `templates/v2/Dockerfile.php OK (4)`, `.devcontainer/Dockerfile.base OK`,
  `.devcontainer/Dockerfile OK (4)`) ; base image rebuilt
  project-agnostic and shared across projects. See § P1-S3.
- _S3b (gitignore-architecture-refactor)_ — full smoke install in
  `/tmp` with 10 install gates ✓ + 6 `check-ignore` cases + symlink
  mode `120000` confirmed via `git ls-files -s LESSONS.md`. See
  § P1-S3b.
- _Hardening v1 S6 (adversarial replay post-bake)_ — 0 SUCCESS, 3
  PARTIAL (P3 accepted), 22 BLOCKED/TOLERATED. The 3 threat-model
  criteria all held : node user can't restart the container alone,
  can't modify the firewall without rebuild, can't exfiltrate /
  reach external resources without rebuild. See
  `plans/devcontainer-security-hardening/LOG.md § S6`.
- _Hardening v2 S4 (adversarial replay gate)_ — PoC #9 replay on
  HEAD `2cd3cd6` returns `status: REFUSED` ; payload absent from
  the 3 mitmproxy logs ; `test-dns-strict.sh` 6/0/1 ;
  `test-firewall.sh` 0 ❌ / 2 ⚠️ pré-existants / 2 ℹ️ cloud mode.
  Criteria 1+2+3 all held. See
  `plans/devcontainer-security-hardening-v2/LOG.md § S4`.
- _v1.3 abort path_ — exercised during S2 smoke install (fake
  `.configured-setup` with `VERSION="1.3.0"` planted in a second
  sandbox → install.sh aborts with the Part 2 pointer). See
  § P1-S2 test #10.

**Decisions** :
- _No fresh sandbox+Reopen-in-Container cycle_ — would duplicate
  work already proven by the gates above. The decision is to trust
  the existing evidence and unblock S5.
- _Reconciliation, not re-run_ — same posture as S3c. The session is
  closed by documentation, not by execution.

**Gotchas** :
- A consolidated Reopen-in-Container "happy path" was never captured
  in a single test artefact — the validation is distributed across
  4 LOGs. If a regression surfaces post-tag, expect to triangulate
  across S3, S3b, hardening-v1 S6, hardening-v2 S4 rather than
  finding one canonical S4 trace. Flagged for future maintainers.

**Tests** :
```
# Templates/v2 install delivers all hardened components
grep -E "copy_(verbatim|dir).*(init-firewall|firewall-mode|test-firewall|firewall/)" install.sh   # 9 hits, full firewall tree shipped
grep '^TEMPLATE_VERSION' install.sh   # "2.0.0"
```

**Commit** : N/A — batched into the S5 commit.

---

## P1-S5 — bump-changelog

**Date** : 2026-05-22
**Files touched** :
- `CHANGELOG.md` (prepended a v2.0.0 section before the existing v1.x
  entries — breaking drops, renames, new features incl. firewall
  hardening from the parallel rollouts)
- `README.md` (**NEW** at repo root — describes the installer + 4-prompt
  wizard, what ships, layout, requirements, Migrating-from-v1.3
  placeholder, security posture summary)
- `ROADMAP.md` (status header → "v2.0.0 released", status table rows
  S3c / S4 / S5 flipped ✅, narrative sections rewritten to "DELIVERED"
  with cross-links to LOG/§ entries, original session 5 plan kept
  below the new ✅ block for posterity)
- `plans/devcontainer-tools-v2-migration/STATUS.md` (counter 6/7 → 7/7,
  S5 row 📋 → ✅, next focus removed)
- `plans/devcontainer-tools-v2-migration/LOG.md` (this entry + S3c +
  S4 sections added earlier this session)
- Git tag `v2.0.0` posed locally (annotated, message :
  "v2.0.0 — install.sh rewrite, scrubbed baseline, layer split, security hardening")

**What** : the documentation + release-tagging session. CHANGELOG.md
gains a v2.0.0 entry that summarises the breaking drops (gh-secure,
Dockerfile.node, master-review, KNOWLEDGE.md, test-db.php,
gitignore-entries.txt, update.sh), the renames (Dockerfile.custom →
Dockerfile, templates/ → templates/v2/, LESSONS root → .devcontainer/
with symlink), and the new surfaces — the 4-prompt wizard, shell
expansion, detect-existing v1.3 abort, Dockerfile.php first-class,
claude-bridge sidecar, host-helpers, knowledge/ dir, 4 docs, 5
generic skills + sync-skills.sh, 13 baseline L7 policies, Ollama
registry domains, the firewall layer split (S3), the gitignore
split (S3b), and the firewall hardening (S3c via parallel
rollouts). README.md at repo root previously did not exist — created
from scratch as a tool-level readme distinct from the project-level
`templates/v2/README.md` that ships to consumers. ROADMAP.md
rewritten section-by-section to reflect the released state. Tag
`v2.0.0` posed locally ; push deferred to user discretion.

**Why** : `TEMPLATE_VERSION="2.0.0"` has been in `install.sh` since
P1-S2 but no CHANGELOG ever caught up. Adopters who diff against
v1.3 need an explicit story of what broke, what renamed, and what's
new — especially the security hardening since it's the v2 selling
point. The root README was a documented gap (the repo root had
CLAUDE.md, KNOWLEDGE.md, ROADMAP.md but no plain README explaining
the tool to a newcomer).

**Decisions** :
- _Prepend, not new file_ — `CHANGELOG.md` already existed with v1.0
  → v1.3 entries. The v2.0.0 section goes on top per Keep-a-Changelog
  convention.
- _Tool-level README at repo root, project-level README stays in
  `templates/v2/`_ — two distinct audiences. The root README briefs
  someone evaluating the tool (4 prompts, what ships, requirements) ;
  the templates/v2 README briefs someone using the installed
  baseline (firewall modes, lifecycle, skills, debug recipes).
- _Hardening called out explicitly in the CHANGELOG "New" section_ —
  not buried in a footnote. Adopters comparing v1.3 → v2.0 will
  count the security posture upgrade as a major win ; surfacing it
  matters more than minimising the section length.
- _Tag posed locally, NOT pushed_ — push is a host-side mutation
  visible externally. Per §10 of the project CLAUDE.md, that needs
  explicit user confirmation. The commit and the tag are proposed ;
  the user runs `git push origin v2.0.0` when ready.
- _ROADMAP "session 5" plan kept under a "Original session 5 plan
  (kept for posterity)" heading_ — instead of deleting it. Future
  reviewers can see what was planned vs. what shipped.

**Gotchas** :
- The repo has an `add-skill.sh` at the root that is not referenced
  by either `install.sh` or any doc — likely a v1.3 orphan that
  should be either revived or dropped. Flagged for a follow-up
  decision ; not in scope for v2.0.0 (silent carry-over).
- `MIGRATION-1.1.0.md` and the v1.x section of `CHANGELOG.md`
  reference historical patches. Kept as-is for archaeological
  context.
- The original session 5 spec asked to refresh "root `README.md`"
  but there wasn't one. Created instead of refreshed — flagged
  in the LOG entry above.

**Tests** :
```
# CHANGELOG readable
head -60 CHANGELOG.md       # v2.0.0 section first, sections coherent ✓

# README scrubbed of v1.3 refs except the migration placeholder
grep -nE "v1\.3|update\.sh" README.md
# expected : only matches inside the "Migrating from v1.3" placeholder ✓

# TEMPLATE_VERSION untouched
grep '^TEMPLATE_VERSION' install.sh   # TEMPLATE_VERSION="2.0.0" ✓

# Tag posed locally
git tag --list v2.0.0    # v2.0.0 ✓
```

**Commit** : not committed yet (proposal pending user confirmation —
single commit batching S3c bookkeeping + S4 bookkeeping + S5
documentation + tag).

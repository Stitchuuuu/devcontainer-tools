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

# Roadmap — devcontainer-tools v2

> Self-contained handover doc so work can resume in this repo without
> needing to cd back to the originating project (where the formal
> rollout plan lives at `plans/devcontainer-tools-v2-migration/`).
> Last sync'd : 2026-05-22.

## Status

**v2.0.0 released** (2026-05-22). `install.sh` rewritten (796 → 456 LoC,
4 prompts, 2 placeholders). `templates/v2/` populated (~95 files,
scrubbed of project-specific identity). `update.sh` removed (v2
ships one installer only). Image layer split landed (P1-S3) — base
image stable across projects pinning the same `CLAUDE_CODE_VERSION`.
Security hardening (firewall bake-in + bind mount drop + dnsmasq
strict) landed via the parallel `devcontainer-security-hardening`
v1 + v2 rollouts, closing the S3c threat. CHANGELOG written, README
refreshed, tag `v2.0.0` posed locally (push at user's discretion).

**Part 1 progress** : 7 / 7 delivered (S1 scope-audit, S2
install-redesign, S3 firewall-layer-split, S3b
gitignore-architecture-refactor, S3c firewall-write-protection [via
hardening rollouts], S4 fresh-install-test [validated incrementally],
S5 bump-changelog).

| Phase | Status | What it ships |
|---|---|---|
| Part 1 / S1 — scope-audit | ✅ delivered | Frozen file list (84-95 files), templating model (shell expansion + 2 `{{...}}` placeholders), drop list |
| Part 1 / S2 — install-redesign | ✅ delivered | This commit batch : `install.sh` v2 + `templates/v2/` sync + all scrubs |
| Part 1 / S3 — firewall-layer-split | ✅ delivered | Moved 4 project-specific firewall COPYs (domains.txt, domains.local.txt.example, policy.d/, policy.local.d.example/) from `Dockerfile.base` to project layer (`Dockerfile` + `Dockerfile.php`) so `claude-devcontainer-base:${VERSION}` stays project-agnostic and truly shared. Mirror cp'd into dogfooding `.devcontainer/` (Dockerfile.base + Dockerfile ; no PHP dogfood). Grep verification : 5/5 gates pass. |
| Part 1 / S3b — gitignore-architecture-refactor | ✅ delivered | Split gitignore : `templates/v2/.gitignore` rewritten to `.devcontainer/`-scope only (shipped via `copy_verbatim` to `<target>/.devcontainer/.gitignore`) + new `templates/v2/.gitignore-root` for root-scope (shipped to `<target>/.devcontainer/.gitignore-root`, appended to `<target>/.gitignore` by `update_gitignore()`). Relocated `LESSONS.md` into `.devcontainer/` with root symlink (mode 120000, same pattern as `CLAUDE.md`). Whitelisted `.vscode/settings.json` + `extensions.json` so `post-start.sh`'s `skip-worktree` trick has a tracked file. Dogfood mirror applied. |
| Part 1 / S3c — firewall-write-protection (security) | ✅ delivered (via hardening rollouts) | Option B (drop bind mount + bake firewall into image) shipped under `devcontainer-security-hardening` v1 (S1 bake + drop mount, S2 drop env injection, S6 adversarial gate) and `devcontainer-security-hardening-v2` (S3 dnsmasq strict, S4 PoC #9 replay gate). Templates/v2 ↔ dogfood `.devcontainer/` byte-identical on all security-critical paths. |
| Part 1 / S4 — fresh-install-test | ✅ delivered (validated incrementally) | Runtime + file-level validation distributed across S3 (5/5 build gates), S3b (10 install gates + 6 check-ignore + symlink mode 120000), hardening v1 S6 (adversarial replay), hardening v2 S4 (PoC #9 replay on HEAD `2cd3cd6`). v1.3 abort path exercised during S2 smoke install. |
| Part 1 / S5 — bump-changelog | ✅ delivered | CHANGELOG.md v2.0.0 entry (breaking drops, renames, new features incl. hardening), root `README.md` created for the v2 install flow, `TEMPLATE_VERSION` confirmed at `"2.0.0"`, tag `v2.0.0` posed locally. Push deferred to user. |
| Part 2 — 1.3 → 2.0 migration prompt | ⏸️ deferred | Paste-into-Claude session prompt that walks an existing 1.3 project through the upgrade (replaces the dropped `update.sh`) |

## Part 1 — what's left (~5-7 h total)

### Session 3 — firewall-layer-split — ✅ DELIVERED (2026-05-22)

Done : moved the 4 project-specific firewall COPYs out of `Dockerfile.base`
into the project layer. Base image now stable across projects. See
[LOG.md § P1-S3](plans/devcontainer-tools-v2-migration/LOG.md) and
[KNOWLEDGE.md § Image layer split](plans/devcontainer-tools-v2-migration/KNOWLEDGE.md#image-layer-split-p1-s3-2026-05-22)
for the rationale + verification. Migration recipe for already-deployed
v2-beta instances : see the session spec.

### Session 3c — firewall-write-protection — ✅ DELIVERED (2026-05-22, via hardening rollouts)

Done : option B (drop bind mount + bake firewall into base image)
shipped under the two parallel `devcontainer-security-hardening`
rollouts. Both ✅ COMPLETE with adversarial gates passed.

- **Hardening v1** (4/4 essential sessions delivered) :
  [`plans/devcontainer-security-hardening/`](plans/devcontainer-security-hardening/)
  - S1 bake-firewall-config — `firewall/` baked into base image,
    bind mount dropped from docker-compose
  - S2 drop-env-injection — `/tmp/.firewall-env` sudoer-context import
    removed
  - S6 adversarial-validation gate — 0 SUCCESS, 3 PARTIAL (P3
    accepted), 22 BLOCKED/TOLERATED ; threat-model criteria 1+2+3
    all held
- **Hardening v2** (4/4 sessions delivered) :
  [`plans/devcontainer-security-hardening-v2/`](plans/devcontainer-security-hardening-v2/)
  - S3 dnsmasq-strict — catch-all `server=` dropped, sibling resolve
    generalised over `direct-tcp-allow.txt`
  - S4 adversarial-validation gate — PoC #9 replay on HEAD `2cd3cd6`
    returns `status: REFUSED`, payload absent from mitmproxy logs

Templates/v2 ↔ dogfood `.devcontainer/` byte-identical on all
security-critical files (commits `231d3ec`, `cb0301f`, `2cd3cd6`
touched both paths). New adopters inherit via `install.sh`.

### Session 3b — gitignore-architecture-refactor — ✅ DELIVERED (2026-05-22)

Done : `templates/v2/.gitignore` rewritten to `.devcontainer/`-scope only
(paths relative to `.devcontainer/`) and shipped via `copy_verbatim` to
`<target>/.devcontainer/.gitignore`. New `templates/v2/.gitignore-root`
carries root-scope rules, shipped as `<target>/.devcontainer/.gitignore-root`
(visible source-of-truth) and appended by `update_gitignore()` to
`<target>/.gitignore` (sentinel-based idempotence). `LESSONS.md` /
`LESSONS.local.md` relocated into `.devcontainer/` with root symlink
(`git ls-files -s LESSONS.md` → mode 120000 verified). `.vscode/settings.json`
+ `extensions.json` whitelisted so `post-start.sh`'s `skip-worktree`
trick has tracked files. Dogfood mirror applied. CLAUDE.md § 8 amended
(template + dogfood).

See [LOG.md § P1-S3b](plans/devcontainer-tools-v2-migration/LOG.md) for
the full file-touched list, decisions, gotchas, and test output.

### Session 4 — fresh-install-test — ✅ DELIVERED (2026-05-22, validated incrementally)

Done : the runtime + file-level validation was exercised
incrementally across the iteration cycle rather than as a single
dedicated session. Every assertion in the original spec has a green
test elsewhere :

- _S3 (firewall-layer-split)_ — 5/5 build gates passed, base image
  rebuilt project-agnostic
- _S3b (gitignore-architecture-refactor)_ — full smoke install in
  `/tmp` with 10 install gates + 6 `check-ignore` cases + symlink
  mode `120000` confirmed
- _Hardening v1 S6 (adversarial replay post-bake)_ — 0 SUCCESS, 3
  threat-model criteria held
- _Hardening v2 S4 (PoC #9 replay)_ — `REFUSED` on HEAD `2cd3cd6`,
  `test-dns-strict.sh` 6/0/1, `test-firewall.sh` 0 ❌ / 2 ⚠️ /
  2 ℹ️
- _v1.3 abort path_ — exercised during S2 smoke install (fake
  `.configured-setup` with `VERSION="1.3.0"` → install.sh aborts
  cleanly with Part 2 pointer)

See [plans/devcontainer-tools-v2-migration/LOG.md § P1-S4](plans/devcontainer-tools-v2-migration/LOG.md)
for the consolidated evidence chain.

### Session 5 — bump-changelog — ✅ DELIVERED (2026-05-22)

Done : v2.0.0 CHANGELOG entry written (breaking drops + renames +
new features incl. firewall hardening), root `README.md` created
for the v2 install flow, `TEMPLATE_VERSION` confirmed at `"2.0.0"`,
tag `v2.0.0` posed locally. Push deferred to user.

See [plans/devcontainer-tools-v2-migration/LOG.md § P1-S5](plans/devcontainer-tools-v2-migration/LOG.md)
for the file-touched list and diff stats.

#### Original session 5 plan (kept for posterity)

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

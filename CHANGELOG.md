# Changelog

## 2.0.0 (2026-05-22)

Major rewrite. v1.3 → v2.0 : one installer, scrubbed baseline, no
auto-migration (see Part 2 when specced).

### Breaking changes (drops)

- `gh-secure/` (6 scripts) — superseded by the `/prepare-pr` skill.
- `Dockerfile.node` — the generic `Dockerfile` covers Node projects.
- `templates/master-review/` skill — superseded by `/prepare-pr`.
- `templates/KNOWLEDGE.md` (single file) — superseded by the
  `knowledge/` directory (6 files : INDEX, firewall, wtf,
  extension-points, docker-base-image, ollama-local).
- `templates/test-db.php` — unused.
- `templates/gitignore-entries.txt` — `install.sh` ships
  `.gitignore` content inline now (further refined in S3b — split
  between `.devcontainer/.gitignore` and root-scope inline list).
- `update.sh` — the full-resync script proved too fragile for a
  major bump. Replaced by two Claude-driven playbooks shipped at
  repo root : [MIGRATION-v1-to-v2.md](MIGRATION-v1-to-v2.md)
  (one-shot v1.x → v2.0) and [UPGRADE-v2.md](UPGRADE-v2.md)
  (routine v2.x → v2.y). Bootstrap = `git clone --branch <tag>` +
  paste a one-line prompt into Claude.

### Renames / relocations

- `templates/Dockerfile.custom` → `templates/v2/Dockerfile` (default
  project layer).
- `templates/` → `templates/v2/` (versioned scheme ; allows future
  variants under `templates/v3/`, `templates/v2/minimal/`, etc.).
- `LESSONS.md` / `LESSONS.local.md` : root → `.devcontainer/` with a
  root symlink for `LESSONS.md` (mode 120000, same pattern as
  `CLAUDE.md`).

### New

- 4-prompt wizard (down from 13) : PROJECT_ID, PROJECT_DISPLAY_NAME,
  PROJECT_TYPE, shared creds volume.
- Shell expansion (`${VAR:-default}`) replaces 11 sed placeholders.
  Only `{{PROJECT_ID}}` and `{{PROJECT_DISPLAY_NAME}}` survive.
- Detect-existing logic : v1.3 marker → abort with Part 2 pointer ;
  v2 marker → reinstall / abort choice.
- `Dockerfile.php` ships as a first-class variant
  (PROJECT_TYPE=php).
- `claude-bridge/` sidecar (UniClaudeProxy for local Ollama, opt-in
  via `host-helpers/claude-switch`).
- `host-helpers/` (12 host-side utilities incl. `claude-switch`,
  `claude-bridge`, `verify-slim-base`, etc.).
- Full `knowledge/` directory ships (6 files, replaces the single
  `KNOWLEDGE.md`).
- 4 docs ship (README + RUNBOOK + SECURITY + RESEARCH).
- 5 generic skills ship + `sync-skills.sh` at post-start (excludes
  `.local` skills, which stay per-user manual adds).
- 13 baseline L7 policies in `firewall/policy.d/`.
- Ollama online registry domains (`ollama.com`, `docs.ollama.com`,
  `registry.ollama.ai`) added to the firewall allowlist for debug /
  model-query from inside the container.
- **Firewall layer split** (S3) : project-specific firewall data
  (`domains.txt`, `policy.d/`) moved from `Dockerfile.base` to the
  project layer (`Dockerfile` / `Dockerfile.php`) so
  `claude-devcontainer-base:${VERSION}` stays project-agnostic and
  shareable across projects pinning the same `CLAUDE_CODE_VERSION`.
- **Gitignore architecture split** (S3b) : shipped
  `.devcontainer/.gitignore` for `.devcontainer/`-scoped rules ;
  `install.sh`'s `update_gitignore()` reduced to root-scope only ;
  `.vscode/settings.json` + `.vscode/extensions.json` whitelisted
  so the `post-start.sh` `skip-worktree` trick has tracked files
  to act on.
- **Security hardening** (S3c, delivered via parallel
  `devcontainer-security-hardening` rollouts) — the in-container
  firewall tampering vector is closed :
  - **Firewall baked into the base image** : `firewall/` tree
    (rules, addons, dnsmasq, mode, `direct-tcp-allow.txt`)
    `COPY`'d into `/etc/devcontainer-firewall/` at build time.
  - **Bind mount dropped** : the writable workspace mount no
    longer exposes a path for the node user to tamper with
    firewall config mid-session. Editing firewall config now
    requires an image rebuild — the only audit trail.
  - **Env injection path removed** : the `source /tmp/.firewall-env`
    sudoer-context import is gone.
  - **dnsmasq strict mode** : the catch-all `server=` upstream is
    dropped ; unknown domains return `REFUSED`. Sibling resolve
    generalised to loop over `direct-tcp-allow.txt`.
  - Adversarial validation on HEAD `2cd3cd6` : PoC #9 returns
    `REFUSED`, payload absent from mitmproxy logs ; all three
    threat-model criteria hold (no restart, no firewall mod, no
    exfil without rebuild).

### Fixed

- **`dc-project` fallback restored to v1 parity** — `docker-compose.yml`,
  `initialize.sh` and the `prepare-research` skill now substitute the
  project slug into the bash fallback `${DC_PROJECT:-<slug>}` at install
  time (v1 behavior). Doc examples + `<main-dc-project>` metavariables
  in `RESEARCH.md`, `host-helpers/research-cleanup` and the skill md
  also templatised so installed projects display their own name
  end-to-end. `install.sh` flips 5 `copy_verbatim` → `copy_templated`
  (with post-`copy_dir` overwrites for `host-helpers/` and `skills/`).
- **Stale `paranoid` mentions** referring to the deprecated alias as the
  current mode name — fixed in `firewall/firewall-blocks` (error msg),
  `firewall/domains.txt` and `firewall/policy.d/marketplace.visualstudio.com.yaml`
  comments → now `strict`.

### Notes

- `TEMPLATE_VERSION` is `"2.0.0"` ; the v1.3 detect path aborts
  cleanly with a pointer to Part 2 for existing project upgrades.
- Re-installing v2 over a v2 marker re-prompts for the wizard
  values even though `.configured-setup` holds them ; flagged for
  a follow-up patch (not blocking the v2.0.0 tag).

---

## 1.3.0 (2026-04-28)

### Added
- **Skill: `master-review`** — multi-agent PR code review packaged as a portable skill folder. Wraps the upstream `/review` 5-agent core inside a project-aware flow (tier scoring T1-T4+, distance-from-main check, custom domain agents loaded from `review-config.md`, surface coverage matrix, fix-commit hygiene linter, plateau-detection metrics). Ships with 2 Stop hooks (suggest-fresh-session, log-review-session), 3 review templates, an interactive 5-question bootstrap when no `review-config.md` is found, and a standalone `install-manual.sh` for projects without devcontainer-tools.
- **`add-skill.sh`** — third entrypoint alongside `install.sh` (full bootstrap) and `update.sh` (full re-sync). Copies ONE skill template into an already-bootstrapped project's `.devcontainer/skills/<name>/`. Interactive when run with no args (lists available skills). Idempotent; protects user-customized files via per-skill preservation rules. Use this to add a new skill (or pick up a newly-shipped one) without re-running `update.sh` over the whole project.
- **`copy_if_missing()` / `update_if_missing()` helpers** — `install.sh` and `update.sh` now support a "create-only" semantic (preserve dest if it already exists). Used for files the user is expected to customize, e.g. `master-review/review-config.md`.

### Changed
- `install.sh` and `update.sh` learned to install/update the `master-review` skill folder; `chmod +x` is applied to the 3 hook scripts + `install-manual.sh`.

## 1.2.0 (2026-04-23)

### Added
- **Creds auto-sync during sessions** — new `claude/sync-creds.sh` script + `Stop` / `SessionEnd` hooks merged into `~/.claude/settings.json` by `post-start.sh`. Propagates refreshed OAuth tokens from the per-project `.credentials.json` to the shared `claude-creds` volume in real time, so sibling devcontainers always start with a fresh token (no more `/login` after a few hours).
- **`KNOWLEDGE.md`** — architecture & debug reference (volumes, auth flow, hook/skill install patterns, debug recipes). Committed alongside the devcontainer.

### Changed
- **`post-start.sh`** — `.credentials.json` sync logic extracted to `claude/sync-creds.sh` (same expiry-based merge behavior, reused at boot + runtime). Conflict resolution flow unchanged.
- **`shell-init.sh`** — credentials sync on terminal open now delegates to `claude/sync-creds.sh` instead of duplicating the Python block.

## 1.1.0 (2026-04-10)

### Added
- **Skills system** — `skills/` directory with `sync-skills.sh`, auto-synced at container start via `post-start.sh`
  - `/user:hours` — estimation heures-valeur en fin de session
  - `/user:hours-calibrate` — recalibration mensuelle des prix marche
  - `hooks.json` — time tracking hooks (SessionStart/End, Prompt, Stop, PostToolUse)
  - `hours-calibration.json` — seed data (copied only if < 30 days old)
- **`domains.local.txt`** — gitignored local firewall domains file, read by `init-firewall.sh` alongside `domains.txt`
- **`gitignore-entries.txt`** — canonical list of `.gitignore` entries for devcontainer projects
- **`install.sh update` subcommand** — delegates to `update.sh`
- **Install detects existing `.devcontainer/`** — proposes Update / Full reinstall / Abort
- **Version detection in `update.sh`** — legacy (pre-1.0.0), older, current, newer with appropriate behavior
- **Diff display** — `update.sh` shows `diff -u` (max 30 lines) for each changed file before overwriting
- **`copy_if_fresh()` helper** — copies JSON config files only if source is < 30 days old (install + update)

### Changed
- **GitHub CLI install** — from `apt-get install gh` to official GitHub repo (CVE-2024-53858 fix, >= 2.63.0)
- **Shell init** — sourced from workspace path instead of baked into image (no rebuild needed on changes)
- **Firewall config** — baked into image via COPY + mounted read-only via docker-compose (belt & suspenders)
- **Firewall domains** — calibration domains moved from `domains.txt` to `domains.local.txt` (gitignored)
- **`post-start.sh`** — added `sync-skills.sh` call at end

### Fixed
- **`init-firewall.sh`** — added "already active" guard to prevent re-flush killing network on container restart

## 1.0.0 (2026-03-15)

Initial release.
- Interactive installer (`install.sh`) with 13 prompts
- Update script (`update.sh`) with raw file refresh + migration mode
- Multi-stage Dockerfile (base + node/php/custom)
- Firewall with ipset + domain resolution + CIDR ranges
- gh-secure (GitHub App read-only + scoped PR write)
- Claude Code settings (dev/reviewer modes, read-only permissions template)
- Credential sharing across devcontainers (OAuth token sync with expiry detection)
- `.configured-setup` for config persistence across reinstalls

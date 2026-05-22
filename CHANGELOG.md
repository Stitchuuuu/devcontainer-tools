# Changelog

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

# Existing — technical inventory

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Patches in `updates-v2.1/`

15 patches dated 2026-05-29 → 2026-06-11, produced on a fork dogfood
("Symptems") :

| # | File | Targets | Action |
|---|------|---------|--------|
| 1 | `20260529-0813-claude-project-display-name-rename.patch` | `claude/CLAUDE-project.md` | MODIFY — replace `<PROJECT_DISPLAY_NAME>` literal with `Symptems` (reverse to `{{PROJECT_DISPLAY_NAME}}` for template) |
| 2 | `20260529-1657-install-oh-my-zsh-per-dev-override.patch` | `Dockerfile.base`, `zshrc-base`, `zshrc.local.example`, `shell-init.sh`, `.gitignore` | ADD+MODIFY — OMZ install + per-dev zshrc override (already partially landed : commits `0f97e18` `98498a8`) |
| 3 | `20260529-1700-port-forward-default-ignore.patch` | `devcontainer.json`, `README.md` | MODIFY — `otherPortsAttributes.onAutoForward = "ignore"` (already partially landed : commit `4f4983f`) |
| 4 | `20260529-1735-auto-seed-claude-settings-local-json.patch` | `.claude/settings.local.json.example` (new), `post-create.sh`, `.gitignore` | ADD+MODIFY — auto-seed from `.example` if missing (already partially landed : commits `89f1f00` `009d9eb`) |
| 5 | `20260604-1022-prepare-plan-collapse-session-wrapper.patch` | `skills/prepare-plan/prepare-plan.skill.md` | MODIFY — drop 5-backtick wrapper |
| 6 | `20260604-1023-claude-dev-query-registry-for-versions.patch` | `claude/CLAUDE-dev.md` | MODIFY — add `npm view` / `composer show -a` guidance |
| 7 | `20260604-1038-remove-git-ref-shell-init.patch` | `shell-init.sh` | MODIFY — strip gh auth interactive block |
| 8 | `20260604-1045-patch-claude-ext-primary-location.patch` | `Dockerfile.base`, `patch-claude-extension.py` (new at .devcontainer/ root) | ADD+MODIFY — **superseded by #10** (file renamed to `claude/vscode-ext-patchs/`) |
| 9 | `20260604-1150-dockerfile-base-split-claude-install-cache.patch` | `Dockerfile.base` | MODIFY — **superseded by #13** |
| 10 | `20260610-0911-notify-daemon-vscode-ext-patcher.patch` | 74 files — `notify/`, `claude/vscode-ext-patchs/`, `skills/notify-queue/`, `.env.example`, `.gitignore`, `Dockerfile.base`, `claude/CLAUDE-dev.md`, `initialize.sh` | ADD+MODIFY — major feature ; contains 4 `Symptems` occurrences (tests/docs/`PROJECT_NAME` constant) |
| 11 | `20260610-0925-notify-package-json-cjs-scope.patch` | `notify/package.json`, `skills/notify-queue/package.json` | ADD — `{ "type": "commonjs" }` markers |
| 12 | `20260610-0938-align-post-start-banner-widths.patch` | `post-start.sh` | MODIFY — cosmetic 64-char banner normalisation |
| 13 | `20260610-1126-dockerfile-base-cache-split.patch` | `Dockerfile.base` | MODIFY — supersedes #9 |
| 14 | `20260610-1126-firewall-docker-setup-add.patch` | `firewall/firewall-docker-setup.sh` | ADD — **already present in both `.devcontainer/` and `templates/v2/`** ; content-wise a no-op but verifies installer copy |
| 15 | `20260611-1302-devcontainer-per-project-base-tag.patch` | `.dockerignore` (new), `Dockerfile`, `docker-compose.yml` | ADD+MODIFY — per-project base image tagging ; hardcodes `ARG DC_PROJECT=symptems` to re-templatise |

## Target surface 1 — `templates/v2/` (shipped template)

Top-level layout (excerpt) :

```
templates/v2/
├── .claude/settings.local.json.example
├── .env.example, .gitignore, .gitignore-root
├── Dockerfile, Dockerfile.base, Dockerfile.php
├── claude/                   # CLAUDE-dev.md, CLAUDE-project.md, CLAUDE-local-dev.md, CLAUDE-reviewer.md, sync-creds.sh
├── claude-bridge/            # local Claude proxy sidecar
├── devcontainer.json, docker-compose.yml
├── firewall/                 # addons/, policy.d/, policy.local.d/, policy.local.d.example/, tests/, plus 9 root files
├── host-helpers/             # 12 extensionless utilities
├── initialize.sh, on-create.sh, post-create.sh, post-start.sh, shell-init.sh, install-extensions.sh
├── knowledge/                # 6 maintainer docs
├── skills/                   # sync-skills.sh + 5 generic (prepare-pr, prepare-research, watch-log, scan-deps, prepare-plan)
├── tests/
├── vscode-settings.json, zshrc-base, zshrc.local.example
└── README.md, RUNBOOK.md, SECURITY.md, RESEARCH.md, LESSONS.md
```

## Target surface 2 — `.devcontainer/` (dogfood instance)

Same layout as `templates/v2/`, minus `Dockerfile.php`, plus :
- `.configured-auth`, `.configured-claude-mode`, `.configured-setup`
- `.env` (concrete, `DC_PROJECT=devcontainer-tools`)
- `claude-bridge/` (instantiated sidecar)
- `logs/`, `pending/`, `research-bundles/` (runtime artefacts)
- `SECURITY-AUDIT-2026-05.md` (dogfood-only)
- `diag-ollama-local.sh`

`.devcontainer/initialize.sh` is the substituted version of
`templates/v2/initialize.sh` (no `{{PROJECT_ID}}` left in this fork's
copy).

## Templating mechanics

`install.sh` is the installer that materialises `templates/v2/` into a
target project.

- `install.sh:6` : `TEMPLATE_VERSION="2.0.0"` (will become `"2.1.0"` in S6).
- `install.sh:247-310 copy_templated()` substitutes via :
  ```bash
  sed -e "s|{{PROJECT_ID}}|${PROJECT_ID_ESC}|g" \
      -e "s|{{PROJECT_DISPLAY_NAME}}|${DISPLAY_NAME_ESC}|g" \
      "$src" > "$dst"
  ```
- Currently templated files (8 sed substitutions) :
  - `devcontainer.json`, `.env.example`
  - `initialize.sh`, `docker-compose.yml`
  - `claude/CLAUDE-project.md`, `RESEARCH.md`
  - `host-helpers/research-cleanup`, `skills/prepare-research/prepare-research.skill.md`
- Currently copied verbatim (no substitution) :
  - `Dockerfile.base`, `Dockerfile`, `Dockerfile.php`, `vscode-settings.json`
  - `on-create.sh`, `post-create.sh`, `post-start.sh`, `shell-init.sh`, `install-extensions.sh`
  - `zshrc-base`, `zshrc.local.example`
  - All `firewall/` root files (including `firewall-docker-setup.sh`) + 5 subdirs
  - `claude/` directory (then `claude/CLAUDE-project.md` re-templatised on top)
  - `knowledge/`, `tests/`, `claude-bridge/`, `host-helpers/`, `skills/`
- `install.sh:sed_escape()` escapes `/&|\` — portable BSD+GNU.

## Project identity placeholders observed

| File | Line | Pattern |
|------|------|---------|
| `templates/v2/devcontainer.json` | 2, 41 | `{{PROJECT_DISPLAY_NAME}}` (name + window.title) |
| `templates/v2/initialize.sh` | 66, 68 | `${DC_PROJECT:-{{PROJECT_ID}}}` (CREDS_VOLUME + banner) |
| `templates/v2/.env.example` | 25 | `#DC_PROJECT={{PROJECT_ID}}` |
| `templates/v2/claude/CLAUDE-project.md` | 1 | `# {{PROJECT_DISPLAY_NAME}} — Project rules` |
| `templates/v2/RESEARCH.md` | 38–39 | `<main-{{PROJECT_ID}}>-research-…` |
| `templates/v2/skills/prepare-research/prepare-research.skill.md` | 18, 50–57 | `<main-{{PROJECT_ID}}>-research-<task-slug>` |

## Suspected installer gaps (to confirm in session 5)

- **`firewall-docker-setup.sh` propagation** : `install.sh:278` copies it
  via `copy_verbatim` — looks correct, but patch #14 exists because the
  file was missing on the Symptems fork, so re-test from scratch.
- **`{{PROJECT_ID}}` survival in `initialize.sh`** : the user reported
  hand-editing this twice in two different projects. `initialize.sh` is
  in `copy_templated`, so substitution should work. Hypothesis to test :
  either the imbricated `${DC_PROJECT:-{{PROJECT_ID}}}` confuses sed, or
  other placeholder-bearing files are not in the templated list.
- **`claude-bridge/`, `host-helpers/`** : copied via `copy_dir` — if any
  file under these contains `{{PROJECT_ID}}` / `{{PROJECT_DISPLAY_NAME}}`,
  it gets copied verbatim (no substitution). Grep-test in S5.

## Current branch + recent commits (state at scaffold time)

Branch : `main`.

Recent commits :
```
89f1f00 feat(template): track settings.local.json.example + auto-seed on rebuild
009d9eb chore(dogfood): apply settings.local.json auto-seed to .devcontainer/
0f97e18 chore(dogfood): apply zsh OMZ refactor to .devcontainer/
98498a8 feat(template): drop ZSH_CUSTOM redirect, eastwood default, bashcompinit for wtf
4f4983f chore(devcontainer): default port-forward policy to ignore
```

This suggests patches #2, #3, #4 have already been **partially** landed
in `.devcontainer/` + `templates/v2/`. Sessions 1 and 2 must **verify**
which hunks are still missing rather than blindly re-applying.

Uncommitted state (also at scaffold time) :
- `M .devcontainer/firewall/domains.txt`
- `M CHANGELOG.md` (in-flight, S6 will land 2.1.0 here)
- `D MIGRATION-1.1.0.md` (replaced by `MIGRATION-v1-to-v2.md`)
- `M README.md`, `M plans/devcontainer-tools-v2-migration/STATUS.md`
- `M templates/v2/firewall/domains.txt`
- Untracked : `.tmp/`, `.vscode/`, `.wtfcmd.yaml`, `MIGRATION-v1-to-v2.md`,
  `UPGRADE-v2.md`, `devcontainer-tools-v1.3.0/`, `updates-v2.1/`,
  `upstream-devcontainer-tools.md`, `zshrc`

Session 1 must first decide what to do with the uncommitted state (commit
separately, stash, or fold into the session).

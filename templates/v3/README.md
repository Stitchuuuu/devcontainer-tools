# templates/v3 — what lives where

The v3 architecture splits the old all-in-one template into distributed
artifacts. This directory no longer holds everything :

- **`project/`** — the **copy-paste `.devcontainer/` for a v3 project** :
  thin fw-bake Dockerfile over the published GHCR image, `devc-hook`
  lifecycle, firewall allowlist, hook overlay dirs, patched
  `initialize.sh` (no local base build). Hand-assembled precursor of the
  session-5 `devc init` scaffold — see its README for install steps.

- **`dockerbase/`** — the **project-side** template source only : the thin
  project `Dockerfile` (fw-bake + `FROM` the published base),
  `devcontainer.json`, `docker-compose.yml`, `initialize.sh`, firewall
  allowlists, `claude/` seeds, docs. Session 5's `devc init` scaffolds from
  this material.
- **Image content moved out** (2026-08-01, session 4) : everything that ships
  inside the base image — `bin/` (devc-hook, firewall toolchain), `hooks/`,
  `skills/`, `knowledge/`, `zshrc-base`, `claude/vscode-ext-patchs/`,
  `firewall/{dnsmasq.conf,tests,addons}`, `Dockerfile.base` and the five
  stack Dockerfile variants — now lives in the standalone repo
  **`packages/devcontainer-base/`** (its own git history, published as
  `ghcr.io/meitogi/devcontainer-claude-code`, gitignored here). The stack
  variants became documented blocks under its `stacks/`.
- **`cli-devcontainer/`** — pointer only ; the CLI is
  `packages/devcontainer-cli/` (tracked in this repo).

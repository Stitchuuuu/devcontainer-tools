# Session 1 — bump-and-verify

> **Effort** : ~1 session (~½–1 day, host rebuild dominates) | **Dependencies** : none (first session)

## Prompt to paste

`````
Je démarre la session 1 du rollout `node-24-bump`.

Entry point : `/workspace/plans/node-24-bump/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory, with line-precise file pointers)
- `sessions/session-1-bump-and-verify.md` (this spec)

## Goal

Migrate the devcontainer v2.1 base image from `node:20-slim` to `node:24-slim`.
This is also a Debian distro jump (bookworm → trixie, glibc 2.36 → 2.40).
Preserve PHP 8.2 on the variant via the Sury APT repo (`packages.sury.org`)
because trixie main no longer ships `php8.2-*`.

## Scope — 6 files to edit

### 1. `.devcontainer/Dockerfile.base`
- Line 7 : `FROM node:20-slim` → `FROM node:24-slim`
- Line 111 (comment) : `(node already exists in node:20-slim)` → `(node already exists in node:24-slim)`
- No other structural changes. `build-essential` / `libssl-dev` / `python3` are already present for node-gyp postinstalls (sharp, bcrypt).

### 2. `.devcontainer/Dockerfile.php`
- Update the comment block at lines 14-18 : drop the bookworm explanation, replace with a note that trixie main ships PHP 8.4 by default and Sury provides byte-identical `php8.2-*` package names.
- Before the existing `apt-get install -y --no-install-recommends php8.2-cli ...` RUN, insert a new RUN that wires up the Sury repo. Use this exact pattern :

```dockerfile
# Sury APT repo — Ondřej Surý (official Debian PHP maintainer) provides
# php8.2-* packages on trixie. Required because trixie main now ships
# PHP 8.4 by default. Package names match Debian's, so the install
# block below is byte-identical to the bookworm version.
RUN apt-get update && apt-get install -y --no-install-recommends \
      apt-transport-https lsb-release ca-certificates curl \
    && curl -fsSL https://packages.sury.org/php/apt.gpg \
        -o /usr/share/keyrings/sury-php-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/sury-php-archive-keyring.gpg] https://packages.sury.org/php/ $(lsb_release -sc) main" \
        > /etc/apt/sources.list.d/sury-php.list \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
```

- The existing `apt-get install -y --no-install-recommends php8.2-cli ... php8.2-phar` block stays **byte-identical** — Sury uses the same package names as Debian.
- `COPY --from=composer:2 /usr/bin/composer /usr/bin/composer` (line 38) is unchanged.

### 3. `.devcontainer/host-helpers/verify-slim-base`
- Line 160 (comment) : `node:20-slim` → `node:24-slim`
- Line 161 (`hdr` call) : `node:20-slim` → `node:24-slim` in both the title and the `(not node:20)` parenthetical
- Lines 166 + 168 : `full node:20 ≈ 250+` → `full node:24 ≈ 250+` (two occurrences in `pass` / `info` messages)
- The 150-vs-250 package count threshold stays — confirm empirically post-rebuild and bump only if reality disagrees.

### 4. `.devcontainer/README.md`
- Line 148 : `[v2.1] heavy base image (~1.1 GB) — node:20-slim + apt` → `node:24-slim + apt` and the size figure is updated **after** the host rebuild (see Verification below).

### 5. `.devcontainer/RUNBOOK.md`
- Line 492 : narrative example "Image went from 1.1 GB to 1.4 GB" — only edit if the new baseline diverges meaningfully from 1.1 GB. Otherwise leave as-is (example is hypothetical anyway).

### 6. `.devcontainer/knowledge/docker-base-image.md`
- Line 9 (ASCII diagram) : `(~1.1 GB live-measured)` → measured size from rebuild
- Line 52 (layer table) : `| 1 | `FROM node:20-slim` | Debian/Node base bump | ~140 MB |` → `node:24-slim` + measured layer size
- Line 65 (glibc note) : rewrite to explicit trixie / glibc 2.40 and confirm sharp / bcrypt postinstalls still work
- Line 71 : if measured baseline exceeds 1.1 GB significantly, also lift the `size cap 1.2 GiB` mention (otherwise leave)

## Verification (host-side — cannot be run inside the devcontainer)

The base image rebuild and most checks require the host's Docker engine.
Generate this script for the user (via the watch-log skill or as a plain
host script) :

```bash
#!/usr/bin/env bash
set -euo pipefail

cd /workspace/.devcontainer

# 1. Rebuild the base image from scratch
docker build --no-cache -f Dockerfile.base \
  -t claude-devcontainer-base:2.1.145 .

# 2. Measure size — feeds the doc updates in §4 / §6
SIZE=$(docker image inspect claude-devcontainer-base:2.1.145 \
        --format='{{.Size}}' | numfmt --to=iec)
echo "Measured baseline: ${SIZE}"

# 3. 9-gate consistency check
./host-helpers/verify-slim-base claude-devcontainer-base:2.1.145

# 4. Failsafe Claude chain smoke test
docker run --rm claude-devcontainer-base:2.1.145 cat /etc/claude-source
docker run --rm claude-devcontainer-base:2.1.145 claude --version

# 5. Third-party tools — trixie compat
docker run --rm claude-devcontainer-base:2.1.145 bash -c \
  'mitmdump --version && git --version && delta --version && gh --version && wtf --help | head -1'

# 6. PHP variant — Sury + extensions
docker build -f Dockerfile.php -t claude-devcontainer:php-test .
docker run --rm claude-devcontainer:php-test bash -c \
  'php -v && composer --version && php -m | grep -E "^(curl|gd|mbstring|xml|zip|intl|mysqli|bcmath)$" | wc -l'

# 7. Native postinstalls under glibc 2.40
docker run --rm -v /tmp:/tmp claude-devcontainer-base:2.1.145 bash -c \
  'cd /tmp && rm -rf node-postinstall-smoke && mkdir node-postinstall-smoke && \
   cd node-postinstall-smoke && npm init -y && \
   npm install sharp bcrypt --no-audit --no-fund'
```

## Success criteria

- `verify-slim-base` returns 9 / 9 PASS
- `claude --version` prints `2.1.145` (any failsafe branch is acceptable, but check `/etc/claude-source` notes which one fired)
- `php -v` prints `PHP 8.2.x` (NOT 8.4)
- Composer 2 and the 8 listed PHP extensions are present
- `sharp` and `bcrypt` install without compile errors
- Image size ≤ 1.2 GiB (current cap)
- 4 documentation files (README, RUNBOOK if applicable, knowledge/docker-base-image, EXISTING.md in this plan) reflect the measured size

## DoD at the end of this session

1. **STATUS.md** : flip session 1 row 📋 → ✅, replace prompt link with `—`, bump Delivered counter (0 → 1), set "Next focus" to `rollout complete` (or add session 2 if follow-up work surfaced).
2. **LOG.md** : append `## 1 — bump-and-verify` section dated today with files touched + What / Why / Decisions / Gotchas / Tests / Commit subject.
3. **EXISTING.md** : update the file inventory if any file was added (e.g. the Sury sources/keyring path now exists as a baked artifact — note it).
4. Propose a commit. Self-contained message (no plan / session / phase reference per the user's standing preference). Suggested subject : `Bump base image from node:20-slim to node:24-slim` with a body covering the distro jump, Sury for PHP 8.2, and the measured size delta. **Do NOT run `git commit` without explicit user confirmation.**

## Out of scope (do NOT fold in)

- Bumping `CLAUDE_CODE_VERSION`, `MITM_VERSION`, `GIT_DELTA_VERSION`, or `WTF_VERSION`. If a bump is desired, open a separate session.
- Broader APT audit (other package version bumps).
- Firewall allowlist changes — Sury access is only needed at `docker build` time which bypasses the runtime firewall.
- Multi-arch testing beyond the user's normal architecture, unless the user explicitly requests it. The diff is arch-agnostic by design.
`````

## Next session

To be decided at the end of session 1. If the rebuild surfaces an unexpected regression (size cap breach, package compat issue), add a row in STATUS.md and create `sessions/session-2-<slug>.md`. Otherwise mark the rollout complete in STATUS.md.

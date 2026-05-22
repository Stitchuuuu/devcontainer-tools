# Existing — technical inventory

> Snapshot of the code state. Updated when a session adds / removes /
> restructures major files. Reflects state **after session 1 delivery**.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Files referencing the Node version + Debian base

All paths now reference `node:24-bookworm-slim` (explicit pin) after session 1.
Mirror invariant : `.devcontainer/` (dogfood) and `templates/v2/` (template
source-of-truth) are byte-identical for the 5 files below.

| File | Line | Reference |
|---|---|---|
| [.devcontainer/Dockerfile.base](../../.devcontainer/Dockerfile.base#L7) | 7 | `FROM node:24-bookworm-slim` (the only `FROM` line that matters) |
| [.devcontainer/Dockerfile.base](../../.devcontainer/Dockerfile.base#L117) | 117 | Comment `(node already exists in node:24-bookworm-slim)` |
| [templates/v2/Dockerfile.php](../../templates/v2/Dockerfile.php#L14) | 14-24 | Sury APT repo wire-up RUN block (future-proof for trixie ; also serves bookworm with newer point releases) |
| [templates/v2/Dockerfile.php](../../templates/v2/Dockerfile.php#L26) | 26-49 | `apt-get install` block for `php8.2-*` extensions (byte-identical names across bookworm + trixie) |
| [.devcontainer/host-helpers/verify-slim-base](../../.devcontainer/host-helpers/verify-slim-base#L160) | 160-172 | Check 7 (slim distinction) + check 3 (npm-squat) — recalibrated to exclude `.vscode-server` from `/home/node` and threshold raised to `< 300` pkgs |
| [.devcontainer/README.md](../../.devcontainer/README.md#L148) | 148 | `[v2.1] heavy base image (~1.1 GB) — node:24-bookworm-slim + apt` |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L52) | 52 | Layer table row `FROM node:24-bookworm-slim ~140 MB` |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L65) | 65 | "Glibc 2.36 (Debian bookworm) preserved …" note + explanation of why we pin |

## Dockerfile.php location

The PHP variant lives **only** in [templates/v2/Dockerfile.php](../../templates/v2/Dockerfile.php). It is **not** mirrored to `.devcontainer/` (this repo dogfoods Node-only — no PHP project). `install.sh` copies it as `Dockerfile` in projects that pick `PROJECT_TYPE=php`.

## Baked artefacts added on PHP variant by Sury wire-up

| Artefact | Path | Source |
|---|---|---|
| Sury keyring | `/usr/share/keyrings/sury-php-archive-keyring.gpg` | `curl packages.sury.org/php/apt.gpg` at build time |
| Sury APT list | `/etc/apt/sources.list.d/sury-php.list` | Written via `echo "deb [signed-by=…] https://packages.sury.org/php/ $(lsb_release -sc) main"` |

`lsb_release -sc` resolves at build time → `bookworm` today (so Sury serves bookworm), `trixie` later if/when node:24-bookworm-slim pin is changed.

## Build pipeline

- Build orchestrator: [.devcontainer/initialize.sh](../../.devcontainer/initialize.sh) — drives `docker build` for the base image, tagged `claude-devcontainer-base:${CLAUDE_CODE_VERSION}`.
- Single source of truth for Claude version: [.devcontainer/.env](../../.devcontainer/.env) → `CLAUDE_CODE_VERSION=2.1.145`.
- Compose entry: [.devcontainer/docker-compose.yml](../../.devcontainer/docker-compose.yml) — references the tagged base image.

## CLI tools baked into the base image (distro coupling notes)

| Tool | Install method | Line | Coupling |
|---|---|---|---|
| `mitmproxy` 12.2.3 | Standalone tarball | [Dockerfile.base:61-74](../../.devcontainer/Dockerfile.base#L61-L74) | Forward-compat (binary linked against older glibc, runs on newer fine) |
| `gh` | APT from cli.github.com | [Dockerfile.base:77-80](../../.devcontainer/Dockerfile.base#L77-L80) | Codename-resolved by upstream repo ; works on bookworm/trixie |
| `wtf` | GitHub release binary | [Dockerfile.base:95-108](../../.devcontainer/Dockerfile.base#L95-L108) | Static Go binary — distro-agnostic |
| `git-delta` 0.18.2 | `.deb` from GitHub releases | [Dockerfile.base:126-129](../../.devcontainer/Dockerfile.base#L126-L129) | `.deb` is distro-version-independent (statically linked) |
| Claude Code CLI 2.1.145 | VSIX-baked + npm fallback | [Dockerfile.base:149-228](../../.devcontainer/Dockerfile.base#L149-L228) | Bun-compiled native binary, glibc forward-compat |
| `build-essential` + `libssl-dev` | APT main | [Dockerfile.base:45-47](../../.devcontainer/Dockerfile.base#L45-L47) | Required for `sharp` / `bcrypt` postinstalls — works identically on bookworm |
| PHP 8.2 (variant) | APT via Sury repo | [templates/v2/Dockerfile.php:18-49](../../templates/v2/Dockerfile.php#L18-L49) | Sury resolves on bookworm (8.2.31 observed) and trixie (when upstream rebases) |
| Composer 2 | Multi-stage COPY | [templates/v2/Dockerfile.php:52](../../templates/v2/Dockerfile.php#L52) | Distro-agnostic (static binary) |

## What is NOT impacted

- Firewall stack (`init-firewall.sh`, `mitm-init.sh`, `compile-policy.py`, iptables/ipset/dnsmasq) — Node-agnostic and distro-agnostic at the iptables/ipset/dnsmasq versions installed.
- Failsafe Claude chain logic — 3 scenarios via `/etc/claude-source` are orthogonal to the base distro.
- Workspace user code — runs against the user's project Node version, not the container's system Node (which only powers the global CLIs).

## Verification helper (post-recalibration)

[.devcontainer/host-helpers/verify-slim-base](../../.devcontainer/host-helpers/verify-slim-base) provides 9 PASS/FAIL gates. Two gates were recalibrated in session 1 :

- **Check 3 — .npm squat regression test** : `du` of `/home/node` now excludes `.vscode-server` so the 5 MiB cap stays meaningful as a squat detector (the v2.1-1 VSIX-bake puts ~233 MiB at `.vscode-server/extensions/` which would otherwise always tank the gate).
- **Check 7 — slim package count** : threshold raised from `< 200` to `< 300` because `node:24-bookworm-slim` ships ~255 pkgs vs ~150 on `node:20-slim`. Same Debian base, just more Node 24 dependencies pre-installed by upstream.

Other gates unchanged : image presence, total size ≤ 1.2 GiB, Claude pin match, opencode absent, mitmproxy bundled, layer dedup, docker system df.

Measured baseline post-session-1 rebuild : **1.08 GiB** (1110 MiB exact), well under the 1.2 GiB cap.

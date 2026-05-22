# Existing — technical inventory

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Files referencing `node:20` or the underlying distro

| File | Line | Reference |
|---|---|---|
| [.devcontainer/Dockerfile.base](../../.devcontainer/Dockerfile.base#L7) | 7 | `FROM node:20-slim` (the only `FROM` line that matters) |
| [.devcontainer/Dockerfile.base](../../.devcontainer/Dockerfile.base#L111) | 111 | Comment `(node already exists in node:20-slim)` |
| [.devcontainer/Dockerfile.php](../../.devcontainer/Dockerfile.php#L14) | 14 | Comment `Debian bookworm slim (base image upstream) ships php8.2-* in main, no PPA required` — outdated under trixie |
| [.devcontainer/host-helpers/verify-slim-base](../../.devcontainer/host-helpers/verify-slim-base#L160) | 160-168 | Check 7 header + pass/info strings mention `node:20-slim` / `full node:20` |
| [.devcontainer/README.md](../../.devcontainer/README.md#L148) | 148 | `[v2.1] heavy base image (~1.1 GB) — node:20-slim + apt` |
| [.devcontainer/RUNBOOK.md](../../.devcontainer/RUNBOOK.md#L492) | 492 | Narrative example `"Image went from 1.1 GB to 1.4 GB"` |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L9) | 9 | ASCII diagram `(~1.1 GB live-measured)` |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L52) | 52 | Layer table row `FROM node:20-slim ~140 MB` |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L65) | 65 | "Glibc preserved" note — needs trixie/glibc 2.40 update |
| [.devcontainer/knowledge/docker-base-image.md](../../.devcontainer/knowledge/docker-base-image.md#L71) | 71 | `verify-slim-base` gate description with `size cap 1.2 GiB` |

## Build pipeline

- Build orchestrator: [.devcontainer/initialize.sh](../../.devcontainer/initialize.sh) — drives `docker build` for the base image, tagged `claude-devcontainer-base:${CLAUDE_CODE_VERSION}`.
- Single source of truth for Claude version: [.devcontainer/.env](../../.devcontainer/.env) → `CLAUDE_CODE_VERSION=2.1.145`.
- Compose entry: [.devcontainer/docker-compose.yml](../../.devcontainer/docker-compose.yml) — references the tagged base image.

## CLI tools baked into the base image (impact surface for the distro jump)

| Tool | Install method | Line | Coupling to distro |
|---|---|---|---|
| `mitmproxy` 12.2.3 | Standalone tarball | [Dockerfile.base:61-74](../../.devcontainer/Dockerfile.base#L61-L74) | glibc forward-compat only (binary linked against older glibc, runs on 2.40 fine) |
| `gh` | APT from cli.github.com | [Dockerfile.base:77-80](../../.devcontainer/Dockerfile.base#L77-L80) | Repo supports trixie ; verify deb is selected for trixie codename |
| `wtf` | GitHub release binary | [Dockerfile.base:95-108](../../.devcontainer/Dockerfile.base#L95-L108) | Static Go binary — distro-agnostic |
| `git-delta` 0.18.2 | `.deb` from GitHub releases | [Dockerfile.base:126-129](../../.devcontainer/Dockerfile.base#L126-L129) | `.deb` is distro-version-independent (built statically against modern glibc) |
| Claude Code CLI 2.1.145 | VSIX-baked + npm fallback | [Dockerfile.base:149-228](../../.devcontainer/Dockerfile.base#L149-L228) | Bun-compiled native binary, glibc forward-compat |
| `build-essential` + `libssl-dev` | APT main | [Dockerfile.base:45-47](../../.devcontainer/Dockerfile.base#L45-L47) | Required for `sharp` / `bcrypt` postinstalls — trixie versions are newer but ABI-compatible |
| PHP 8.2 (variant) | APT main (bookworm) | [Dockerfile.php:19-35](../../.devcontainer/Dockerfile.php#L19-L35) | **NOT available in trixie main** — switch to Sury repo |
| Composer 2 | Multi-stage COPY | [Dockerfile.php:38](../../.devcontainer/Dockerfile.php#L38) | Distro-agnostic (static binary) |

## What is NOT impacted

- Firewall stack (`init-firewall.sh`, `mitm-init.sh`, `compile-policy.py`, iptables/ipset/dnsmasq) — Node-agnostic and distro-agnostic at the iptables/ipset/dnsmasq versions installed.
- Failsafe Claude chain logic — 3 scenarios via `/etc/claude-source` are orthogonal to the base distro.
- Workspace user code — runs against the user's project Node version, not the container's system Node (which only powers the global CLIs).

## Verification helper

[.devcontainer/host-helpers/verify-slim-base](../../.devcontainer/host-helpers/verify-slim-base) provides 9 PASS/FAIL gates: size cap 1.2 GiB, `/home/node` ≤ 5 MiB, Claude pin match, opencode absent, mitmproxy bundled, slim package count consistent, layer dedup vs other images, docker system df snapshot, base image is `slim` flavour (not full).

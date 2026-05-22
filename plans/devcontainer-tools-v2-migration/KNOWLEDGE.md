# Knowledge — Devcontainer Tools V2

> Architectural reference for the v2 template. Captures the
> *invariants* — what is built where, by whom, and why. For the
> chronological history of changes, see [LOG.md](LOG.md). For the
> Part 1 file inventory, see [SCOPE.md](SCOPE.md).

## Architecture

The v2 devcontainer ships as **two image layers** stacked at build
time, with one fallback variant for PHP projects :

```
┌──────────────────────────────────────────────────────────────┐
│ Project layer   ← Dockerfile  OR  Dockerfile.php             │
│   per-project, rebuilt on every project change (~5s)         │
│   FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}       │
│   Contains : project firewall data (4 files), project apt    │
│   deps (optional), PHP 8.2 + Composer (Dockerfile.php only)  │
├──────────────────────────────────────────────────────────────┤
│ Base layer      ← Dockerfile.base                            │
│   built ONCE per CLAUDE_CODE_VERSION                         │
│   tagged claude-devcontainer-base:${CLAUDE_CODE_VERSION}     │
│   shared across ALL projects pinning the same version        │
│   Contains : Debian + apt baseline, Claude Code CLI, Node /  │
│   Python toolchains, firewall scripts (init/test/compile/    │
│   addons/dnsmasq.conf), firewall tests/, sudoers, shell init │
└──────────────────────────────────────────────────────────────┘
```

**Build trigger.** `.devcontainer/initialize.sh` detects a missing
base tag and runs `docker build -f Dockerfile.base -t
claude-devcontainer-base:${CLAUDE_CODE_VERSION}` before docker-compose
builds the project layer. A `CLAUDE_CODE_VERSION` bump in
`.devcontainer/.env` is the *only* trigger for a base rebuild.

**Project layer always rebuilds.** It's cheap (~5s, just the firewall
data COPY + chown/chmod). Docker's build cache reuses unchanged base
layers, so a project firewall edit only invalidates the project layer.

## Copy logic

Which file goes where, and why :

| Path                                          | Layer      | Rationale                                               |
|-----------------------------------------------|------------|---------------------------------------------------------|
| `init-firewall.sh` → `/usr/local/bin/`        | **base**   | infra script, identical across projects                 |
| `test-firewall.sh` → `/usr/local/bin/`        | **base**   | infra script                                            |
| `firewall/compile-policy.py` → `/usr/local/bin/` | **base** | infra script                                            |
| `firewall/mitm-init.sh` → `/usr/local/bin/`   | **base**   | infra script                                            |
| `firewall/firewall-blocks` → `/usr/local/bin/`| **base**   | infra script                                            |
| `firewall/dnsmasq.conf` → `/etc/devcontainer-firewall/` | **base** | DNS infra config, never per-project              |
| `firewall/tests/` → `/etc/devcontainer-firewall/tests/` | **base** | self-test probes, infra                          |
| `firewall/addons/` → `/etc/devcontainer-firewall/addons/` | **base** | mitmproxy Python addons, infra                 |
| `firewall/firewall-docker-setup.sh` → `/usr/local/bin/` | **base** | build-time perms-finalize script (called from project layer RUN) |
| `firewall/domains.txt` → `/etc/devcontainer-firewall/`  | **project** | per-project allowlist baseline                  |
| `firewall/policy.d/` → `/etc/devcontainer-firewall/`    | **project** | per-project L7 policy baseline (13 files)       |

The `.example` files (`domains.local.txt.example`,
`policy.local.d.example/`) are deliberately **NOT** COPY'd into the
container : they're pure host-side reference, accessible from inside
via the `/workspace/.devcontainer/firewall/` bind mount. Runtime
firewall scripts (`init-firewall.sh`, `compile-policy.py`) only ever
read `domains.local.txt` / `policy.local.d/`, never the `.example`
variants — so shipping them in the image would be dead weight.

Runtime resolution : `init-firewall.sh` reads from
`$FIREWALL_CONFIG_DIR` (default `/etc/devcontainer-firewall/`).
The layer split is invisible to the runtime — both layers COPY into
the same path.

**Project-layer perms finalization via `firewall-docker-setup.sh`.**
The project layer's `RUN /usr/local/bin/firewall-docker-setup.sh`
delegates to a 3-line script in the base image :
  1. `touch domains.local.txt` (runtime expects the file to exist,
     even if empty)
  2. `chown -R root:root /etc/devcontainer-firewall`
  3. `chmod -R u=rwX,go=rX /etc/devcontainer-firewall`
     (capital X = "x only if dir or already +x" → 755 on dirs +
     644 on files in ONE pass)

Idempotent on base-layer files (already at those perms — no-op).
Centralising in the script means : (a) `Dockerfile` and `Dockerfile.php`
don't duplicate ~15 lines of perms logic ; (b) perms logic evolves per
`CLAUDE_CODE_VERSION` bump (one source of truth in base), zero
project-side re-`install.sh` needed.

## Image layer split (P1-S3, 2026-05-22)

**Problem.** v2-beta `Dockerfile.base` COPYed 4 project-specific
firewall files (`domains.txt`, `domains.local.txt.example`,
`policy.d/`, `policy.local.d.example/`). Result : any project tweaking
its allowlist would invalidate the base image hash, breaking the
shared-base mental model.

**Fix.** Move the 2 data COPYs that the runtime actually reads
(`domains.txt`, `policy.d/`) to the project layer (`Dockerfile` +
`Dockerfile.php`), and drop the `.example` COPYs entirely from the
container (host-side access via bind mount suffices). Centralise
the perms-finalize logic in `firewall-docker-setup.sh` shipped in
the base image's `/usr/local/bin/`. The base image becomes truly
project-agnostic — two projects on the same host pinning the same
`CLAUDE_CODE_VERSION` now share a single base image, period.

**Cost.** ~5s extra per project rebuild (the 2 project COPY +
1 RUN script invocation). Far cheaper than full base rebuilds.

**Verification.**
- Build sandbox A with default `domains.txt` → note base image ID.
- Build sandbox B with `domains.txt` + `example.com` → base image ID
  MUST be unchanged. Project layer ID differs (expected).
- Inside sandbox A : `cat /etc/devcontainer-firewall/domains.txt`
  MUST NOT contain `example.com` (no cross-leakage from sandbox B).

**Migration for already-deployed v2-beta instances.** Manually edit
the project's 2 Dockerfiles (`.devcontainer/Dockerfile` +
`.devcontainer/Dockerfile.php` if applicable), add the COPY+RUN+USER
block from the post-split template, then `docker build` once. See
[sessions/part-1-session-3-firewall-layer-split.md](sessions/part-1-session-3-firewall-layer-split.md)
for the recipe.

## Dogfooding (templates vs `.devcontainer/`)

This repo dogfoods its own template : `templates/v2/` is the
**canonical** source, `.devcontainer/` is a mirror used to run
Claude Code on the template itself.

**Sync mechanism.** `cp` from templates/v2/ to .devcontainer/
(byte-identical), not hand-edit. Skip files absent in dogfood (e.g.
`Dockerfile.php` — no PHP dogfood). Guarantees zero drift.

For the file inventory and sync rules, see [SCOPE.md](SCOPE.md).

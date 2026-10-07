# devcontainer-tools

A hardened **Claude Code devcontainer** setup: a default-deny outbound
firewall, a sandboxed Claude Code runtime, a sidecar for local Ollama
backends, and a curated set of skills for PR drafting, dependency audits,
scoped research, and time tracking.

The current generation (v3) ships as two separately-versioned artefacts,
each in its own repo:

| Artefact | Source | Distributed via |
|---|---|---|
| the **image** | [`meitogi/devcontainer-sandbox`](https://github.com/meitogi/devcontainer-sandbox), tagged `v1.2.0` | `ghcr.io/meitogi/devcontainer-sandbox:<base>-cc<claude-code>` |
| the **CLI** | [`meitogi/devcontainer-cli`](https://github.com/meitogi/devcontainer-cli) | npm, `@meitogi/devcontainer-cli` |

The CLI scaffolds a thin project layer that pulls the image at container
build time; the image carries the hooks, skills, knowledge base and
firewall machinery. Neither is useful alone.

## Quick start

```bash
npx @meitogi/devcontainer-cli init
```

The wizard detects the stack, asks a handful of questions (project id,
display name, Claude credentials volume, Claude Code version), and writes
`.devcontainer/`. Then open the project in VS Code and run *Dev Containers:
Reopen in Container* — the first build pulls the published image;
subsequent projects pinning the same Claude Code version reuse it.

See [`@meitogi/devcontainer-cli`'s own README](https://github.com/meitogi/devcontainer-cli#readme)
for the full command reference (`devc init`, `devc initialize`).

## What ships

Inside a scaffolded `.devcontainer/` (~25 files — everything else is
inherited from the image at runtime):

- `Dockerfile`, `docker-compose.yml`, `devcontainer.json`, `.env` — the
  project layer that builds on the published base image.
- **Firewall** — a project-level allowlist (`firewall/domains.txt`,
  `domains.d/`, `policy.d/`) layered on top of the image's own baseline;
  `basic`/`strict`/`off` modes, strict by default.
- **Hooks and skills overlay dirs** — `hooks/{on-create,post-create,
  post-start}.d/`, empty until the project adds its own; the image's own
  fragments run regardless.
- `claude/CLAUDE-dev.md`, `claude/CLAUDE-project.md`, a blank `LESSONS.md`.

The image itself carries the toolchain, the baked-in skills, the knowledge
base, and the firewall runtime — see
[`meitogi/devcontainer-sandbox`](https://github.com/meitogi/devcontainer-sandbox).

## Layout

```
.
├── packages/                   # four independently-published sub-projects
│   ├── devcontainer-cli/       # the CLI — own git history, own repo, gitignored here
│   ├── devcontainer-sandbox/   # the image's source — own git history, own repo, gitignored here
│   ├── claude-ext-patchs/      # own git history, own repo, gitignored here
│   └── vscode-ext-patch-starter/  # own git history, own repo, gitignored here
├── plans/                      # multi-session rollout journals
├── CHANGELOG.md                # v1/v2 version history
└── CLAUDE.md                   # dev guidelines for working in this repo
```

Each `packages/*` entry is its own git repository with its own remote — this
repo tracks none of their history; `.gitignore` excludes them.

## Requirements

- **Docker** on the host (Docker Desktop on macOS/Linux; tested on Apple
  Silicon and Linux amd64).
- **VS Code** with the **Dev Containers** extension.
- Node 18+ to run the CLI via `npx`.

## Migrating from v2

There is no automated rewrite of a v2 tree, by design: measured on three real
projects, there is no pristine baseline to diff a v2 tree against, and the
three files the switch must change (`Dockerfile`, `docker-compose.yml`,
`devcontainer.json`) are the three most hand-edited. `devc init` refuses such a
tree and points at `devc migrate`, which reads it, sorts its entries by what
becomes of them, and prints the switch as a checklist with the project's own
values filled in — every step a file edit git can show and revert. It writes
nothing. Once the checklist is done, `devc init` recognises the tree and adds
the missing files. The bash `initializeCommand` can survive the switch through
the shim the CLI ships (`templates/devcontainer/initialize.sh` in the
`devcontainer-cli` package), which hands the step to the published CLI.

## Security posture

The baseline assumes Claude Code may be **fully prompt-injected** inside the
main container. Three threat-model criteria hold:

1. **No restart** — the node user can't restart the container alone.
2. **No firewall modification** — the firewall is baked into the base
   image; editing it requires a rebuild (the only audit trail).
3. **No exfiltration without rebuild** — default-deny outbound, dnsmasq
   strict (no catch-all), mitmproxy with L7 addons.

See `plans/devcontainer-security-hardening/` and
`plans/devcontainer-security-hardening-v2/` for the rollouts that landed
this posture in v2; the same model carries into v3's image.

## Status

- **CLI**: see [`meitogi/devcontainer-cli`'s own release history](https://github.com/meitogi/devcontainer-cli/releases) —
  currently `0.1.1` on npm as `@meitogi/devcontainer-cli`.
- **Image**: `v1.2.0`, published from
  [`meitogi/devcontainer-sandbox`](https://github.com/meitogi/devcontainer-sandbox).

## History: v1 → v2

v1 and v2 shipped as a bash installer (`install.sh`) copying a template tree
into the project; both were retired on 2026-10-07, once the dogfood moved to
v3. [CHANGELOG.md](CHANGELOG.md) keeps the full v1/v2 version history, and a
project still on v2 switches with `devc migrate` (see above).

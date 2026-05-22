# devcontainer-tools

A one-shot installer that drops a hardened **Claude Code devcontainer
baseline** (~95 files) into a fresh project. The baseline ships a
default-deny outbound firewall, a sandboxed Claude Code runtime, a
sidecar for local Ollama backends, and a curated set of skills for
PR drafting, dependency audits, scoped research, and time tracking.

## Quick start

```bash
bash /path/to/devcontainer-tools/install.sh ~/my-project
```

The wizard asks **4 questions** (down from 13 in v1.x) :

1. `PROJECT_ID` — slug used for Docker volumes, defaults to the
   project directory basename.
2. `PROJECT_DISPLAY_NAME` — human-readable name, defaults to the
   title-cased basename.
3. `PROJECT_TYPE` — `node` (default), `php`, or `custom`.
4. **Shared Claude credentials volume** — share OAuth across
   sibling devcontainers (default `claude-creds-shared`) or pick a
   per-project volume.

Then open the project in VS Code and run *Dev Containers : Reopen
in Container*. The first build pulls the base image
(`claude-devcontainer-base:${CLAUDE_CODE_VERSION}`) ; subsequent
projects pinning the same Claude Code version reuse the cached
base.

## What ships

Inside the freshly-installed `<target>/.devcontainer/` :

- **Lifecycle scripts** — `initialize.sh`, `on-create.sh`,
  `post-create.sh`, `post-start.sh`, `shell-init.sh` (each
  idempotent ; safe to replay).
- **Firewall** — DNS + iptables + mitmproxy with 4 addons, baked
  into the image (no runtime bind mount). Strict mode by default ;
  `basic` and `off` modes available via `firewall-mode.sh`.
- **Claude integration** — settings, hooks, dev vs reviewer mode,
  bidirectional OAuth sync across sibling devcontainers via the
  shared credentials volume.
- **Skills** — `/prepare-pr`, `/watch-log`, `/prepare-research`,
  `/scan-deps`, `/prepare-plan` (synced into
  `~/.claude/commands/` at every post-start via
  `skills/sync-skills.sh`).
- **Host helpers** — 12 utilities the user runs on the host to
  drive the container : `claude-switch` (cloud ↔ local Ollama),
  `verify-slim-base`, `analyze-base-image`, `rebuild-base-image`,
  `mitm-capture`, `bring-back-result`, etc.
- **Knowledge base** — `.devcontainer/knowledge/` (6 files :
  INDEX, firewall internals, wtf, extension-points, base-image
  layout, ollama-local).
- **Documentation** — `.devcontainer/README.md`, `RUNBOOK.md`,
  `SECURITY.md`, `RESEARCH.md` — operations, threat model, scoped
  research workflow.

## How it works

`install.sh` copies `templates/v2/` verbatim to
`<target>/.devcontainer/`, with two `sed` replacements
(`{{PROJECT_ID}}`, `{{PROJECT_DISPLAY_NAME}}`) and shell expansion
(`${VAR:-default}`) for everything else. Three additional surfaces :

- A **shipped `.devcontainer/.gitignore`** scoped to internal
  artefacts (logs, scratch dirs, local overrides).
- A **root-scope `.gitignore`** fragment appended to
  `<target>/.gitignore` for entries that must sit at the project
  root (`.claude/`, `.vscode/*` with `settings.json` whitelisted,
  `.env.dev`, `.DS_Store`).
- A **root `LESSONS.md` symlink** pointing at
  `.devcontainer/LESSONS.md` (mode 120000), so per-project
  lessons live inside the devcontainer tree like `CLAUDE.md`.

Re-running `install.sh` against an existing project :

- Detects a **v1.3 marker** (`.configured-setup` with
  `VERSION="1.3.0"`) → aborts with a pointer to the (deferred)
  Part 2 migration prompt — `install.sh` does **not** auto-upgrade
  from 1.3.
- Detects a **v2 marker** → offers `Reinstall` (overwrite) or
  `Abort`.

## Layout

```
.
├── install.sh                  # the installer (4 prompts)
├── templates/v2/               # the baseline (~95 files) shipped to projects
├── CHANGELOG.md                # version history
├── ROADMAP.md                  # high-level roadmap and rollout state
├── KNOWLEDGE.md                # rollout-specific internals
├── plans/                      # multi-session rollout journals
│   ├── devcontainer-tools-v2-migration/      # this tool's v2 rollout
│   ├── devcontainer-security-hardening/      # firewall write-protect rollout
│   ├── devcontainer-security-hardening-v2/   # dnsmasq strict rollout
│   └── INDEX.md                              # index of rollouts
└── CLAUDE.md                   # dev guidelines for working in this repo
```

`templates/v2/` is the only versioned baseline today. A future
`templates/v3/` (or a sibling variant under `templates/v2/`) would
be selectable via the `TEMPLATE_VARIANT` env var — currently
hardcoded to `v2`.

## Requirements

- **Docker** on the host (Docker Desktop on macOS / Linux ; tested
  on Apple Silicon and Linux amd64).
- **VS Code** with the **Dev Containers** extension.
- Roughly **5-10 minutes + ~1.2 GB** on the first base-image build
  (subsequent projects pinning the same `CLAUDE_CODE_VERSION`
  reuse the cached image).

## Migrating from v1.3

`install.sh` v2 does **not** auto-migrate. The original `update.sh`
full-resync proved too brittle for the v1.3 → v2.0 jump. Per-file
reconciliation with judgment calls will ship as a paste-into-Claude
session prompt under `plans/devcontainer-tools-v2-migration/Part 2`
once the v2 baseline has been validated by a few new-project
installs in the wild.

In the meantime, projects on v1.3 stay on v1.3 — `install.sh`
detects the marker and aborts cleanly.

## Security posture

The baseline assumes Claude Code may be **fully prompt-injected**
inside the main container. Three threat-model criteria hold :

1. **No restart** — the node user can't restart the container
   alone.
2. **No firewall modification** — the firewall is baked into the
   base image ; editing it requires a rebuild (the only audit
   trail).
3. **No exfiltration without rebuild** — default-deny outbound,
   dnsmasq strict (no catch-all), mitmproxy with 4 L7 addons.

See `plans/devcontainer-security-hardening/` and
`plans/devcontainer-security-hardening-v2/` for the two rollouts
that landed this posture, and `templates/v2/SECURITY.md` for the
threat model shipped to consumers.

## Status

`v2.0.0` — released 2026-05-22. See [CHANGELOG.md](CHANGELOG.md).

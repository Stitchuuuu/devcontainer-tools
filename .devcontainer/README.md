# DevContainer — Claude Code Sandbox

<!-- devcontainer layout: v3 — published base image ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.280 -->

This devcontainer runs Claude Code under a default-deny outbound firewall, on top of the published base image `ghcr.io/meitogi/devcontainer-sandbox`. Every outbound connection that is not in an explicit allowlist is refused at the DNS layer; in `strict` mode an L7 filter (mitmproxy) additionally scopes paths, methods and POST body sizes per endpoint. This tree runs `basic` — host-level allowlist, no L7 filter (see [SECURITY](docs/SECURITY.md) for what that does and does not protect). The container can read code and talk to `api.anthropic.com`; it cannot push, cannot create PRs, and cannot reach arbitrary third-party APIs.

This README is the maintainer's handbook. Skim it once to understand what's where; consult [RUNBOOK.md](RUNBOOK.md) for step-by-step operations and [SECURITY](docs/SECURITY.md) for the threat model. The image's own documentation is baked at `/opt/devcontainer/base/docs/` (start with `getting-started.md`, `concepts.md`, `boot-warnings.md`, `troubleshooting.md`, `how-to/`) and its AI-facing internals at `/opt/devcontainer/base/knowledge/INDEX.md`.

## Table of contents

- [Port forwarding (dev servers)](#port-forwarding-dev-servers)
- [TL;DR](#tldr)
- [The two-container model](#the-two-container-model)
- [Quick start](#quick-start)
- [Container lifecycle](#container-lifecycle)
  - [Boot panel and banners](#boot-panel-and-banners)
- [Directory layout](#directory-layout)
- [Firewall modes](#firewall-modes)
- [Local hosts — DNS-driven aliases (no `extra_hosts`)](#local-hosts--dns-driven-aliases-no-extra_hosts)
- [Local backends — switch Claude to Ollama](#local-backends--switch-claude-to-ollama)
- [Claude Code and the base image](#claude-code-and-the-base-image)
  - [Bumping Claude Code or the base](#bumping-claude-code-or-the-base)
  - [VS Code extension patchers](#vs-code-extension-patchers)
- [Host helpers (debug + sanity)](#host-helpers-debug--sanity)
- [Commands and skills](#commands-and-skills)
  - [Skills shipped by the image](#skills-shipped-by-the-image)
  - [Project-local skills (gitignored)](#project-local-skills-gitignored)
  - [Anthropic global skills](#anthropic-global-skills)
  - [Baked container commands](#baked-container-commands)
  - [Host scripts](#host-scripts)
  - [`wtf` commands](#wtf-commands)
- [Configuration — files you edit by hand](#configuration--files-you-edit-by-hand)
  - [`firewall/domains.txt` — baseline (rarely touched)](#firewalldomainstxt--baseline-rarely-touched)
  - [`firewall/domains.d/<eco>.txt` — per-ecosystem deps (committed)](#firewalldomainsdecotxt--per-ecosystem-deps-committed)
  - [`firewall/domains.local.txt` — your personal overrides](#firewalldomainslocaltxt--your-personal-overrides)
  - [`firewall/ports.txt` — non-HTTP services](#firewallportstxt--non-http-services)
  - [`firewall/policy.d/<host>.yaml` — advanced L7 rules](#firewallpolicydhostyaml--advanced-l7-rules)
  - [`firewall/policy.local.d/<host>.yaml` — your personal L7 overrides](#firewallpolicylocaldhostyaml--your-personal-l7-overrides)
  - [`firewall/default-mode` and `.env`](#firewalldefault-mode-and-env)
  - [`hooks/` — lifecycle overlay](#hooks--lifecycle-overlay)
  - [`devcontainer.json`](#devcontainerjson)
- [Workflows](#workflows)
  - [Daily dev → PR](#daily-dev--pr)
  - [Blocked install → `firewall-blocks` → `domains.d/<eco>.txt`](#blocked-install--firewall-blocks--domainsdecotxt)
  - [Long-running script → `/watch-log`](#long-running-script--watch-log)
- [GitHub authentication](#github-authentication)
- [Shared Claude credentials across projects](#shared-claude-credentials-across-projects)
- [Troubleshooting (quick pointers)](#troubleshooting-quick-pointers)
- [FAQ](#faq)
- [See also](#see-also)

## Port forwarding (dev servers)

By default this devcontainer **does not auto-forward** any port — VS Code's `otherPortsAttributes` is set to `ignore` in [devcontainer.json](devcontainer.json) so a process listening inside the container does not silently expose itself to the host. Ports detected in `LISTEN` state appear in the VS Code **Ports** panel under *Not Forwarded* and have to be activated by hand (right-click → *Forward Port*).

To **whitelist a port** for automatic forwarding — typical for a long-running dev server you start every time — uncomment the `portsAttributes` block in `.devcontainer/devcontainer.json` and adapt the port number / label :

```jsonc
"portsAttributes": {
  "5173": { "label": "client dev", "onAutoForward": "silent" }
},
"otherPortsAttributes": { "onAutoForward": "ignore" }
```

`onAutoForward` accepts `"silent"` (forward without notification), `"notify"` (toast on detect), or `"openBrowser"` (forward + auto-open). Reload the window (or rebuild) for changes to apply.

### Example — client dev server via `wtf`

The image bakes `wtf` as the project task runner (authoring guide: `/opt/devcontainer/base/knowledge/wtf.md`). Drop a `.wtfcmd.yaml` at the repo root to expose your client compile / dev-server commands :

```yaml
- name: client
  desc: Client (frontend) tasks
- name: dev
  group: client
  desc: Start the client dev server on :5173
  cmd: cd client && npm run dev -- --host 0.0.0.0 --port 5173
- name: build
  group: client
  desc: Build the client for production
  cmd: cd client && npm run build
```

Then `wtf client dev` from any container shell starts the Vite / webpack dev server, and once `devcontainer.json` whitelists port `5173` (block above), VS Code forwards it to the host — open `http://localhost:5173` in your host browser.

> The `--host 0.0.0.0` flag is critical : a dev server bound to `127.0.0.1` only is unreachable through Docker's port forward. Bind to `0.0.0.0` (or `::`) so the container's listener accepts proxied connections from the VS Code port-forward.

## TL;DR

- **Published image, thin project layer**: the `Dockerfile` is `FROM ${BASE_IMAGE}` plus this project's own RUN blocks. Nothing is built locally except that layer; `BASE_IMAGE=` in `.env` is the pin and the rollback knob
- **Default-deny outbound**: only the allowlisted hosts resolve. This tree runs `basic` (DNS allowlist, no L7 filter)
- **No push, no `gh` write**: PRs are drafted in-container (`/prepare-pr`), the human runs `gh pr create` on the host
- **Allowlist is baked**: edit `firewall/`, then Rebuild Container. In `basic` mode `wtf firewall reload` (host) hot-reloads the local layer for the life of the container
- **Lifecycle = `devc-hook`**: baked fragments under `/opt/devcontainer/base/hooks/<phase>.d/` plus this tree's overlay `hooks/<phase>.d/`

## The two-container model

```
┌─────────────────────────────────────────────────────────────────────┐
│  app — daily work                                                   │
│                                                                      │
│  • Default-deny firewall (DNS + ipset; + mitmproxy L7 in strict)   │
│  • POST allowed: api.anthropic.com, *.statsig.com, sentry.io,       │
│    github.com/anthropics/*.git/git-upload-pack                       │
│  • git commit local OK · git push BLOCKED · gh write BLOCKED        │
│  • mitmproxy binary baked · CA per-project volume                   │
│  • Skills: /prepare-pr · /watch-log · /prepare-plan · /tokens …     │
│  • DOCKER_HOST=tcp://dind:2375                                       │
└─────────────────────────────────────────────────────────────────────┘
                          │ compose-internal network
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  dind — nested Docker daemon (sidecar, starts with the stack)       │
│                                                                      │
│  • Serves the agent-side image suites (wtf image test /            │
│    release-check) from a daemon whose container list does NOT       │
│    include app · own network stack (app's ruleset does not apply)   │
│  • Same /workspace bind-mount as app (load-bearing for the suites)  │
└─────────────────────────────────────────────────────────────────────┘
                          ▲ workspace bind-mount R/W
┌─────────────────────────────────────────────────────────────────────┐
│  HOST — trust boundary, gates outbound writes                       │
│                                                                      │
│  • Holds GitHub PAT, SSH key, OAuth tokens                          │
│  • Runs: gh pr create (from a /prepare-pr draft), claude-switch,    │
│          wtf firewall reload, docker-reclaim.sh, the notify daemon  │
└─────────────────────────────────────────────────────────────────────┘
```

The host is the only place that holds long-lived secrets. The `app` container cannot push to GitHub even if Claude is fully prompt-injected. Compose commands for this stack are **host** commands only: typed inside `app`, `docker compose up` retargets at `dind` and builds a dind-in-dind. See [SECURITY](docs/SECURITY.md) for the threat model.

## Quick start

1. **Open in container** — VS Code → `Dev Containers: Reopen in Container`
2. **Host-side initialize** — `initialize.sh` is a shim that finds a node ≥ 18 and runs `npx @meitogi/devcontainer-cli@0.x initialize`: it creates `.env` from `.env.example` if missing, records the Claude mode (`dev` / `reviewer`) in `tmp/configured/claude-mode`, creates the external credentials volume, and starts the host-side notify daemon. Make the CLI a root devDependency (`npm i -D @meitogi/devcontainer-cli`) so `npx` runs the local copy instead of downloading at every window open
3. **Compose pulls the base image** and builds the thin project layer (the firewall bake stage freezes `firewall/` into `/etc/devcontainer-firewall/`)
4. **Lifecycle phases run** — `devc-hook on-create`, `post-create`, `post-start` (see below). The boot panel closes `post-start`
5. **First terminal** — the baked shell-init sets the CA env vars and prints the cached boot panel. `gh auth login` is manual (see [GitHub authentication](#github-authentication))
6. **Validate** — `sudo /usr/local/bin/test-firewall.sh` in the container, then `bash .devcontainer/tests/run.sh`

## Container lifecycle

```
            initializeCommand                       onCreateCommand
HOST ─────► initialize.sh ───────────► CONTAINER ─► devc-hook on-create ──┐
            (shim → devc initialize:    starts      10-firewall-init      │
             .env, claude-mode flag,                45-ext-patches        │
             creds volume, notify daemon)                                 │
                                                                          ▼
            postCreateCommand                                             │
       ┌──► devc-hook post-create ◄────────────────────────────────────────┘
       │    10-symlink-claude-mode · 20-seed-settings-local
       │    30-warn-test-root · 40-test-firewall
       │
       │    postStartCommand                        every container restart
       ├──► devc-hook post-start ─────────────────► 05-log-rotation · 20-firewall-reinit
       │                                            banners (22/25/30/35/40/42/45)
       │                                            55-claude-creds-sync · 60-claude-json-sync
       │                                            74-settings-backup* · 75-skills-sync
       │                                            80-watch-log-cleanup · 85-merge-creds-hooks
       │                                            86-merge-backup-state-hook*
       │                                            90-install-extensions-safety · 95-boot-summary
       │
       │    sourced by .zshrc/.bashrc               every terminal
       └──► baked shell init (/opt/devcontainer/ ─► CA env, creds-conflict prompt,
            base/) — no copy in this tree           cached boot panel
                                                    (* = this tree's overlay)
```

`devc-hook <phase>` enumerates `/opt/devcontainer/base/hooks/<phase>.d/` (image), `/opt/devcontainer/ext/hooks/` (an extending image, unused here) and `.devcontainer/hooks/<phase>.d/` (this project), dedups by filename (overlay wins), sorts by numeric prefix and runs them. `devc-hook post-start --dry-run` lists what would run without running it. Each phase writes `.devcontainer/tmp/logs/<phase>-<ts>.log` (rotated after 7 days by `05-log-rotation`). Every fragment is idempotent — replay a phase mid-session without harm.

### Boot panel and banners

`95-boot-summary` renders the closing panel (`boot-summary`) once and caches it in `.devcontainer/tmp/boot-summary.txt`; every new terminal prints the cached text. Above it, a few one-line signals; none are blocking, each names the action to take. Full catalogue: `/opt/devcontainer/base/docs/boot-warnings.md`.

| Trigger | Banner | What to do |
|---|---|---|
| `/etc/claude-fallback-warn` exists | Yellow — Claude binary: npm fallback active | Read `/etc/claude-source`; pick another published `BASE_IMAGE` tag if it persists ([RUNBOOK § Bump](RUNBOOK.md#13-bump-claude-code-or-the-base-image)) |
| `registry.npmjs.org/@anthropic-ai/claude-code` reports a newer version | Yellow 1-line — Claude Code X available | Wait for a published `<base>-cc<X>` tag, then bump `BASE_IMAGE` |
| Image carries no baked firewall, or baked ruleset ≠ sources | Red — firewall bake warning | Rebuild Container; the bake is what makes the ruleset immutable |
| `domains.local.txt` / `policy.local.d/` present but not baked | Yellow — local overrides STAGED, NOT ACTIVE | `reload-firewall --dry-run` to preview, `wtf firewall reload` (host) to apply, or `FIREWALL_ALLOW_LOCAL_AT_REBUILD=1` in `.env` + Rebuild |
| Firewall mode = `basic` | Yellow — BASIC mode (DNS allowlist only, no L7 filter) | Expected in this tree. To harden: `strict` in `firewall/default-mode` + `.env` sync + Rebuild ([RUNBOOK § Switch mode](RUNBOOK.md#4-switch-firewall-mode)) |
| Patchers: an apply would change the extension / nothing cached for this line | Yellow — Patchers | `ext-patches-sync --status`, then `wtf ext-patch update` ([VS Code extension patchers](#vs-code-extension-patchers)) |
| `anthropic.claude-code` pinned in `devcontainer.json`, or baked extension missing / duplicated | Red or yellow — extension consistency sentinel | Remove the pin; the image bakes the extension |
| `/tmp/.claude-creds-conflict` present (sync detected divergence) | Yellow interactive prompt at next terminal | Decide which side wins ([RUNBOOK § Inspect Claude OAuth](RUNBOOK.md#10-inspect--rotate-claude-oauth-credentials)) |

## Directory layout

```
.devcontainer/
├── Dockerfile                  ARG BASE_IMAGE=ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.280
│                               stage 1: firewall bake (COPY firewall/ → firewall-docker-setup.sh → /out)
│                               final: FROM ${BASE_IMAGE} + COPY --from=fw-bake + this project's RUN blocks
│                               (anim tools, docker CLI, rust, zig + cargo-zigbuild, cargo-xwin)
├── Dockerfile.AndroidMin / .AndroidStd / .CapacitorAndroidMin / .CapacitorAndroidStd
│                               stack variants, adopted via compose build.dockerfile
├── HOW-TO-CAPACITOR-PLUGIN.md  Capacitor plugin guide for the variants above
├── docker-compose.yml          services app + dind, volumes, sysctls, NET_ADMIN/NET_RAW
├── devcontainer.json           VS Code config + lifecycle (devc-hook on-create|post-create|post-start)
├── initialize.sh               [host] 4-line shim → npx @meitogi/devcontainer-cli@0.x initialize
├── .env / .env.example         BASE_IMAGE pin, DC_PROJECT, CLAUDE_CREDS_VOLUME, firewall + proxy vars,
│                               Ollama routing (claude-switch), EXT_PATCHES_*, NOTIFY_*
├── .wtfcmd.yaml                devcontainer-scoped wtf commands (firewall reload)
│
├── README.md                   ← you are here
├── RUNBOOK.md                  operational procedures (add domain, troubleshoot, reset, bump)
├── docs/SECURITY.md            threat model + accepted gaps, for the basic mode actually running
├── LESSONS.md                  project-wide lessons (root symlink) · LESSONS.local.md gitignored
│
├── hooks/                      lifecycle overlay — fragments merged over the image's by devc-hook
│   └── post-start.d/
│       ├── 74-settings-backup.sh            timestamped copy of ~/.claude/settings.json
│       └── 86-merge-backup-state-hook.sh    Stop/SessionEnd hook → claude/backup-state.sh
│
├── claude/
│   ├── CLAUDE-dev.md           symlinked to /workspace/CLAUDE.md in dev mode (cloud)
│   ├── CLAUDE-reviewer.md      symlinked in reviewer mode
│   ├── CLAUDE-local-dev.md     symlinked when claude-switch is in local (Ollama) mode
│   ├── CLAUDE-project*.md      stack conventions (base, android, android-capacitor)
│   ├── backup-state.sh         snapshot ~/.claude + /commandhistory volumes onto the bind mount
│   ├── settings-backups/       written by hooks/post-start.d/74-settings-backup.sh
│   ├── settings.local.json     Claude Code local settings kept with the tree
│   ├── scripts/                gate.mjs (commit gate), wave-run.sh — project-local
│   └── vscode-ext-patchs/      model-timing-probe.py + README — the other patchers come from the
│                               pinned tarball of meitogi/claude-ext-patchs at boot
│
├── firewall/                   COPYed into the image at build (bake) — edit, then Rebuild
│   ├── CLAUDE.md               reading rules for the agent (basic = host granularity only)
│   ├── default-mode            basic   ← this tree's mode (strict | basic | off)
│   ├── domains.txt             baseline allowlist (Claude-only hosts)
│   ├── domains.d/              per-ecosystem additive, committed, maintained by hand:
│   │                           npm.txt, ecosystem-docs.txt, rust.txt, zig.txt, xwin.txt
│   ├── domains.android.txt / domains.capacitor-android.txt   used by the stack variants
│   ├── domains.local.txt       per-dev overrides (gitignored) · .example = starter pack
│   ├── ports.txt               non-HTTP host:port entries (host = Docker host gateway)
│   ├── policy.d/               project-specific L7 rules only: ollama.internal.yaml
│   │                           (the image ships the Anthropic / GitHub / npm / VS Code ones)
│   ├── policy.local.d/         per-dev L7 overrides (gitignored) · policy.local.d.example/ templates
│   └── effective/, baked-at    bake output, gitignored — never a source
│
├── skills/                     project-local skills only (hours.local, master-review.local, …)
│                               + disabled.txt to opt out of a baked skill
├── host-helpers/               host-side wrappers (see § Host helpers)
├── tests/                      run.sh, lib.sh, README.md, validate-claude-switch.sh,
│                               integration/test-claude-switch.sh
│
├── pr-drafts/                  output of /prepare-pr (gitignored except .keep)
└── tmp/                        everything a machine writes — gitignored, `rm -rf tmp/` is safe:
    ├── logs/                   devc-hook phase logs (<phase>-<ts>.log), host-helper logs
    ├── boot-summary.txt        cached boot panel
    ├── configured/claude-mode  dev | reviewer
    ├── pending/                /watch-log scripts + logs (purged past 60 min)
    └── cache/ext-patchs/<ref>/ patcher tarball cache
```

Baked beside these, in the image: `/opt/devcontainer/base/hooks/` (fragments), `/opt/devcontainer/base/skills/` (the 9 shipped skills), `/opt/devcontainer/base/knowledge/` (INDEX, firewall, firewall-reload-local, docker-base-image, extension-points, ollama-local, wtf, cache-and-cost, workspace-mount), `/opt/devcontainer/base/docs/` (how-tos), and the firewall infrastructure (`compile-policy.py`, `mitm-init.sh`, addons, dnsmasq.conf) at `/usr/local/bin/` and `/etc/devcontainer-firewall/`.

## Firewall modes

| Mode | Stack | Outbound | Use case |
|---|---|---|---|
| `strict` | DNS + iptables ipset + mitmproxy + 4 addons + IPv6 lockdown | Only through mitmproxy (`HTTPS_PROXY=http://127.0.0.1:8080`), iptables UID-matches `mitmproxy` | Path/method/body-size enforcement |
| **`basic`** (this tree) | DNS + iptables ipset only | Direct from any UID to allowlisted hosts; **an allowlisted host accepts all paths** | Daily work here; when an app refuses HTTPS_PROXY |
| `off` | None (kill-switch) | Direct internet, no allowlist | Emergencies only (the boot panel reports it) |

Deprecated aliases (still accepted with a stderr warn): `okeish` → `basic`, `paranoid` → `strict`.

```
strict mode:

  node app
   │ HTTPS_PROXY=http://127.0.0.1:8080
   ▼
  mitmproxy :8080  (UID=mitmproxy)
   │  addons: policy_enforce + format_detect + passive_log + stream_sse
   │  resolves via 127.0.0.53 (dnsmasq, UID-restricted)
   ▼
  iptables OUTPUT: ACCEPT only if UID=mitmproxy AND dst ∈ ipset allowed-domains
   │
   ▼
  internet (allowlisted hosts only)

  App that bypasses HTTPS_PROXY → REJECT (UID mismatch)
  IPv6 outbound                 → DROP (sysctl + ip6tables)
  DNS to 8.8.8.8 / DoT / DoH    → DROP (UDP/53 limited to UID=dnsmasq)
```

The mode is read at boot from `/etc/devcontainer-firewall/default-mode` — the baked copy of `firewall/default-mode`. Switch with `npx @meitogi/devcontainer-cli firewall-mode <off|basic|strict>` on the host (writes `firewall/default-mode` and aligns the proxy/CA variables in `.env`; no argument reports the mode), then VS Code → `Dev Containers: Rebuild Container`. See [RUNBOOK.md § Switch mode](RUNBOOK.md#4-switch-firewall-mode).

## Local hosts — DNS-driven aliases (no `extra_hosts`)

The firewall's allowlist is **DNS-driven** : every authorized host is declared in `firewall/domains.txt` (or `domains.local.txt`), dnsmasq resolves it, and `ipset=/host/allowed-domains` directives auto-populate the kernel ipset with the returned IPs. iptables then accepts traffic to any IP in that ipset.

This pipeline relies on the client *actually making a DNS query*. `extra_hosts` in `docker-compose.yml` would inject an entry into `/etc/hosts`, and `nsswitch.conf` (which has `files dns`) consults `/etc/hosts` **before** dnsmasq — short-circuiting the resolver and leaving the ipset unpopulated. The traffic might still work (depending on iptables defaults), but the firewall *doesn't know about it*, the audit log skips it, and `test-firewall.sh` reports a confusing `❌ DNS resolution failed`.

For internal aliases that need to point at the host gateway (`ollama.internal`, `ollama.local`), the baked `init-firewall.sh` therefore builds a **CNAME chain inside dnsmasq** :

1. It resolves `host.docker.internal` via Docker's internal resolver (`127.0.0.11`) at boot — the IP is runtime-assigned so it is captured dynamically.
2. It appends to the generated dnsmasq config :
   ```
   host-record=host.docker.internal,<IP>
   cname=ollama.internal,host.docker.internal
   cname=ollama.local,host.docker.internal
   ipset=/host.docker.internal/allowed-domains
   ```
3. Client query for `ollama.internal` → dnsmasq returns CNAME → resolves locally via the `host-record` → returns the IP **and** adds it to `allowed-domains`.
4. `curl http://ollama.internal:11434/...` works directly from the terminal post-rebuild ; `test-firewall.sh` reports `✔ ollama.internal reachable`.

For every `host:port` entry of `firewall/ports.txt`, `init-firewall.sh` also emits a `<host>.local` CNAME (sibling-resolve for compose services) and `test-firewall.sh` probes the TCP port. A new alias on the host gateway itself (e.g. `mysql.internal`) needs the CNAME block in the image's `init-firewall.sh`, which this tree does not carry — declare the service as `host:3306` in `ports.txt` and reach it as `host.docker.internal` instead. Full Claude-Code-local example: `/opt/devcontainer/base/knowledge/ollama-local.md`.

## Local backends — switch Claude to Ollama

Claude Code in the container can talk to Anthropic's cloud (default) or to a host-side Ollama backend, via the host helper [`host-helpers/claude-switch`](host-helpers/claude-switch). Two modes :

| Mode | Endpoint | When to use |
|---|---|---|
| `cloud` | `api.anthropic.com` | Default. Anthropic SDK route. |
| `local` | `http://ollama.internal:11434` | Raw Ollama. Non-reasoning models only — reasoning models (qwen3, deepseek-r1) hang Claude Code on the raw `<think>` blocks, and nothing in this tree translates them any more. |

Quick setup (host-side) :

```bash
bash .devcontainer/host-helpers/claude-switch status   # what mode is active right now
bash .devcontainer/host-helpers/claude-switch local    # switch to local Ollama
bash .devcontainer/host-helpers/claude-switch cloud    # back to Anthropic cloud
```

The helper runs on the host on purpose (toggling the in-container endpoint from outside keeps the threat boundary clean). It edits `.devcontainer/.env` (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CONFIG_DIR`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`), syncs `firewall/ports.txt` (`host:11434` for local, nothing for cloud) and repoints `CLAUDE.md` to `CLAUDE-local-dev.md` or back. After the switch : **Rebuild Container** in VS Code — `.env` and `firewall/` are both read at container creation only (Reload Window won't refresh PID 1's env or the baked ruleset).

Prereq for `local` : Ollama running on the host. Pick a hardware profile + model and start it with one of the tuned helpers ([`ollama-serve-16k`](host-helpers/ollama-serve-16k), [`ollama-serve-32k`](host-helpers/ollama-serve-32k)).

**Full guide** (install Ollama, hardware profiles, model choices, tuning, audit, troubleshooting) : `/opt/devcontainer/base/knowledge/ollama-local.md`. Validate a switch with `bash .devcontainer/tests/validate-claude-switch.sh` after each rebuild.

## Claude Code and the base image

Claude Code — the CLI binary and the VS Code extension — is **baked into the published base image**. The image tag carries both versions:

```
ghcr.io/meitogi/devcontainer-sandbox:<base-version>-cc<claude-code-version>
                                     1.9.1-cc2.1.280     ← this tree's default
```

The pin lives in one place, `.devcontainer/.env`:

```bash
#BASE_IMAGE=ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.280   # commented = the Dockerfile default
```

Compose passes it as the `BASE_IMAGE` build arg; `Dockerfile` does `FROM ${BASE_IMAGE}`. The pairs published together are listed in the image repo's `cc-versions.json`. `anthropic.claude-code` is deliberately **not** pinned in `devcontainer.json`: the image registers the baked extension itself, and a pin is the one path by which an unpatched Marketplace copy could arrive (the `42-claude-ext-pin-warn` fragment banners if one reappears).

The image keeps its build-time failsafe: when the extension's embedded binary could not be symlinked at build, `/usr/local/bin/claude` falls back to the npm CLI, `/etc/claude-fallback-warn` is touched and `/etc/claude-source` says why. The `40-claude-fallback-warn` fragment surfaces it at boot; the fix is another published tag, not a local rebuild.

### Bumping Claude Code or the base

```bash
# 1. Edit .devcontainer/.env — pick a published tag
BASE_IMAGE=ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.272

# 2. VS Code → Dev Containers: Rebuild Container (compose pulls the tag)

# 3. Verify
claude --version                 # → the cc<version> of the tag
cat /etc/claude-source           # → extension:<path> ideally
boot-summary                     # Claude / Firewall / Patchers rows
```

Rollback is the same edit with the previous tag. The `45-claude-update-probe` fragment prints a yellow line when `registry.npmjs.org` publishes a newer Claude Code than the one installed; it is informational until a matching `-cc` tag is published. Full step-by-step: [RUNBOOK § Bump](RUNBOOK.md#13-bump-claude-code-or-the-base-image).

### VS Code extension patchers

The image installs the extension exactly as published. The `45-ext-patches` fragment (on-create, re-checked at every post-start) applies **this operator's** patchers on top: the pinned tarball of `EXT_PATCHES_REPO` (`meitogi/claude-ext-patchs`, read with `EXT_PATCHES_TOKEN`), at `EXT_PATCHES_REF` — unset / `auto` = the newest `cc<version>-r<n>` tag for the installed Claude Code — merged with this tree's own `claude/vscode-ext-patchs/model-timing-probe.py`. `ext-patches-sync --status` says what is configured, cached and live; `wtf ext-patch update` (= `ext-patches-update`) moves to a newer `-r` deliberately and rewrites the pin. A VS Code window reload is needed for a change to reach the extension host.

## Host helpers (debug + sanity)

Host-side tools complementing the baked container commands. All refuse to run inside the container (they need `docker` against the host daemon, or they read the workspace from outside the bind mount).

| Helper | Use case |
|---|---|
| [`host-helpers/claude-switch`](host-helpers/claude-switch) | Toggle Claude Code between `cloud` and `local` Ollama — edits `.env`, `firewall/ports.txt`, repoints `CLAUDE.md`. `{local\|cloud\|status}`. **Rebuild Container** after. Full guide: `/opt/devcontainer/base/knowledge/ollama-local.md` |
| [`host-helpers/ollama-serve-16k`](host-helpers/ollama-serve-16k) / [`-32k`](host-helpers/ollama-serve-32k) | Launch `ollama serve` host-side with env vars tuned for the Compact (16 GB Mac) / Balanced (32 GB Mac) profile : `OLLAMA_FLASH_ATTENTION=1`, `OLLAMA_KV_CACHE_TYPE=q8_0`, `OLLAMA_CONTEXT_LENGTH=16384/32768`, `OLLAMA_KEEP_ALIVE=30m/1h`, `OLLAMA_MAX_LOADED_MODELS=1`, `OLLAMA_NUM_PARALLEL=1`. Quit the desktop app first (port conflict) |
| [`host-helpers/audit-claude-code-proxies`](host-helpers/audit-claude-code-proxies) | Freshness table of candidate Claude Code → local-model proxies (GitHub API, unauthenticated) — written to `tmp/logs/proxy-audit-<ts>.log` |
| [`host-helpers/mitm-capture`](host-helpers/mitm-capture) | Toggle the `capture_messages_debug.py` mitmproxy addon — dumps every POST `/v1/messages*` body + a replay-curl script into `/tmp/claude-capture/`. Live sentinel (no restart). `{on\|off\|status\|ls\|clear}`. Strict mode only (no mitmproxy in basic). Details : `/opt/devcontainer/base/knowledge/extension-points.md` |
| [`host-helpers/docker-audit.sh`](host-helpers/docker-audit.sh) | Size, layer sharing and disk waste of this project's devcontainer images (parses `Dockerfile*` + compose for the tags) |
| [`host-helpers/docker-reclaim.sh`](host-helpers/docker-reclaim.sh) | Sweep the whole host daemon for reclaimable junk — never touches a tagged image (`wtf docker reclaim`; `wtf docker usage` for the readout) |
| [`host-helpers/docker-test-images.sh`](host-helpers/docker-test-images.sh) | Remove the tagged images this repo's own test runs leave behind, and nothing else |
| [`host-helpers/open-devcontainer`](host-helpers/open-devcontainer) | Open a folder in its dev container from the command line (`--uri` prints the URI) |

## Commands and skills

Slash commands come from the image (`/opt/devcontainer/base/skills/`) and from `.devcontainer/skills/` (project overlay); the baked `sync-skills` installs both into `~/.claude/commands/` at every start, overlay winning by filename, and merges their `hooks.json` into `~/.claude/settings.json`.

### Skills shipped by the image

| Slash command | Purpose |
|---|---|
| `/prepare-pr` | Generate a PR draft pair (`.md` body + `.yaml` metadata) under `pr-drafts/` — the host runs the actual `gh pr create` |
| `/watch-log` | Generate a script in `tmp/pending/<id>.sh` that ends with an `__END__` sentinel + drive Bash background or Monitor stream |
| `/prepare-plan` | Scaffold a multi-session rollout directory (ROLLOUT + STATUS + LOG + EXISTING + sessions/) for features that need ≥3 sessions |
| `/prepare-stack` | Wire a project onto the base image: extending Dockerfile, allowlist discovered from `firewall-blocks`, lifecycle fragments |
| `/visual-loop` | Figma ↔ running-app fidelity loop (`wtf claude-script` / `wtf claude-live` tooling) |
| `/diagram` | `.excalidraw` from a description, rendered to SVG/PNG headlessly |
| `/tokens` | Token consumption recap for the session |
| `notify-queue`, `session-gap` | Hook-only skills: desktop-notification body (§14 of CLAUDE.md) and the > 1 h cold-cache signal |

To opt out of a baked skill, list it in `skills/disabled.txt` (one name per line); to replace one, ship a same-named skill in `skills/`.

### Project-local skills (gitignored)

Skills suffixed `.local/` are personal — not shared across the team. Present in this tree:

| Slash command | Purpose | Status |
|---|---|---|
| `/hours`, `/hours-calibrate` | Time estimation for tasks (hours-to-value) + market-data calibration | local opt-in |
| `/master-review` | Multi-agent PR review with tier scoring and metrics | local opt-in |

### Anthropic global skills

Provided by Claude Code itself, available everywhere. Selected entries relevant in this devcontainer:

| Slash command | Purpose |
|---|---|
| `/init` | Generate or refresh `CLAUDE.md` from a codebase analysis |
| `/review` | Review a PR or diff |
| `/security-review` | Security-focused review of pending changes |
| `/simplify` | Review code for reuse, quality, efficiency |
| `/run` | Launch the project's app to observe a change live |
| `/loop` | Run a prompt or skill on a recurring interval |
| `/schedule` | Create cron-based remote agents (routines) |
| `/claude-api` | Build / debug / migrate code that uses the Anthropic SDK |
| `/update-config` | Configure `settings.json` (permissions, hooks, env vars) |
| `/keybindings-help` | Customize Claude Code keyboard shortcuts |
| `/fewer-permission-prompts` | Reduce permission prompts by adding common allowlist entries |

### Baked container commands

All on `PATH` (`/usr/local/bin/`), shipped by the image — this tree carries no copy.

| Command | Purpose | Invoke |
|---|---|---|
| `devc-hook <phase> [--dry-run]` | Lifecycle dispatcher (`on-create`, `post-create`, `post-start`) | `devc-hook post-start` replays a phase |
| `init-firewall.sh` | Apply the baked firewall mode — dnsmasq + iptables (+ mitmproxy in strict). Re-run at every start by `20-firewall-reinit` | `sudo /usr/local/bin/init-firewall.sh` (NOPASSWD sudoers entry) |
| `test-firewall.sh` | Connectivity smoke test (DNS allowlist, `ports.txt` TCP probes, `ollama.internal`) | `sudo /usr/local/bin/test-firewall.sh` |
| `reload-firewall` | Merge `domains.local.txt` + `policy.local.d/` into the LIVE ruleset — basic mode only, root only, ephemeral until the next start | `reload-firewall --dry-run` (unprivileged preview); apply via `wtf firewall reload` from the host |
| `firewall-blocks [N\|--follow\|--reset]` | Recent L7 refusals with reasons (strict only — empty in basic) | no sudo needed |
| `boot-summary` | Re-render the boot panel | — |
| `sync-skills` / `sync-creds` | Skills install + OAuth sync (called by fragments 75 / 55) | `VERBOSE=1 sync-creds`, `DEBUG=1 sync-creds` |
| `ext-patches-sync [--status\|--force]` / `ext-patches-update` | Extension patchers: apply / inspect / move the pin | `wtf ext-patch status\|update` |
| `install-extensions` | Safety-net VS Code extension install (idempotent, called by fragment 90) | `install-extensions` |
| `outbound-tester` | Drive the extension's outbound control channel (list / answer pending permission requests via `tmp/logs/*.jsonl`) — companion to the outbound-action-injector / webview-simulated-click patchers | `outbound-tester list` |
| `devc-conf.sh` | Parser for the `.txt` list format (`disabled.txt`, allowlists) — library | sourced |

### Host scripts

| Script | Purpose |
|---|---|
| `initialize.sh` | Shim: find node ≥ 18 (nvm/fnm/volta/asdf/… directories), then `exec npx --package=@meitogi/devcontainer-cli@0.x devc initialize`. `DEVC_NODE=/path/to/node` forces one interpreter. Run via VS Code `initializeCommand` |
| `npx @meitogi/devcontainer-cli firewall-mode [mode]` | Report or set the firewall mode: writes `firewall/default-mode` + aligns the proxy/CA vars in `.env` |
| `npx @meitogi/devcontainer-cli migrate` / `init` | Migrate a v2 tree / scaffold a fresh one (already done here) |

### `wtf` commands

Root `.wtfcmd.yaml`: `wtf notif build|build-debug|test|bin-path|log|dev` (the notifier crate), `wtf image test|release-check` (base-image suites against `dind`), `wtf ext-patch status|update`, `wtf token share|promote`, `wtf docker usage|reclaim`. From inside `.devcontainer/`: `wtf firewall reload`. Authoring guide: `/opt/devcontainer/base/knowledge/wtf.md`.

## Configuration — files you edit by hand

Everything under `firewall/` is **baked** at build: edit, then `Dev Containers: Rebuild Container`. The boot panel says `STAGED` when a file on disk is not the ruleset in force.

### `firewall/domains.txt` — baseline (rarely touched)

Claude-only hosts. Don't add to this file casually — adding a host here means every dev gets it. If you need `docs.example.com` only for yourself, use `domains.local.txt` instead. If you need a project dep, add it to `domains.d/<eco>.txt`. Syntax (5 formats):

```
docs.anthropic.com                            # 1. bare = GET only
[GET,POST] api.anthropic.com                  # 2. methods inline, host-wide
[*] api.anthropic.com                         # 3. multi-line, paths inherit methods
  /v1/messages                                #    (2-space indent STRICT)
  /v1/files
POST api.anthropic.com/v1/messages            # 4. single-line path
[GET] api.github.com/repos/anthropics/*       # 5. wildcard (trailing * on path)
[POST] *.statsig.com                          #    or wildcard host (leading *.)
```

Methods and paths are enforced in `strict` only — in `basic`, an allowlisted host accepts every path (see `firewall/CLAUDE.md`). Full reference (precedence, `!disable`, `policy.d/<host>.yaml`): the baked compiler at `/usr/local/bin/compile-policy.py`.

### `firewall/domains.d/<eco>.txt` — per-ecosystem deps (committed)

One file per concern (`npm.txt`, `ecosystem-docs.txt`, `rust.txt`, `zig.txt`, `xwin.txt`), maintained **by hand**. The discovery method is the error message: a blocked install reports `ENOTFOUND` / `Could not resolve host` with the hostname (DNS refusals are silent by design — nothing logs them); in `strict`, `firewall-blocks` lists the L7 refusals. Write a comment above each entry saying what needed it. The file is committed so PR reviewers see the additive surface change. How-to: `/opt/devcontainer/base/docs/how-to/allow-a-domain.md`.

### `firewall/domains.local.txt` — your personal overrides

Same syntax as `domains.txt`. Use this for:
- A doc site you read often but the team doesn't need (`docs.example.com`)
- Disabling baseline telemetry: `!disable *.statsig.com`
- Redefining a host with broader methods (host-level `redefine` wipes baseline paths and replaces methods)

Gitignored, and **not baked by default**: the file is container-writable, so a rebuild would otherwise turn "a postinstall appended a domain" into a wider firewall nobody flagged. Apply it per container with `wtf firewall reload` (host, basic mode, lasts until the next start) after previewing with `reload-firewall --dry-run`, or opt in permanently with `FIREWALL_ALLOW_LOCAL_AT_REBUILD=1` in `.env` (personal setting, never a team default). A starter pack lives in `domains.local.txt.example`.

### `firewall/ports.txt` — non-HTTP services

One `host:port` per line; the bare word `host` means the Docker host gateway, anything else is a compose service name (`db.internal:5432`, `host:11434`). Opens the iptables L4 path and gives `test-firewall.sh` a TCP probe. HTTP(S) services belong in the `domains` files, whatever the port. `claude-switch` maintains the `host:11434` line.

### `firewall/policy.d/<host>.yaml` — advanced L7 rules

One file per host. Specifies `endpoints` (path regex + methods + `max_body_kb`), `defaults_override`, `blocked_paths`, header patterns. Committed. Enforced in `strict` only. This tree carries only `ollama.internal.yaml` — a project-specific host; the Anthropic, GitHub, npm and VS Code marketplace policies ship in the image at `/etc/devcontainer-firewall/policy.d/` and a same-named project file would **replace** the baked one at bake (and freeze it). Example:

```yaml
endpoints:
  - path: "^/v1/messages$"
    methods: [POST]
    max_body_kb: 32768
blocked_paths:
  - "^/v1/admin"
```

### `firewall/policy.local.d/<host>.yaml` — your personal L7 overrides

Same shape, gitignored. Deep-merges into the committed `policy.d/<host>.yaml`. Same bake rule as `domains.local.txt`.

### `firewall/default-mode` and `.env`

| File | Holds | Reset / change |
|---|---|---|
| `firewall/default-mode` | `strict` / `basic` / `off` — baked to `/etc/devcontainer-firewall/default-mode` | `npx @meitogi/devcontainer-cli firewall-mode <mode>` (also aligns `.env`), then Rebuild |
| `tmp/configured/claude-mode` | `dev` / `reviewer` — `10-symlink-claude-mode` picks `CLAUDE-dev.md` or `CLAUDE-reviewer.md` | `rm` it and reopen: `devc initialize` re-prompts |
| `.env` | `BASE_IMAGE`, `DC_PROJECT`, `CLAUDE_CREDS_VOLUME`, proxy/CA vars, `CLAUDE_CODE_FIREWALL_ALLOWED`, `FIREWALL_ALLOW_LOCAL_AT_REBUILD`, Ollama routing, `EXT_PATCHES_*`, `NOTIFY_*`, `DEBUG=1` (per-fragment `.trace` in `tmp/logs/`) | Edit, then Rebuild (compose reads `env_file` at creation) |

### `hooks/` — lifecycle overlay

Drop `hooks/<phase>.d/NN-name.sh` with the `# @name / @phase / @required / @description` header; `devc-hook` merges it with the image's fragments by filename (a same-named file replaces the baked one). `hooks/disabled.txt` lists baked fragments to skip (`post-start.d/25-firewall-mode-banner.sh`; `@required true` fragments need the explicit `!` opt-in). Check with `devc-hook <phase> --dry-run`. How-to: `/opt/devcontainer/base/docs/how-to/add-a-lifecycle-hook.md`.

### `devcontainer.json`

- `customizations.vscode.extensions`: eslint, prettier, gitlens, markdown-code-copy-button — **no** `anthropic.claude-code` pin (the image bakes it)
- `containerEnv`: `NODE_OPTIONS`, `LANG=C.UTF-8`, `GH_CONFIG_DIR`, `HOST_WORKSPACE_PATH`. `CLAUDE_CONFIG_DIR` is deliberately left to `.env` so `claude-switch` can toggle it
- Lifecycle: `initializeCommand` → `initialize.sh`; `onCreateCommand` / `postCreateCommand` / `postStartCommand` → `devc-hook <phase>`

When you change extensions or env, `Rebuild Container` is required (not `Reload Window`).

## Workflows

### Daily dev → PR

```
Claude (in container)            User (on host)              GitHub
─────────────────────            ──────────────              ──────
git checkout -b feat/xyz
edit code
git commit (local OK)
/prepare-pr ─────────► pr-drafts/<slug>-<ts>.md + .yaml
                       │
                       ▼ user reviews draft
                                  git push -u origin feat/xyz
                                  yq -r .body <draft>.yaml | gh pr create --body-file - …
                                  ─────────────────────────────► PR created
```

The container never pushes. The `.yaml` carries title, base, head, labels and body so a single `yq` call feeds `gh pr create`; the `.md` is the same content for copy-paste.

### Blocked install → `firewall-blocks` → `domains.d/<eco>.txt`

```
npm install / cargo build fails
  ├─► "ENOTFOUND cdn.example.net" / "Could not resolve host"
  │     └─► DNS refusal: the hostname is IN the error message (nothing else logs it)
  │         getent hosts cdn.example.net        → empty = not allowed
  └─► 403 from a host that resolves (strict only)
        └─► firewall-blocks                     → reason, host, path → policy.d/<host>.yaml

Add at the right scope: domains.local.txt (yours) · domains.d/<eco>.txt (team, committed) · domains.txt (fundamental)
Rebuild Container (or, basic mode, wtf firewall reload for the local layer)
Check: getent hosts <host> · boot-summary (Firewall count went up, "baked in") · firewall-blocks quiet
```

The committed `domains.d/<eco>.txt` shows up in PR review — every allowlist change is auditable.

### Long-running script → `/watch-log`

```
Claude needs to run a build / test / install that takes minutes.
  └─► /watch-log
        ├─► generates .devcontainer/tmp/pending/<id>.sh with trap "echo __END__" EXIT
        └─► proposes to user:
              bash .devcontainer/tmp/pending/<id>.sh > .devcontainer/tmp/pending/<id>.log 2>&1

User runs the command in another terminal.
Claude meanwhile:
  Pattern A (single notification):  Monitor tailing the log with grep `__END__|FATAL`
                                    (fallback if Monitor unavailable: Bash run_in_background
                                     with `until grep __END__`)
  Pattern B (live stream):          Monitor tailing the log with grep filter
```

Stale pending files (>60 min) are dropped by the baked `80-watch-log-cleanup` fragment at every start.

## GitHub authentication

`gh auth login` is **manual**: run it in a container terminal (OAuth device flow) when you first need `gh`; the token lands in `GH_CONFIG_DIR=/home/node/.claude/gh`, on the `claude-config` volume, so it survives rebuilds. Whatever scopes the token has are what the container gets for *read* operations — writes (`git push`, `gh pr create`) are refused by the firewall regardless, by design: PR creation goes through `/prepare-pr` → host.

## Shared Claude credentials across projects

Set `CLAUDE_CREDS_VOLUME=claude-creds-shared` in `.env` for each project (the volume is `external: true`; `devc initialize` creates it if missing). The `55-claude-creds-sync` fragment runs the baked `sync-creds` at every start: `.credentials.json` is copied between the shared volume and the local `~/.claude` — most-recently-refreshed `expiresAt` wins, both-valid-but-different flags `/tmp/.claude-creds-conflict` for the next terminal. `85-merge-creds-hooks` registers `sync-creds` as a Claude Code `Stop` / `SessionEnd` hook so a mid-session refresh reaches the shared volume. Bidirectional logic: `/opt/devcontainer/base/knowledge/INDEX.md` § Claude OAuth sync flow.

This tree adds `86-merge-backup-state-hook`: the same hook events also run `claude/backup-state.sh`, which snapshots `~/.claude` (transcripts, memory, plans) and `/commandhistory` into `.devcontainer/.state-backup/` on the bind mount — the one place a `docker compose down -v` cannot reach. `backup-state.sh --list` / `--restore [archive]`.

## Troubleshooting (quick pointers)

| Symptom | First check | Procedure |
|---|---|---|
| `curl X.example.com` blocked | `cat /etc/devcontainer-firewall/default-mode` · `getent hosts X.example.com` | [RUNBOOK § Troubleshoot curl](RUNBOOK.md#3-troubleshoot-a-blocked-curl) |
| Edited `firewall/`, nothing changed | boot panel says `STAGED` | Rebuild Container, or `wtf firewall reload` (basic, local layer) — [RUNBOOK § Add a domain](RUNBOOK.md#1-add-a-read-only-domain) |
| Yellow Claude fallback banner at boot | `cat /etc/claude-source` | Another published `BASE_IMAGE` tag — [RUNBOOK § Bump](RUNBOOK.md#13-bump-claude-code-or-the-base-image) |
| Newer Claude Code available banner | `BASE_IMAGE` in `.env` vs the image repo's `cc-versions.json` | [RUNBOOK § Bump](RUNBOOK.md#13-bump-claude-code-or-the-base-image) |
| Model selector is stock / a patch is missing | `ext-patches-sync --status` | [RUNBOOK § Update patchers](RUNBOOK.md#14-update-the-extension-patchers) |
| VS Code extension missing | `install-extensions` | [RUNBOOK § Reinstall extensions](RUNBOOK.md#9-reinstall-vs-code-extensions) |
| Claude OAuth conflict prompt | `/tmp/.claude-creds-conflict` | [RUNBOOK § Inspect Claude OAuth](RUNBOOK.md#10-inspect--rotate-claude-oauth-credentials) |
| `mitmproxy CA invalid` (strict) | volume `mitmproxy-${DC_PROJECT}` | [RUNBOOK § Regenerate CA](RUNBOOK.md#6-regenerate-mitmproxy-ca) |
| A fragment of mine never runs | `devc-hook post-start --dry-run` · `hooks/disabled.txt` | `/opt/devcontainer/base/docs/troubleshooting.md` |
| No desktop notifications | boot panel `Notify` row · `NOTIFY_*` in `.env` | `/opt/devcontainer/base/docs/boot-warnings.md` § Notify |
| `git push` fails | (by design — push from host) | [Daily dev → PR](#daily-dev--pr) |
| Disk full of images | `wtf docker usage` | `wtf docker reclaim`, `host-helpers/docker-test-images.sh` |
| Want to add PHP / Python / other runtime | project `Dockerfile` RUN block | `/opt/devcontainer/base/docs/how-to/add-a-stack.md`; PHP block in the image repo's `stacks/php.md` |

Where the panel is clean and something is still wrong: `/opt/devcontainer/base/docs/troubleshooting.md` (sorted by symptom).

## FAQ

**Can I just `curl docs.example.com`?**
Only if `docs.example.com` is in `domains.txt`, `domains.d/<eco>.txt`, or your `domains.local.txt` — and baked (or reloaded). Otherwise the DNS query returns nothing. Add to `domains.local.txt` for read-only docs.

**Can I `npm install <new-package>`?**
Try it. A missing registry or CDN host fails with `ENOTFOUND` naming the host — add it to `domains.local.txt` (you) or `domains.d/npm.txt` (team), rebuild or `wtf firewall reload`. A postinstall that needs network you do not want to allow is a reason to pick another package.

**Can I `git push`?**
No — the container has no PAT, no SSH key, and `git-receive-pack` POST is not allowlisted. Use `/prepare-pr` → `gh pr create` on the host.

**Can I `gh pr create`?**
No, same reason. Use `/prepare-pr`. The draft includes title, body, base, head, labels in a YAML the host `yq`s.

**Where do I add a new lifecycle hook?**
`hooks/<phase>.d/NN-name.sh` in this tree; choose the phase by what it needs (sudo and no network yet → `on-create`; once per creation → `post-create`; every start → `post-start`). See `/opt/devcontainer/base/docs/how-to/add-a-lifecycle-hook.md`.

**Why is `policy.compiled.yaml` not in the repo?**
It's a build artifact: compiled at bake into `/etc/devcontainer-firewall/effective/` and installed at every boot by `init-firewall.sh` to `/var/run/devcontainer-firewall/`. The committed sources are `domains.txt` + `domains.d/` + `policy.d/` (+ the image's own). Editing the compiled file is futile (overwritten at next boot) and dangerous (no source-of-truth). Same for `firewall/effective/` and `firewall/baked-at` in the tree — gitignored outputs of the bake stage.

**Can I run `docker` inside the container?**
Yes — the CLI talks to the `dind` sidecar (`DOCKER_HOST=tcp://dind:2375`), never the host daemon. Do not run `docker compose` for *this* stack from inside: it would target `dind`.

## See also

- [SECURITY](docs/SECURITY.md) — threat model + accepted gaps, as actually configured (`basic`)
- [RUNBOOK.md](RUNBOOK.md) — operational procedures (add domain, switch mode, reset, bump)
- [HOW-TO-CAPACITOR-PLUGIN.md](HOW-TO-CAPACITOR-PLUGIN.md) — the Android / Capacitor stack variants
- `/opt/devcontainer/base/docs/` — the image's user documentation (getting-started, concepts, boot-warnings, troubleshooting, how-to/)
- `/opt/devcontainer/base/knowledge/INDEX.md` — the image's internals (volumes, OAuth flow, hooks, idempotency contracts, extension points, firewall, Ollama, wtf, cache-and-cost, workspace-mount)
- `firewall/CLAUDE.md` — how to read the allowlist files without inferring path scopes that `basic` does not enforce

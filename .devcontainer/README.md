# DevContainer — Claude Code Sandbox (Niveau 1 strict)

This devcontainer runs Claude Code under a default-deny outbound firewall. Niveau 1 strict (the default) blocks every outbound connection that is not in an explicit allowlist, filters paths and methods via mitmproxy, and bounds POST body sizes per endpoint. The container can read code and talk to `api.anthropic.com`; it cannot push, cannot create PRs, and cannot reach arbitrary third-party APIs.

This README is the maintainer's handbook. Skim it once to understand what's where; consult [RUNBOOK.md](RUNBOOK.md) for step-by-step operations and [SECURITY.md](SECURITY.md) for the threat model. AI-modifiable internals live in [knowledge/INDEX.md](knowledge/INDEX.md).

## Table of contents

- [TL;DR](#tldr)
- [The three-container model](#the-three-container-model)
- [Quick start](#quick-start)
- [Container lifecycle](#container-lifecycle)
  - [Boot banners (post-start.sh)](#boot-banners-post-startsh)
- [Directory layout](#directory-layout)
- [Firewall modes](#firewall-modes)
- [Local backends — switch Claude to Ollama](#local-backends--switch-claude-to-ollama)
- [Claude Code installation (v2.1)](#claude-code-installation-v21)
  - [Failsafe chain — `docker build` never fails on a Claude problem](#failsafe-chain--docker-build-never-fails-on-a-claude-problem)
  - [Bumping Claude Code](#bumping-claude-code)
- [Host helpers (debug + sanity)](#host-helpers-debug--sanity)
- [Variant `Dockerfile.php` (PHP stack)](#variant-dockerfilephp-php-stack)
- [Commands and skills](#commands-and-skills)
  - [Skills committed (project-wide)](#skills-committed-project-wide)
  - [Skills locaux opt-in (gitignored)](#skills-locaux-opt-in-gitignored)
  - [Skills globaux Anthropic](#skills-globaux-anthropic)
  - [Host helpers](#host-helpers)
  - [Container scripts (sudo-friendly)](#container-scripts-sudo-friendly)
  - [Host scripts](#host-scripts)
- [Configuration — files you edit by hand](#configuration--files-you-edit-by-hand)
  - [`firewall/domains.txt` — baseline (rarely touched)](#firewalldomainstxt--baseline-rarely-touched)
  - [`firewall/domains.d/<eco>.txt` — per-ecosystem deps (auto-generated, committed)](#firewalldomainsdecotxt--per-ecosystem-deps-auto-generated-committed)
  - [`firewall/domains.local.txt` — your personal overrides](#firewalldomainslocaltxt--your-personal-overrides)
  - [`firewall/policy.d/<host>.yaml` — advanced L7 rules](#firewallpolicydhostyaml--advanced-l7-rules)
  - [`firewall/policy.local.d/<host>.yaml` — your personal L7 overrides](#firewallpolicylocaldhostyaml--your-personal-l7-overrides)
  - [`.configured-*` flag files](#configured--flag-files)
  - [`devcontainer.json`](#devcontainerjson)
- [Workflows](#workflows)
  - [Daily dev → PR](#daily-dev--pr)
  - [Scope enlargement → research project](#scope-enlargement--research-project)
  - [Dependency audit → `domains.d/<eco>.txt`](#dependency-audit--domainsdecotxt)
  - [Long-running script → `/watch-log`](#long-running-script--watch-log)
- [GitHub authentication](#github-authentication)
- [Shared Claude credentials across projects](#shared-claude-credentials-across-projects)
- [Troubleshooting (quick pointers)](#troubleshooting-quick-pointers)
- [FAQ](#faq)
- [See also](#see-also)

## TL;DR

- **Strict by default**: outbound is `default-deny`, only Anthropic + a small allowlist resolves
- **No push, no `gh` write**: PRs are drafted in-container, the human runs them on the host (`/prepare-pr`)
- **Scope enlargement = separate container**: spawn a research project (`/prepare-research`) rather than weakening the main allowlist
- **Dependencies are audited**: `/scan-deps` writes `firewall/domains.d/<eco>.txt`, committed and reviewed in PR

## The three-container model

```
┌─────────────────────────────────────────────────────────────────────┐
│  MAIN container — daily work, Niveau 1 strict                       │
│                                                                      │
│  • Default-deny firewall (DNS + path + method)                      │
│  • POST allowed: api.anthropic.com, *.statsig.com, sentry.io,       │
│    github.com/anthropics/*.git/git-upload-pack                       │
│  • git commit local OK · git push BLOCKED · gh write BLOCKED        │
│  • mitmproxy binary baked (~80 MB) · CA per-project                 │
│  • Skills: /prepare-pr · /watch-log · /prepare-research ·           │
│            /scan-deps · /prepare-plan                                │
└─────────────────────────────────────────────────────────────────────┘
                          │ workspace bind-mount R/W
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  HOST — trust boundary, gates outbound writes                       │
│                                                                      │
│  • Holds GitHub PAT, SSH key, OAuth tokens                          │
│  • Runs: pr-from-draft, import-research, research-cleanup,          │
│          bring-back-result, watch-log-cleanup                       │
└─────────────────────────────────────────────────────────────────────┘
                          │ user invokes
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  RESEARCH container — spawned on demand, scoped enlargement         │
│                                                                      │
│  • Path: ~/research-projects/<task>/                                │
│  • Niveau 1 strict + policy.local.d/ from bundle (e.g. POST         │
│    api.stripe.com authorized for this research only)                │
│  • Workspace = subset rsync of main                                 │
│  • DC_PROJECT=research-<task> → isolated Docker volumes             │
│  • claude-creds NOT shared (no Anthropic token cross-pollination)   │
└─────────────────────────────────────────────────────────────────────┘
```

The host is the only place that holds long-lived secrets. The main container cannot push to GitHub even if Claude is fully prompt-injected. See [SECURITY.md](SECURITY.md) for the threat model.

## Quick start

1. **Open in container** — VS Code → `Dev Containers: Reopen in Container`
2. **First boot prompts** — `initialize.sh` (runs on the host before build) writes flag files:
   - `.configured-auth` — `standard` (gh auth login on first terminal) vs `advanced` (gh-secure host-side, archived in Phase 3)
   - `.configured-claude-mode` — `dev` (default) vs `reviewer`
   - `.configured-firewall-mode` — `strict` written silently (default since A4)
3. **Container builds**, lifecycle scripts run (see schema below)
4. **First terminal** — `gh auth login` prompts if standard mode, `shell-init.sh` sets CA env vars and prints session banner
5. **Validate** — `bash .devcontainer/tests/diagnose.sh` from the **host** (not from inside the container — the script refuses with exit 2)

## Container lifecycle

```
            initializeCommand                onCreateCommand
HOST ─────► initialize.sh ──────► CONTAINER ─► on-create.sh ──┐
            (menu, flag files,     starts        (firewall      │
             sync .env)                          early init,   │
                                                 sudo)          │
                                                                ▼
            postCreateCommand                                   │
       ┌──► post-create.sh ◄───────────────────────────────────┘
       │    (symlink CLAUDE.md by mode, test-firewall.sh)
       │
       │    postStartCommand                  every container restart
       ├──► post-start.sh ──────────────────► (firewall fallback, sync
       │    (banner, creds sync, scan-deps      creds, install missing
       │    reminder, sync-skills)              extensions safety net)
       │
       │    sourced by .zshrc/.bashrc          every terminal
       └──► shell-init.sh ─────────────────► (CA env, gh device auth,
            (REQUESTS_CA_BUNDLE,                creds conflict prompt,
             SSL_CERT_FILE, …)                  session banner)
```

Each script is idempotent — replay any of them mid-session without harm. See [knowledge/INDEX.md § Idempotency contracts](knowledge/INDEX.md#idempotency-contracts) for the invariants.

### Boot banners (post-start.sh)

A few signals are shown at every container start. None are blocking; all give the user a single action to take if anything is amiss:

| Trigger | Banner | What to do |
|---|---|---|
| `/etc/claude-fallback-warn` exists | Yellow loud banner — Scenario 2 or 3 hit at last build | Read `/etc/claude-source`; if Scenario 2 stays after a rebuild, follow [RUNBOOK § Troubleshoot Claude failsafe](RUNBOOK.md#15-troubleshoot-claude-failsafe-scenarios) |
| `registry.npmjs.org/@anthropic-ai/claude-code` reports a newer version | Yellow 1-line — Claude update available | Edit `.env` `CLAUDE_CODE_VERSION=X.Y.Z` then Rebuild Container ([RUNBOOK § Bump Claude version](RUNBOOK.md#14-bump-claude-version)) |
| Manifest mtime > `scan-deps/.last-scan.json` ts (and `ignored_until` not in future) | Cyan 1-line — `/scan-deps` recommended | Run `/scan-deps` in Claude |
| Local overrides active (`domains.local.txt` or `policy.local.d/`) | Cyan 1-line — N overrides applied | Informational; review with `yq '.runtime._overrides_applied' /var/run/devcontainer-firewall/policy.compiled.yaml` if needed |
| Firewall mode = `basic` (not the default `strict`) | Yellow loud banner — degraded mode active | If unintentional, `bash .devcontainer/firewall-mode.sh strict` + Rebuild Container |
| `/tmp/.claude-creds-conflict` present (sync detected divergence) | Yellow interactive prompt at next terminal | Decide which side wins ([RUNBOOK § Inspect Claude OAuth](RUNBOOK.md#11-inspect--rotate-claude-oauth-credentials)) |

## Directory layout

```
.devcontainer/
├── Dockerfile.base             [v2.1] heavy base image (~1.1 GB) — node:24-bookworm-slim + apt
│                               + mitmproxy binary baked + gh + Claude VSIX + Phase B symlink
│                               + firewall scripts COPY. Tagged claude-devcontainer-base:${VERSION}.
│                               Built once per CLAUDE_CODE_VERSION by initialize.sh.
├── Dockerfile                  [v2.1] slim project layer — FROM claude-devcontainer-base
│                               + project-specific RUNs (empty for Node-only, ~5 MB delta).
│                               Built by docker compose.
├── Dockerfile.php              [v2.1-3] variant for PHP stack — FROM base + PHP 8.2
│                               + Composer 2 + 13 extensions. Adopted via docker-compose
│                               build.dockerfile: Dockerfile.php in consuming projects.
├── docker-compose.yml          services, volumes, sysctls, NET_ADMIN/NET_RAW
├── devcontainer.json           VS Code config + lifecycle hooks
├── initialize.sh               [host] menu + flag files + .env sync + build_base_if_missing
│                               (env -u HTTPS_PROXY strip, rolling progress, tee logging)
├── on-create.sh                [container] early firewall (before vscode-server downloads)
├── post-create.sh              [container] symlink CLAUDE.md by mode + smoke test
├── post-start.sh               [container] every restart: banner, creds sync, scan-deps reminder,
│                               Claude update check (registry npm), fallback warning banner
├── shell-init.sh               [container] sourced by every terminal: CA env, gh auto-auth, banner,
│                               Binary: indicator (extension/npm-fallback)
├── init-firewall.sh            [container, sudo] applies firewall mode at boot
├── firewall-mode.sh            [host] flip .configured-firewall-mode + sync .env
├── install-extensions.sh       [container] safety-net VS Code extension install (idempotent)
├── test-firewall.sh            [container] smoke test (called by post-create.sh)
│
├── README.md                   ← you are here
├── SECURITY.md                 threat model + accepted gaps
├── knowledge/                  AI-facing internals (entry: knowledge/INDEX.md)
│   ├── INDEX.md                topic index + small inline topics
│   ├── firewall.md             firewall internals + strict mode
│   ├── extension-points.md     how to add a skill / host-helper / variant
│   ├── docker-base-image.md    base-image scheme + layer ordering
│   ├── ollama-local.md         host-side Ollama backend + claude-switch
│   └── wtf.md                  .wtfcmd.yaml authoring (task runner)
├── RUNBOOK.md                  operational procedures (add domain, troubleshoot, reset)
├── RESEARCH.md                 research bundle workflow end-to-end
│
├── claude/
│   ├── CLAUDE-dev.md           symlinked to /workspace/CLAUDE.md in dev mode
│   ├── CLAUDE-reviewer.md      symlinked to /workspace/CLAUDE.md in reviewer mode
│   └── sync-creds.sh           bidirectional OAuth sync (claude-creds volume ↔ local)
│
├── firewall/
│   ├── domains.txt             baseline allowlist (17 hosts, Claude-only)
│   ├── domains.d/              per-ecosystem additive (committed): npm.txt, ecosystem-docs.txt
│   ├── domains.local.txt       per-dev overrides (gitignored OR committed by user choice)
│   ├── domains.local.txt.example   starter pack (npm public, MDN, telemetry-off, …)
│   ├── compile-policy.py       parser + compiler → policy.compiled.yaml + dnsmasq.conf
│   ├── policy.d/<host>.yaml    advanced rules per host (committed)
│   ├── policy.local.d/<host>.yaml  per-dev L7 overrides (gitignored)
│   ├── policy.local.d.example/     templates (committed)
│   ├── mitm-init.sh            launches mitmproxy with the 4 addons (strict only)
│   ├── addons/                 policy_enforce.py, format_detect.py, passive_log.py, stream_sse.py
│   └── tests/                  parse-domains.sh (44 cases), addons.sh (21), bypass.sh, probes.txt
│
├── skills/                     installed slash commands (synced by sync-skills.sh)
│   ├── prepare-pr/             /prepare-pr → pr-drafts/<id>.{md,yaml}
│   ├── watch-log/              /watch-log → pending/<id>.sh + Bash/Monitor wait
│   ├── prepare-research/       /prepare-research → research-bundles/<task>/ (5-file bundle)
│   ├── scan-deps/              /scan-deps → extract-auto-dependencies + AI review
│   ├── prepare-plan/           /prepare-plan → scaffold multi-session rollout dir
│   └── sync-skills.sh          merges skill commands + hooks into ~/.claude/settings.json
│
├── host-helpers/               host-side wrappers (invoked by user after Claude proposes)
│   ├── verify-slim-base        [v2.1] 9 PASS/FAIL gates sur la base image (size cap, /home/node, …)
│   ├── analyze-base-image      [v2.1] per-layer + per-dir + per-pkg breakdown (debug "où passent les GB")
│   ├── watch-log-cleanup       drop pending/* older than 60 min
│   ├── research-cleanup        list/delete sibling research projects (dry-run by default)
│   └── bring-back-result       archive research output back into research-bundles/
│
├── logs/                       [v2.1] build logs (build-base-<V>-<ts>.log, rebuild-context-<ts>.log) — gitignored
├── pr-drafts/                  output of /prepare-pr (gitignored except .keep)
├── pending/                    /watch-log scripts + logs (gitignored except .keep)
├── research-bundles/           /prepare-research output (gitignored except .keep)
├── scan-deps/                  /scan-deps audit trail + sentinel (gitignored except .keep)
│
├── tests/
│   ├── diagnose.sh             ~180 PASS/FAIL host-side (drives container via docker exec)
│   └── diag-a2.sh              per-mode snapshot capture
│
├── gh-secure/                  ARCHIVED in A3 — not COPY'd into the image
└── (phases/ removed in A5 cleanup)
```

## Firewall modes

| Mode | Stack | Outbound | Use case |
|---|---|---|---|
| **`strict`** (default) | DNS + iptables ipset + mitmproxy + 4 addons + IPv6 lockdown | Only through mitmproxy (`HTTPS_PROXY=http://127.0.0.1:8080`), iptables UID-matches `mitmproxy` | Production daily work |
| `basic` | DNS + iptables ipset only | Direct from any UID to allowlisted hosts | Debugging, when an app refuses HTTPS_PROXY |
| `off` | None (kill-switch) | Direct internet, no allowlist | Emergencies only (Claude reports the mode in its boot banner) |

Deprecated aliases (still accepted with a stderr warn): `okeish` → `basic`, `paranoid` → `strict`.

```
strict mode (the default):

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

Switch modes via `bash .devcontainer/firewall-mode.sh <off|basic|strict>` on the host, then VS Code → `Dev Containers: Rebuild Container`. See [RUNBOOK.md § Switch mode](RUNBOOK.md#4-switch-firewall-mode).

## Local hosts — DNS-driven aliases (no `extra_hosts`)

The firewall's allowlist is **DNS-driven** : every authorized host is declared in `firewall/domains.txt` (or `domains.local.txt`), dnsmasq resolves it, and `ipset=/host/allowed-domains` directives auto-populate the kernel ipset with the returned IPs. iptables then accepts traffic to any IP in that ipset.

This pipeline relies on the client *actually making a DNS query*. `extra_hosts` in `docker-compose.yml` would inject an entry into `/etc/hosts`, and `nsswitch.conf` (which has `files dns`) consults `/etc/hosts` **before** dnsmasq — short-circuiting the resolver and leaving the ipset unpopulated. The traffic might still work (depending on iptables defaults), but the firewall *doesn't know about it*, the audit log skips it, and `test-firewall.sh` reports a confusing `❌ DNS resolution failed`.

For internal aliases that need to point at the host gateway (`ollama.internal`, `ollama.local`, future MySQL/Redis on the host…), we therefore use a **CNAME chain inside dnsmasq** instead :

1. `init-firewall.sh` resolves `host.docker.internal` via Docker's internal resolver (`127.0.0.11`) at boot — the IP is runtime-assigned so we capture it dynamically.
2. It appends to the generated dnsmasq config :
   ```
   host-record=host.docker.internal,<IP>
   cname=ollama.internal,host.docker.internal
   cname=ollama.local,host.docker.internal
   ipset=/host.docker.internal/allowed-domains
   ```
3. Client query for `ollama.internal` → dnsmasq returns CNAME → resolves locally via the `host-record` → returns the IP **and** adds it to `allowed-domains` (matched by the auto-emitted `ipset=/ollama.internal/...` from `domains.txt`).
4. `curl http://ollama.internal:11434/...` works directly from the terminal post-rebuild ; `test-firewall.sh` reports `✔ ollama.internal reachable`.

To add a new host-gateway alias (e.g. `mysql.internal` for a Mac-side MySQL) : declare it in `firewall/domains.txt`, then append the CNAME line to `init-firewall.sh` next to the existing Ollama injection block, and add the corresponding port to `CLAUDE_CODE_FIREWALL_ALLOWED` in `.env` for the L4 TCP probe. See [knowledge/ollama-local.md](knowledge/ollama-local.md) for the full Claude-Code-local example.

## Local backends — switch Claude to Ollama

Claude Code in the container can talk to Anthropic's cloud (default) or to a host-side Ollama backend, via the host helper [`host-helpers/claude-switch`](host-helpers/claude-switch). Three modes :

| Mode | Endpoint | When to use |
|---|---|---|
| `cloud` | `api.anthropic.com` | Default. Anthropic SDK route. |
| `local` | `http://ollama.internal:11434` | Direct raw Ollama. Non-reasoning models, debug, or simplest setup. |
| `local-proxy` | `http://claude-bridge:9223` | Via the `claude-bridge` sidecar (UniClaudeProxy). Translates `<think>` blocks → Anthropic format. Default for reasoning models (qwen3, deepseek-r1). |

Quick setup (host-side) :

```bash
# Status — what mode is active right now
bash .devcontainer/host-helpers/claude-switch status

# Switch to local Ollama (direct)
bash .devcontainer/host-helpers/claude-switch local

# Switch to local-proxy (sidecar, for reasoning models)
bash .devcontainer/host-helpers/claude-switch local-proxy

# Back to Anthropic cloud
bash .devcontainer/host-helpers/claude-switch cloud
```

The helper runs on the host on purpose (toggling the in-container endpoint from outside keeps the threat boundary clean). It edits `.devcontainer/.env`, repoints `CLAUDE.md` to the right rules file, and for `local-proxy` auto-starts the sidecar. After the switch : **Rebuild Container** in VSCode (env_file is read at container creation only — Reload Window won't refresh PID 1's env).

Prereq for `local` / `local-proxy` : Ollama running on the host. Pick a hardware profile + model and start it with one of the tuned helpers ([`ollama-serve-16k`](host-helpers/ollama-serve-16k), [`ollama-serve-32k`](host-helpers/ollama-serve-32k)).

**Full guide** (install Ollama, hardware profiles, model choices, sidecar internals, tuning, audit, troubleshooting) : [knowledge/ollama-local.md](knowledge/ollama-local.md).

## Claude Code installation (v2.1)

The Claude Code binary is **baked into the base image** since v2.1 — no runtime VSIX download for fresh containers in the normal case. The whole install pipeline is governed by a **single source of truth** in `.devcontainer/.env`:

```bash
CLAUDE_CODE_VERSION=2.1.145
```

This one variable drives three install paths:

1. **VSIX URL** at build time → `marketplace.visualstudio.com/.../claude-code/${VERSION}/vspackage?targetPlatform=${PLATFORM}` (linux-x64 or linux-arm64)
2. **npm install fallback** at build time → `@anthropic-ai/claude-code@${VERSION}` (only invoked if the VSIX path fails)
3. **VS Code extension pin** at runtime → `customizations.vscode.extensions: ["anthropic.claude-code@${VERSION}"]` in `devcontainer.json` (manual sync — JSONC comment reminds; serves as a runtime safety net for Scenario 3)

### Failsafe chain — `docker build` never fails on a Claude problem

The build is hardened against Marketplace outages and Anthropic-side hiccups. Three scenarios are encoded by `/etc/claude-source` inside the running container:

| Scenario | Trigger | `/etc/claude-source` | `/usr/local/bin/claude` | Image size | Sentinel `/etc/claude-fallback-warn` |
|---|---|---|---|---|---|
| **1 — optimal** | VSIX DL OK + Phase B symlink OK | `extension:<path>` | symlink to ext binary | ~920 MB | absent |
| **2 — Phase B failed** | VSIX OK but binary path moved / version mismatch | `npm-fallback (VSIX baked, Phase B path issue)` | npm `cli.js` | ~1.16 GB | **present** (yellow banner) |
| **3 — Marketplace down** | VSIX DL KO at build | `npm-fallback (no VSIX, runtime ext install via Marketplace)` | npm `cli.js` | ~900 MB | **present** + VS Code re-DL at runtime |

When the sentinel is present, `post-start.sh` shows a yellow loud banner with diagnostic hints; `shell-init.sh` adds a 1-line `Binary:` indicator at every terminal open. See [RUNBOOK § Troubleshoot Claude failsafe](RUNBOOK.md#15-troubleshoot-claude-failsafe-scenarios).

### Bumping Claude Code

```bash
# 1. Edit .devcontainer/.env
CLAUDE_CODE_VERSION=2.1.146   # ← new pin

# 2. Also bump devcontainer.json extension pin (the JSONC comment reminds you)
#    customizations.vscode.extensions: ["anthropic.claude-code@2.1.146"]

# 3. VS Code → Dev Containers: Rebuild Container
#    initialize.sh's build_base_if_missing() rebuilds only the Claude layer
#    (~30s on arm64; layers 1-6 stay cached)

# 4. Verify
docker exec <ctr> claude --version       # → 2.1.146
docker exec <ctr> cat /etc/claude-source # → extension:<path> (Scenario 1)
```

`post-start.sh check_claude_update()` queries `registry.npmjs.org/@anthropic-ai/claude-code` at boot and prints a yellow banner if a newer version is published. Silent if the registry is unreachable or versions match.

Full step-by-step: [RUNBOOK § Bump Claude version](RUNBOOK.md#14-bump-claude-version).

## Host helpers (debug + sanity)

Host-side tools complementing the container scripts. All refuse to run inside the container (they need `docker` against the host daemon, or they read the workspace from outside the bind mount).

| Helper | Use case |
|---|---|
| [`host-helpers/verify-slim-base`](host-helpers/verify-slim-base) | Post-build sanity — 9 PASS/FAIL gates : size cap 1.2 GiB, `/home/node` ≤ 5 MiB, Claude pin match, opencode absent, mitmproxy bundled, slim package count consistent, layer dedup vs other images, docker system df snapshot. Portable awk fallback for macOS without `numfmt` |
| [`host-helpers/analyze-base-image`](host-helpers/analyze-base-image) | Debug "where do the GB go?" — per-layer (`docker history`) + per-directory (`du -sh` inside image) + per-package (`dpkg-query` top 30) breakdown of the built base image |
| [`host-helpers/research-cleanup`](host-helpers/research-cleanup) | List or delete sibling research projects older than N days (dry-run by default, `--apply` to actually delete) |
| [`host-helpers/bring-back-result`](host-helpers/bring-back-result) | Archive research output (`../<task>/result/`) back into `research-bundles/<task>/` + optional promote |
| [`host-helpers/watch-log-cleanup`](host-helpers/watch-log-cleanup) | Drop `pending/*.{sh,log,meta}` older than 60 min (auto-invoked from `post-start.sh`) |
| [`host-helpers/rebuild-base-image`](host-helpers/rebuild-base-image) | Rebuild `claude-devcontainer-base:<VERSION>` from `Dockerfile.base` with layer cache enabled (~10-30s on M-series). Needed when you edit a `COPY`'d script (`init-firewall.sh`, `test-firewall.sh`, `mitm-init.sh`, `compile-policy.py`, addons/, dnsmasq.conf, sudoers fragment) — VS Code's "Rebuild Container" alone only rebuilds the slim layer on top of the cached base, so script edits stay invisible. Pass `--no-cache` for a full rebuild |
| [`host-helpers/claude-switch`](host-helpers/claude-switch) | Toggle Claude Code between cloud / local Ollama mode — sed-edits `.env` (5 vars : `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CONFIG_DIR`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`) + repoints `CLAUDE.md` symlink. `{local\|cloud\|status}`. **Rebuild Container** after (Reload Window won't refresh PID 1's env). Full guide : [knowledge/ollama-local.md](knowledge/ollama-local.md) |
| [`host-helpers/mitm-capture`](host-helpers/mitm-capture) | Toggle the `capture_messages_debug.py` mitmproxy addon — dumps every POST `/v1/messages*` body + a replay-curl script into `/tmp/claude-capture/`. Live sentinel (no restart). `{on\|off\|status\|ls\|clear}`. Details : [knowledge/extension-points.md § Debug capture addon](knowledge/extension-points.md#debug-capture-addon-capture_messages_debugpy) |
| [`host-helpers/ollama-serve-16k`](host-helpers/ollama-serve-16k) / [`-32k`](host-helpers/ollama-serve-32k) | Launch `ollama serve` host-side with env vars tuned for the Compact (16 GB Mac) / Balanced (32 GB Mac) profile : `OLLAMA_FLASH_ATTENTION=1`, `OLLAMA_KV_CACHE_TYPE=q8_0`, `OLLAMA_CONTEXT_LENGTH=16384/32768`, `OLLAMA_KEEP_ALIVE=30m/1h`, `OLLAMA_MAX_LOADED_MODELS=1`, `OLLAMA_NUM_PARALLEL=1`. Quit the desktop app first (port conflict). Bigger profiles override env on the same `-32k` helper |
| [`host-helpers/audit-claude-code-proxies`](host-helpers/audit-claude-code-proxies) | Diff the embedded Claude Code proxy / network config against the expected pin — sanity for "did the npm install bring an unexpected upstream change?" |

Use cases :

- Edited a baked-in script and VS Code rebuild doesn't reflect the change → `bash .devcontainer/host-helpers/rebuild-base-image`, then VS Code → `Rebuild Container`.
- After a `Rebuild Container` that changed the base image → `bash .devcontainer/host-helpers/verify-slim-base` to confirm the trim still holds.
- Image grew unexpectedly between bumps → `bash .devcontainer/host-helpers/analyze-base-image` to find the offending layer.
- Step-by-step procedures: [RUNBOOK § Analyze image breakdown](RUNBOOK.md#16-analyze-base-image-breakdown).

## Variant `Dockerfile.php` (PHP stack)

For projects that need PHP 8.2 + Composer 2 in addition to the Node baseline, a slim variant lives at [`.devcontainer/Dockerfile.php`](Dockerfile.php) (v2.1-3). Pattern:

```dockerfile
ARG CLAUDE_CODE_VERSION=2.1.145
FROM claude-devcontainer-base:${CLAUDE_CODE_VERSION}
USER root
RUN apt-get install -y --no-install-recommends \
    php8.2-{cli,fpm,curl,gd,mbstring,xml,zip,soap,intl,mysql,readline,bcmath,sockets,phar} \
    && rm -rf /usr/share/doc/* /usr/share/man/*
COPY --from=composer:2 /usr/bin/composer /usr/bin/composer
USER node
```

**Option B chosen** : per-project Dockerfile, no intermediate `claude-devcontainer-base-php` tag, no `initialize.sh` modification. Docker layer cache deduplicates the PHP layer across PHP projects that share the same `FROM` + same `RUN apt install`.

Adoption procedure: [RUNBOOK § Adopt PHP variant](RUNBOOK.md#18-adopt-the-php-variant). To add a different variant (Python ML, Go, etc.), follow [knowledge/extension-points.md § Add a new Dockerfile variant](knowledge/extension-points.md#add-a-new-dockerfile-variant).

## Commands and skills

The devcontainer exposes a collection of slash commands (via `.devcontainer/skills/`), host-side wrappers (via `.devcontainer/host-helpers/`), and container scripts. Listing them all so a newcomer can ground their mental model.

### Skills committed (project-wide)

| Slash command | Purpose | Skill body |
|---|---|---|
| `/prepare-pr` | Generate a PR draft pair (`.md` body + `.yaml` metadata) under `pr-drafts/` — the host runs the actual `gh pr create` | [skills/prepare-pr/](skills/prepare-pr/) |
| `/watch-log` | Generate a script in `pending/<id>.sh` that ends with `__END__` sentinel + drive Bash background or Monitor stream | [skills/watch-log/](skills/watch-log/) |
| `/prepare-research` | Generate a self-contained research bundle (`.devcontainer/` verbatim + workspace files + secrets template) for scope enlargement | [skills/prepare-research/](skills/prepare-research/) |
| `/scan-deps` | Two-layer dependency audit : `extract-auto-dependencies` (deterministic bash) + AI review (suspicious deps, POST candidates) | [skills/scan-deps/](skills/scan-deps/) |
| `/prepare-plan` | Scaffold a multi-session rollout directory (ROLLOUT + STATUS + LOG + EXISTING + sessions/) for features that need ≥3 sessions | [skills/prepare-plan/](skills/prepare-plan/) |

### Skills locaux opt-in (gitignored)

Skills suffixed `.local/` are personal — not shared across the team. The repo ships starter examples:

| Slash command | Purpose | Status |
|---|---|---|
| `/hours` | Time estimation for tasks (heures-valeur) | local opt-in |
| `/claude-limits` | Usage stats and quota tracking | local opt-in |

### Skills globaux Anthropic

Provided by Claude Code itself, available everywhere. Selected entries relevant in this devcontainer:

| Slash command | Purpose |
|---|---|
| `/init` | Generate or refresh `CLAUDE.md` from a codebase analysis |
| `/review` | Review a PR or diff |
| `/security-review` | Security-focused review of pending changes |
| `/simplify` | Review code for reuse, quality, efficiency |
| `/verify` | Run the project + confirm a fix or change works in the real app |
| `/run` | Launch the project's app to observe a change live |
| `/loop` | Run a prompt or skill on a recurring interval |
| `/schedule` | Create cron-based remote agents (routines) |
| `/claude-api` | Build / debug / migrate code that uses the Anthropic SDK |
| `/update-config` | Configure `settings.json` (permissions, hooks, env vars) |
| `/keybindings-help` | Customize Claude Code keyboard shortcuts |
| `/fewer-permission-prompts` | Reduce permission prompts by adding common allowlist entries |

### Host helpers

See the [Host helpers](#host-helpers-debug--sanity) section above for the full list.

### Container scripts (sudo-friendly)

| Script | Purpose | Invoke |
|---|---|---|
| `init-firewall.sh` | Apply firewall mode at boot — dnsmasq + iptables + mitmproxy if strict | `sudo /usr/local/bin/init-firewall.sh` (sudoers entry allows NOPASSWD) |
| `test-firewall.sh` | Connectivity smoke test post-firewall init | `sudo /usr/local/bin/test-firewall.sh` (same sudoers entry) |
| `install-extensions.sh` | Safety net for VS Code extension install | `bash .devcontainer/install-extensions.sh` |
| `firewall/mitm-init.sh` | mitmproxy sanity check + daemon launch (called by init-firewall in strict) | (internal — invoked by init-firewall.sh) |
| `firewall/compile-policy.py` | Parse `domains.txt` + `domains.d/` + `policy.d/` + locals → `policy.compiled.yaml` + dnsmasq config | (internal — invoked by init-firewall.sh) |

### Host scripts

| Script | Purpose |
|---|---|
| `initialize.sh` | First-prompt menu + flag files + `.env` sync + `build_base_if_missing` (builds `claude-devcontainer-base:${VERSION}` once per pin). Run via VS Code `initializeCommand` |
| `firewall-mode.sh` | Flip `.configured-firewall-mode` + sync `.env` proxy vars. Run via `bash .devcontainer/firewall-mode.sh <off|basic|strict>` then Rebuild Container |
| `claude/sync-creds.sh` | Bidirectional `claude-creds` OAuth sync — called by post-start, shell-init, and Claude Code Stop/SessionEnd hooks |

## Configuration — files you edit by hand

### `firewall/domains.txt` — baseline (rarely touched)

17 Claude-only hosts. Don't add to this file casually — adding a host here means every dev gets it. If you need `docs.example.com` only for yourself, use `domains.local.txt` instead. If you need a project dep, let `/scan-deps` write it to `domains.d/<eco>.txt`. Syntax (5 formats):

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

Full reference (precedence, `!disable`, `policy.d/<host>.yaml`): see the policy compiler at `firewall/compile-policy.py`.

### `firewall/domains.d/<eco>.txt` — per-ecosystem deps (auto-generated, committed)

Written by `/scan-deps` extractors (currently: `npm.txt`, `ecosystem-docs.txt`). Don't edit by hand — re-run the skill and let it rewrite. The file is committed so PR reviewers see the additive surface change.

### `firewall/domains.local.txt` — your personal overrides

Same syntax as `domains.txt`. Use this for:
- A doc site you read often but the team doesn't need (`docs.example.com`)
- Disabling baseline telemetry: `!disable *.statsig.com`
- Redefining a host with broader methods (host-level `redefine` wipes baseline paths and replaces methods)

Gitignored by default. Some teams commit it to share team-wide overrides — your call.

A starter pack lives in `domains.local.txt.example` (npm public, MDN, telemetry-off, …) — copy sections you want.

### `firewall/policy.d/<host>.yaml` — advanced L7 rules

One file per host. Specifies `max_body_kb`, `query_params` schemas, `blocked_paths`, `defaults_override`. Committed. Example:

```yaml
endpoints:
  - path: /v1/messages
    max_body_kb: 32768
defaults_override:
  max_query_string_length: 4096
blocked_paths:
  - "^/v1/admin"
```

### `firewall/policy.local.d/<host>.yaml` — your personal L7 overrides

Same shape, gitignored. Deep-merges into the committed `policy.d/<host>.yaml`.

### `.configured-*` flag files

Single source of truth for each setup choice. Delete the file + rebuild to reset:

| Flag | Reset effect |
|---|---|
| `.configured-auth` | re-prompt at next `initialize.sh` for standard vs advanced gh auth |
| `.configured-claude-mode` | re-prompt for dev vs reviewer mode |
| `.configured-firewall-mode` | `initialize.sh` rewrites `strict` silently (default) |
| `.configured-claude-rules` | Claude re-runs the first-prompt project analysis |

### `devcontainer.json`

- `customizations.vscode.extensions`: pinned versions (`anthropic.claude-code@<X>`)
- `containerEnv`: `CLAUDE_CONFIG_DIR`, `GH_CONFIG_DIR`, `NODE_OPTIONS`
- Lifecycle: `initializeCommand`, `onCreateCommand`, `postCreateCommand`, `postStartCommand`

When you change extensions or env, `Rebuild Container` is required (not `Reload Window`).

## Workflows

### Daily dev → PR

```
Claude (in container)            User (on host)              GitHub
─────────────────────            ──────────────              ──────
git checkout -b feat/xyz
edit code
git commit (local OK)
/prepare-pr ─────────► pr-drafts/<id>.md + .yaml
                       │
                       ▼ user reviews draft
                                  pr-from-draft <draft>
                                  (host has gh + PAT)
                                  surconfirm + push + gh pr create
                                  ─────────────────────────────► PR created
```

The container never pushes. The host helper `pr-from-draft` parses the YAML metadata, shows a preview, asks for `yes` confirmation, then runs `git push` + `gh pr create --draft`.

### Scope enlargement → research project

```
Claude (in container)               User (on host)              Research container
─────────────────────               ──────────────              ──────────────────
/prepare-research stripe-api ─► research-bundles/stripe-api/
                                (5 files: domains.local.txt,
                                 policy.local.d.example/,
                                 instructions.md,
                                 files-to-copy.txt,
                                 secrets.env.template)
                                    │
                                    ▼ user reviews + edits
                                  cp -r .devcontainer/research-bundles/stripe-api/ \
                                       ../stripe-api/
                                  cd ../stripe-api/
                                  edit .devcontainer/.env.local with real STRIPE_KEY
                                  code . → Reopen in Container
                                                                  Claude reads INSTRUCTIONS.md
                                                                  works in /workspace
                                                                  writes /output/<result>
                                    │
                                    ▼
                                  bring-back-result stripe-api
                                  (archive output → main bundle)
                                  research-cleanup (after >7d sibling delete)
```

Full procedure: [RESEARCH.md](RESEARCH.md). Templates (`new-api-integration`, `doc-research`, `package-evaluation`): `skills/prepare-research/templates/`.

### Dependency audit → `domains.d/<eco>.txt`

```
post-start.sh boots
  └─► checks scan-deps/.last-scan.json sentinel
       └─► if manifest mtime > sentinel.ts → cyan banner:
           "⚠  Run /scan-deps : package.json modified since last scan"

User: /scan-deps
  └─► step 1: extract-auto-dependencies (bash, deterministic)
       │       parses package.json + lock + node_modules
       │       writes firewall/domains.d/npm.txt + ecosystem-docs.txt (committed)
       └─► step 2: AI review
               flags suspicious deps, postinstall scrutiny, POST candidates
               proposes /prepare-research if scope outside baseline
```

The committed `domains.d/<eco>.txt` shows up in PR review — every allowlist change is auditable.

### Long-running script → `/watch-log`

```
Claude needs to run a build / test / install that takes minutes.
  └─► /watch-log
        ├─► generates .devcontainer/pending/<id>.sh with trap "echo __END__" EXIT
        └─► proposes to user:
              bash .devcontainer/pending/<id>.sh > .devcontainer/pending/<id>.log 2>&1

User runs the command in another terminal.
Claude meanwhile:
  Pattern A (single notification):  Monitor tailing the log with grep `__END__|FATAL`
                                    (fallback if Monitor unavailable: Bash run_in_background
                                     with `until grep __END__`)
  Pattern B (live stream):          Monitor tailing the log with grep filter
```

Stale pending files (>60 min) are cleaned by `host-helpers/watch-log-cleanup` invoked from `post-start.sh`.

## GitHub authentication

Two modes, chosen once at first build (`.configured-auth` flag):

| Mode | What runs | Access |
|---|---|---|
| **Standard** (default) | `gh auth login` on first terminal (OAuth device flow) | Whatever the token scopes give you |
| **Advanced** | gh-secure host-side wrapper (read-only PAT + GitHub App write tokens) | Read PAT for routine ops; write only via wrapper for `gh pr create/edit/comment` |

**Important**: gh-secure was **archived in A3 cleanup** for the Phase 3 main container — the directory still exists at `gh-secure/` for reference but the Dockerfile no longer COPIes it into the image. Phase 3 routes PR creation through the host helper instead (`/prepare-pr` → host `pr-from-draft`).

To reconfigure: `rm .devcontainer/.configured-auth && Rebuild Container`.

## Shared Claude credentials across projects

Set `CLAUDE_CREDS_VOLUME=claude-creds-shared` in `.env` (or `.env.local`) for each project. `post-start.sh` syncs `.credentials.json` between the shared volume and the local container — most-recently-refreshed `expiresAt` wins. See [knowledge/INDEX.md § Claude OAuth sync flow](knowledge/INDEX.md#claude-oauth-sync-flow) for the bidirectional logic.

## Troubleshooting (quick pointers)

| Symptom | First check | Procedure |
|---|---|---|
| `curl X.example.com` blocked | `cat .configured-firewall-mode` | [RUNBOOK § Troubleshoot curl](RUNBOOK.md#3-troubleshoot-a-blocked-curl) |
| Yellow Claude fallback banner at boot | `cat /etc/claude-source` | [RUNBOOK § Troubleshoot Claude failsafe](RUNBOOK.md#15-troubleshoot-claude-failsafe-scenarios) |
| Newer Claude version available banner | `.env` `CLAUDE_CODE_VERSION` | [RUNBOOK § Bump Claude version](RUNBOOK.md#14-bump-claude-version) |
| Base image bigger than expected | `host-helpers/analyze-base-image` | [RUNBOOK § Analyze image breakdown](RUNBOOK.md#16-analyze-base-image-breakdown) |
| VS Code re-downloads Claude extension at boot | `/home/node/.vscode-server/extensions/extensions.json` | [RUNBOOK § Regen extensions.json](RUNBOOK.md#19-regen-extensionsjson-if-vs-code-redownloads) |
| VS Code extension missing | `bash install-extensions.sh` | [RUNBOOK § Reinstall extensions](RUNBOOK.md#10-reinstall-vs-code-extensions) |
| Claude OAuth conflict prompt | `/tmp/.claude-creds-conflict` | [knowledge/INDEX § Debug recipes](knowledge/INDEX.md#debug-recipes) |
| `mitmproxy CA invalid` | volume `mitmproxy-${DC_PROJECT}` | [RUNBOOK § Regenerate CA](RUNBOOK.md#6-regenerate-mitmproxy-ca) |
| Scan-deps banner keeps showing | sentinel `scan-deps/.last-scan.json` | [RUNBOOK § Force rescan](RUNBOOK.md#9-force-a-fresh-scan-deps) |
| `git push` fails | (by design — push from host) | [RUNBOOK § Add a domain](RUNBOOK.md#1-add-a-read-only-domain) for read access |
| Base build fails / want a clean rebuild | `BUILD_BASE_NO_CACHE=1` | [RUNBOOK § Force rebuild base no-cache](RUNBOOK.md#17-force-rebuild-base-no-cache) |
| Want to add PHP / Python / other runtime | `Dockerfile.php` is the reference variant | [RUNBOOK § Adopt PHP variant](RUNBOOK.md#18-adopt-the-php-variant) + [knowledge/extension-points.md § Add a Dockerfile variant](knowledge/extension-points.md#add-a-new-dockerfile-variant) |

Diagnose-all: `bash .devcontainer/tests/diagnose.sh` (from the host).

## FAQ

**Can I just `curl docs.example.com`?**
Only if `docs.example.com` is in `domains.txt`, `domains.d/<eco>.txt`, or your `domains.local.txt`. Otherwise the DNS query returns NXDOMAIN. Add to `domains.local.txt` for read-only docs.

**Can I `npm install <new-package>`?**
First check `/scan-deps` — it will tell you whether the registry + transitive + postinstall hosts are covered. If they aren't, either add to `domains.local.txt` (for read-only deps) or spawn `/prepare-research` (for postinstall that needs network).

**Can I `git push`?**
No — the main container has no PAT, no SSH key, and `git-receive-pack` POST is not allowlisted. Use `/prepare-pr` → host `pr-from-draft`.

**Can I `gh pr create`?**
No, same reason. Use `/prepare-pr`. The draft includes title, body, base, head, labels in a YAML frontmatter the host helper parses.

**Where do I add a new lifecycle hook?**
It depends on the idempotency you need. See [knowledge/INDEX.md § Lifecycle ordering + extension points](knowledge/INDEX.md#lifecycle-ordering--extension-points).

**Why is `policy.compiled.yaml` not in the repo?**
It's a build artifact regenerated at every boot by `init-firewall.sh`. The committed sources are `domains.txt` + `domains.d/` + `policy.d/`. Editing `policy.compiled.yaml` directly is futile (overwritten at next boot) and dangerous (no source-of-truth).

## See also

- [SECURITY.md](SECURITY.md) — threat model + accepted gaps
- [knowledge/INDEX.md](knowledge/INDEX.md) — internals (volumes, OAuth flow, hooks, idempotency contracts, extension points)
- [RUNBOOK.md](RUNBOOK.md) — operational procedures (add domain, reset CA, troubleshoot)
- [RESEARCH.md](RESEARCH.md) — research bundle workflow

# Scope — Part 1 install.sh v2

> Single source of truth for the files included in the **Part 1**
> deliverable (new-project install via `install.sh` v2.0).
> Part 2 (Claude session prompt for migrating existing 1.3 → 2.0
> projects) is out of scope here — deferred per user.

## Templating model — important finding

The v2 baseline in `/workspace/.devcontainer/` **abandons the
`{{PLACEHOLDER}}` + sed substitution pattern** that v1.3 used.
Instead it leans on **shell variable expansion** :

- `docker-compose.yml` : `${DC_PROJECT:-ragnarok-online-general}`,
  `${CLAUDE_CREDS_VOLUME:-claude-creds-${DC_PROJECT}}`, etc.
- `initialize.sh` : sources `.env`, defaults via `${VAR:-default}`
- `.env.example` : pure key=value documentation, no placeholders

**Consequence for install.sh v2** : the wizard collapses from 13
prompts to **4** :

1. `PROJECT_ID` (slug, used as `DC_PROJECT`) — default :
   slugified target dir basename
2. `PROJECT_DISPLAY_NAME` — default : title-cased PROJECT_ID
3. `PROJECT_TYPE` (node / php / custom) — default : node
4. **Shared Claude creds volume** — wizard lists existing
   `claude-creds-*` docker volumes ; if none found, proposes the
   generic default name `claude-credentials-shared` ; user can
   accept, enter another name, or pick `n` for per-project
   isolation

Timezone is **not prompted** — defaults to `Europe/Paris` baked
into the v2 baseline (`.env.example` + `docker-compose.yml` default
expansion). Users edit `.env` post-install if they need otherwise.

The script then :

1. Copies files **verbatim** (no sed) for the vast majority
2. **Copies `.env.example` → `.env`** (the full documented template,
   all vars commented), then sets `DC_PROJECT=<PROJECT_ID>` at the
   top + uncomments `CLAUDE_CREDS_VOLUME=<name>` if the user picked
   a shared creds volume. The rest stays commented for the user to
   tweak post-install
3. Generates `.gitignore` entries
4. Sets exec perms on `.sh` files

Only **3 files** need any text substitution :

| File | Substitution needed |
|---|---|
| `devcontainer.json` | hard-coded "Ragnarok Online Dev" → `{{PROJECT_DISPLAY_NAME}}` (line 2 + line 40 `window.title`) |
| `.env.example` | `DC_PROJECT=ragnarok-online-general-feat` default comment → `DC_PROJECT={{PROJECT_ID}}` |
| `claude/CLAUDE-project.md` | shipped as a **stub with placeholder content** instructing the user (or Claude on first boot) to generate the real project-specific rules. Stub points to CLAUDE-dev.md for the canonical pattern |

Everything else copies verbatim.

## IN-SCOPE — Part 1

### Build (5 files)

- `Dockerfile.base` — ~440 lines, baked image (claude-code,
  mitmproxy, wtf, git-delta, gh)
- `Dockerfile` — ~20 lines, project layer (FROM base + project deps)
- `Dockerfile.php` — PHP variant, copied if user picks "php" in
  wizard project-type prompt (node / php / custom)
- `docker-compose.yml` — uses `${DC_PROJECT:-...}` everywhere
- `devcontainer.json` — **needs templating** for project name

### Lifecycle (6 files)

- `initialize.sh` — host pre-build hook
- `on-create.sh` — early container init (firewall)
- `post-create.sh` — symlink CLAUDE.md, smoke test
- `post-start.sh` — restart banner, creds sync, skills sync,
  firewall re-init
- `shell-init.sh` — sourced per terminal, CA env + auth
- `install-extensions.sh` — VS Code safety-net

### Env / Config (2 files)

- `.env.example` — **needs templating** for default `DC_PROJECT`
- `vscode-settings.json` — currently empty `{}` but required (mount
  target referenced by `devcontainer.json`)

### Firewall (29 entries)

Core scripts (9) :
- `init-firewall.sh` — sudo, dnsmasq + iptables + mitmproxy
- `firewall-mode.sh` — host, flip mode flag
- `test-firewall.sh` — sudo smoke test
- `firewall/dnsmasq.conf`
- `firewall/domains.txt` — baseline 17 hosts (Claude only)
- `firewall/domains.local.txt.example` — starter pack
- `firewall/compile-policy.py` — parser
- `firewall/mitm-init.sh` — mitmproxy daemon
- `firewall/firewall-blocks` — iptables UDP/53 helper

mitmproxy addons (4) — _excludes the 5th `capture_messages_debug.py`
added to `.devcontainer/` post-SCOPE-freeze, intentionally not shipped
in templates/ (debug-only)_ :
- `firewall/addons/policy_enforce.py`
- `firewall/addons/format_detect.py`
- `firewall/addons/passive_log.py`
- `firewall/addons/stream_sse.py`

L7 policies baseline (`firewall/policy.d/`, 13 files) — **NOT
auto-compiled** ; these ship default policies for the core
allowlisted hosts (api.anthropic.com, github, mcp-proxy,
registry.npmjs, sentry, statsig, vscode marketplace + cdn,
mitmproxy targets, ollama, claude-bridge) :
- `api.anthropic.com.yaml`
- `api.github.com.yaml`
- `claude-bridge.yaml`
- `gallerycdn.vsassets.io.yaml`
- `github.com.yaml`
- `marketplace.visualstudio.com.yaml`
- `mcp-proxy.anthropic.com.yaml`
- `ollama.internal.yaml`
- `platform.claude.com.yaml`
- `registry.npmjs.org.yaml`
- `sentry.io.yaml`
- `statsig.com.yaml`
- `vsassets.io.yaml`

L7 user-override starter pack (`firewall/policy.local.d.example/`,
3 files) :
- `README.md`
- `api.anthropic.com.warn.yaml`
- `api.anthropic.com.yaml`

### Skills (6 entries : sync + 5 generic)

- `skills/sync-skills.sh` — auto-merges into `~/.claude/` at
  post-start
- `skills/prepare-pr/` — entire dir
- `skills/watch-log/` — entire dir
- `skills/prepare-research/` — entire dir
- `skills/scan-deps/` — entire dir
- `skills/prepare-plan/` — entire dir

**Excluded** : `skills/hours.local/` (freelance hours estimation
+ TJM calibration — personal workflow) and
`skills/claude-limits.local/` (Anthropic quota display). Per-user
preference, not generic — install.sh v2 does NOT ship them.
Users add them manually post-install.

### Claude (5 files)

- `claude/sync-creds.sh` — bidirectional OAuth sync, auto at
  post-start + shell-init + Claude `Stop`/`SessionEnd` hooks
- `claude/CLAUDE-dev.md` — copied verbatim, user customises after
- `claude/CLAUDE-reviewer.md` — generic reviewer-mode rules
- `claude/CLAUDE-local-dev.md` — rules used when Claude runs against
  a local Ollama backend (paired with `claude-bridge/`)
- `claude/CLAUDE-project.md` — **stub with placeholder content** ;
  user (or Claude on first boot) generates the real project-specific
  rules from the stub instructions

### Knowledge (6 files — full directory)

Confirmed full inclusion : Claude needs these to operate the
devcontainer (debug firewall, understand wtf, navigate extension
points, debug Ollama, etc.) without having to discover the code
each session.

- `knowledge/INDEX.md` — topic index + idempotency contracts
- `knowledge/firewall.md`
- `knowledge/wtf.md`
- `knowledge/extension-points.md`
- `knowledge/docker-base-image.md`
- `knowledge/ollama-local.md`

### Docs (4 files)

- `README.md` (~46 K) — primary entry point, operational reference
  (firewall modes, local backends, host helpers, workflows). Users
  need this in their project, not linked externally
- `RUNBOOK.md` (~26 K) — step-by-step operational procedures
- `SECURITY.md` (~13 K) — threat model + accepted gaps
- `RESEARCH.md` (~12 K) — research bundle workflow, paired with
  the `/prepare-research` skill which ships in install.sh v2

### Ollama / local-backend sidecar (21 entries)

`claude-bridge/` — UniClaudeProxy sidecar (8 files) :
- `claude-bridge/Dockerfile`
- `claude-bridge/config.example.json`
- `claude-bridge/healthcheck.sh`
- `claude-bridge/ucp-overlay/app/config.py`
- `claude-bridge/ucp-overlay/app/main.py`
- `claude-bridge/ucp-overlay/app/converters/anthropic_to_openai.py`
- `claude-bridge/ucp-overlay/app/converters/openai_to_anthropic.py`
- `claude-bridge/ucp-overlay/app/providers/openai_provider.py`

`host-helpers/` — host-side helpers (12 files) :
- `host-helpers/analyze-base-image`
- `host-helpers/audit-claude-code-proxies`
- `host-helpers/bring-back-result`
- `host-helpers/claude-bridge`
- `host-helpers/claude-switch`
- `host-helpers/mitm-capture`
- `host-helpers/ollama-serve-16k`
- `host-helpers/ollama-serve-32k`
- `host-helpers/rebuild-base-image`
- `host-helpers/research-cleanup`
- `host-helpers/verify-slim-base`
- `host-helpers/watch-log-cleanup`

`diag-ollama-local.sh` (1 file) — paired with `claude-bridge/` and
`claude-switch`, debugs the local-Ollama bridge.

**Subtotal IN : ~84 entries**

## OUT-OF-SCOPE — Part 1

### Out — project-specific or per-user (must NOT propagate)

- `IMPLEMENTATION-PLAN.md` — local plans/ redirect, project-specific
- `firewall/domains.d/` — auto-generated by `/scan-deps` per project
- `firewall/domains.local.txt` — per-dev overrides (gitignored)
- `firewall/policy.local.d/` — per-dev L7 overrides (gitignored)
- `.env`, `.configured-*` — runtime/generated
- `logs/`, `pending/`, `pr-drafts/`, `research-bundles/`,
  `scan-deps/` (runtime dirs)
- `skills/hours.local/` — freelance hours estimation, per-user
- `skills/claude-limits.local/` — Anthropic quota display, per-user

### Out — debugging only

- `tests/diagnose.sh`, `tests/diag-a2.sh` — debugging gates,
  invoked manually when troubleshooting

### Out — dropped from v1.3 (breaking change, documented in
CHANGELOG for v2.0.0)

- `gh-secure/` (6 scripts) — archived in Phase 3, `/prepare-pr`
  skill replaced it
- `Dockerfile.node` — user explicit drop
- `templates/Dockerfile.custom` → renamed `templates/Dockerfile`
- `KNOWLEDGE.md` (single-file) — superseded by `knowledge/`
  directory
- `skills/master-review/` — superseded by `/prepare-pr` skill
- `templates/test-db.php` — unused
- `templates/gitignore-entries.txt` — install.sh embeds the
  list inline now

## Placeholders to introduce in install.sh v2

**2 placeholders** total (down from 11 in v1.3) :

| Placeholder | Used in | Source |
|---|---|---|
| `{{PROJECT_ID}}` | `.env.example` (default `DC_PROJECT=...`) | wizard prompt |
| `{{PROJECT_DISPLAY_NAME}}` | `devcontainer.json` (`name` + `window.title`) | wizard prompt (derived from PROJECT_ID by default) |

Plus the `CLAUDE-project.md` stub which has no formal placeholder
syntax but ships with skeleton content instructing the user to
generate the real rules on first container boot.

`CLAUDE_CREDS_VOLUME` is **not** a sed placeholder — it's written
directly into the generated `.env` by the install.sh wizard when
the user picks (or defaults to) a shared creds volume name.

Timezone is **baked** to `Europe/Paris` in the v2 baseline files —
no placeholder, no prompt.

All other v2 files use `${VAR:-default}` shell expansion which
needs no substitution — the new `.env` generated by install.sh
provides the values at runtime.

## Sync mechanism inherited from v2 (no work needed)

Both syncs are **already automatic** in the v2 files we're
copying — install.sh v2 inherits them for free :

- **Creds sync** : `claude/sync-creds.sh` triggered from
  `post-start.sh` (silent), `shell-init.sh` (verbose per terminal),
  and Claude Code hooks `Stop`/`SessionEnd` installed by
  `skills/sync-skills.sh`
- **Skills sync** : `skills/sync-skills.sh` runs at every container
  start (called by `post-start.sh`), merges all
  `skills/*/hooks.json` into `~/.claude/settings.json` and copies
  `*.skill.md` to `~/.claude/commands/`

No new sync logic to write in install.sh v2 itself.

## Templates/ sync mechanism (delivered P1-S2)

The sync from `.devcontainer/ → templates/` runs **inline during
session 2**, no `sync-templates.sh` helper. The canonical refresh
procedure (drop list + copy block + scrub edits) is documented in
[LOG.md](LOG.md) § P1-S2. Future refreshes (when `.devcontainer/`
evolves) re-apply that block manually.

**Scrub rule** : `templates/` must contain zero references to any
historical project name (`ragnarok`, `cyro`, `portal42`, `boa`).
Templated files (e.g. `devcontainer.json` with `{{PROJECT_DISPLAY_NAME}}`)
or files rewritten during sync (e.g. `CLAUDE-reviewer.md`,
`CLAUDE-project.md` stub) carry no project-specific content.

For non-templated files where defaults exist (e.g. shell expansion
defaults `${DC_PROJECT:-...}` in `docker-compose.yml` and
`initialize.sh`), the neutral default is `dc-project`. Propagated
both to `templates/` and `.devcontainer/` for sustainability of the
v2 baseline.

## Reference paths

- [`.devcontainer/`](../../.devcontainer/) — v2 source of truth
- [`devcontainer-tools/templates/`](../../devcontainer-tools/templates/) — v1.3 templates (subject to scope filter)
- [`devcontainer-tools/install.sh`](../../devcontainer-tools/install.sh) — v1.3 install (rewritten in Part 1 session 2)

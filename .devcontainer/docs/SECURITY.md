# Security — DevContainer firewall, as actually configured

> Threat model + security decisions for this devcontainer (the dogfood instance of the
> devcontainer-sandbox base image).
>
> ⚠ **This tree runs in `basic` mode.** [`../firewall/default-mode`](../firewall/default-mode)
> contains `basic`, committed deliberately. `basic` is **DNS + IP allowlisting only** — the L7
> layers (path filtering, method allowlist, body caps, content and URL inspection, audit log)
> described below **do not run here**. `strict` is the stronger posture and is an opt-in that
> costs a rebuild.
>
> Read [`../firewall/CLAUDE.md`](../firewall/CLAUDE.md) before inferring any access rule from
> the files in `../firewall/` — it is the short version of this document's central point.
>
> See also: [`../RUNBOOK.md`](../RUNBOOK.md) for procedures,
> [`../knowledge/firewall.md`](../knowledge/firewall.md) for internals.
>
> ⚠ **Path caveat.** The scripts and addons cited below (`../init-firewall.sh`,
> `../firewall/mitm-init.sh`, `../firewall/addons/`, `../firewall/compile-policy.py`,
> `../knowledge/firewall.md`, `../RUNBOOK.md`) are this tree's **pre-migration v2 copies**, and
> they are what boots today. The v3 migration moves all of them into the base image — the
> project then contributes only the *data* under `../firewall/`. The mode semantics described
> here are identical in both (verified against both scripts) ; only the paths move.

## The one thing to know

**In `basic`, an allowlisted HOST accepts ALL PATHS and ALL METHODS.**

Reading `[GET] github.com` + `/anthropics/*` in `../firewall/domains.txt` and concluding that
`github.com/torvalds/…` is blocked is a **false inference in this tree**. The only matching
primitive in `basic` is an iptables destination-IP match — it cannot see a path, a method, a
header or a body.

The path scopes are not decoration: they are **documentation of intent**, and they become
enforcement the moment someone switches to `strict`. But today they describe a narrower surface
than the one in force.

## What runs, by mode

Measured in [`../init-firewall.sh`](../init-firewall.sh), the script this tree actually boots
(`md5sum /usr/local/bin/init-firewall.sh` matches it).

| | `strict` | `basic` ← **here** | `off` |
|---|---|---|---|
| dnsmasq host allowlist (L1) | ✅ | ✅ | ❌ |
| ipset + iptables default-DROP | ✅ | ✅ | ❌ |
| UDP/53 pinned to the dnsmasq UID | ✅ | ✅ | ❌ |
| mitmproxy running | ✅ | ❌ | ❌ |
| Path filtering (L2) | ✅ | ❌ | ❌ |
| Method allowlist (L3) | ✅ | ❌ | ❌ |
| Content / archive inspection (L4) | ✅ | ❌ | ❌ |
| URL inspection — base64 / hex / internal path (L5) | ✅ | ❌ | ❌ |
| Body-size caps (L6) | ✅ | ❌ | ❌ |
| Audit log of every non-GET | ✅ | ❌ | ❌ |

- `init-firewall.sh:133-139` reads the mode from the **baked** `/etc/devcontainer-firewall/default-mode`.
  Legacy aliases `paranoid`→`strict`, `okeish`→`basic`. An unknown or missing value falls back
  to `strict`, which is the safe direction.
- `init-firewall.sh:599` is the **only** call site of `mitm-init.sh`, and it is gated on
  `strict`. [`../firewall/mitm-init.sh:28-33`](../firewall/mitm-init.sh) refuses a second time
  if invoked by hand outside `strict`.
- `init-firewall.sh:615-622` vs `:623-627` is the egress difference: in `strict` the ipset
  ACCEPT is restricted to `--uid-owner mitmproxy`, so every other UID must go through
  `127.0.0.1:8080`. In `basic`, the comment in the script says it plainly — *"any UID can reach
  allowlisted hosts directly"*.
- `off` bails out early, flushes every rule, kills dnsmasq and mitmdump, and restores the Docker
  resolver.

**The compiled policy is still generated in `basic`, and nothing reads it.**
`policy.compiled.yaml` is produced at every boot, but its only two readers are
`../firewall/addons/policy_enforce.py` and `../firewall/addons/format_detect.py`, neither of
which is loaded. `../reload-local.sh` says so in its own comment.

## The allowlist, measured

| | Committed | Actually in force |
|---|---|---|
| Non-comment entries | **127** (66 host declarations + 61 indented path lines) | same, plus the local layer |
| Distinct hosts | **55** | **~162** |
| Hosts carrying a path restriction | **22** of 55 | inert in `basic` |
| `policy.d/*.yaml` rule files | **13** | inert in `basic` |

⚠ The committed count is **not** the surface. `../firewall/domains.local.txt` is gitignored
(`../.gitignore:23`) yet **baked into the image** by `../Dockerfile:60`
(`COPY firewall/ /etc/devcontainer-firewall/`) and merged by `compile-policy.py`. It currently
adds ~107 host declarations — reddit, Hacker News, Medium, VPS comparators, model registries.
A reader who counts only what is committed underestimates the reachable set by roughly 3×.
`../firewall/domains.local.local-llm.txt` is a staged profile and is **not** loaded
(`compile-policy.py` reads only `domains.txt`, `domains.local.txt` and `domains.d/*.txt`).

Allowlist sources, in merge order:
- `../firewall/domains.txt` — committed baseline
- `../firewall/domains.d/<eco>.txt` — per-ecosystem, **additive** (methods UNION, paths CONCAT),
  generated by `/scan-deps`, committed so every change shows up in `git log`
- `../firewall/domains.local.txt` — per-developer, gitignored, **redefines** entry by entry
- `../firewall/policy.d/<host>.yaml` — per-host L7 rules (`strict` only)

## Trust boundary

| Component | Trust | Why |
|---|---|---|
| **Host (macOS / Linux)** | **Trusted** | Docker daemon, gh CLI tokens, SSH keys — host-side helpers |
| **Main container** | **Untrusted** | Claude can be prompt-injected via any input file or chat message |
| **`claude-bridge` sidecar** | **Untrusted, and reachable** | a compose peer that *accepts* POST from the main container |

The container holds:
- `~/.claude-creds/.credentials.json` — refreshable Anthropic OAuth token, plan-scoped
- whatever `.env` carries. ⚠ **In this tree that includes a real GitHub PAT**
  (`EXT_PATCHES_TOKEN`, used by `ext-patches-sync` to fetch the private patchers repo). `.env` is
  gitignored (`../.gitignore:4`) and must stay so.

⚠ **Correction to the previous version of this document**, which claimed the container "never
holds a GitHub PAT". It does. Combined with `basic` — no body caps, no path scoping, no audit
log — a prompt-injected Claude that can read `.env` can POST it to any allowlisted host and
**leave no trace**. Rotate that token if you have any doubt, and prefer a fine-grained,
read-only, short-lived one.

## POST surface as declared (enforced only in `strict`)

POST is the dangerous verb — it carries a body. The committed baseline grants POST on **9 hosts**
via 10 directives in `../firewall/domains.txt`, bounded by `../firewall/policy.d/`:

| Host | Declared path | Body cap (`strict`) | Why |
|---|---|---|---|
| `api.anthropic.com` | `/v1/messages`, `/v1/complete` | 32768 kB | Claude API itself |
| | `/v1/files` | 50000 kB | file upload / download / delete |
| | `/api/event_logging` | 256 kB | CLI telemetry batches |
| | `/api/*` | 1024 kB | catch-all for Claude Code internal POSTs |
| `mcp-proxy.anthropic.com` | `/v1/mcp/*` | 4096 kB | MCP tool invocation payloads |
| `platform.claude.com` | auth flows | 64 kB | OAuth token refresh |
| `ollama.internal` | Messages-compatible | 32768 kB | local model **on the developer's machine** |
| `claude-bridge` | Messages-compatible | 32768 kB | in-compose sidecar |
| `*.statsig.com` | (any) | 10 kB *(host-level)* | feature-flag telemetry |
| `*.sentry.io` | (any) | 50 kB *(host-level)* | error reports |
| `marketplace.visualstudio.com` | `/_apis/public/gallery/extensionquery` | 64 kB | extension search |
| `github.com` | `/anthropics/*.git/git-upload-pack` | 10240 kB | git smart-pack fetch |

Two distinct mechanisms, not interchangeable: **`max_body_kb`** is *per endpoint*, inside a
`policy.d/<host>.yaml` `endpoints:` block ; **`max_body_size_kb`** is a *host-level* cap, for
telemetry hosts with no meaningful path structure.

⚠ `domains.txt`'s inline comments have drifted from the policies they point at (it says
"max 20MB" for `/v1/messages` where `policy.d` declares 32 MB). **`policy.d/` is what would
run.** And in `basic`, none of this column applies: every one of those hosts accepts any method
on any path with any body size, bounded only by the global URL/header limits — which are
themselves mitmproxy-side, so they do not apply either.

Global limits declared in `../firewall/compile-policy.py:64-69` (`strict` only):
`max_query_string_length: 256`, `max_url_total_length: 2048`, `max_header_count: 30`,
`max_header_value_length: 4096`.

## What this tree adds to the egress surface

Two targets the image's own threat model does not contemplate:

**`claude-bridge` — a POST-accepting peer inside the compose network.** It takes
`POST /v1/messages` at the same 32 MB ceiling as the cloud. Being a compose-internal name, it is
**not subject to DNS allowlisting at all**. A prompt-injected Claude that can POST 32 MB to a
peer has an exfil channel bounded only by what that peer does next: **the bridge's own egress is
the real control, not the firewall.** Mitigating fact — the sidecar sits behind a compose
profile, so it is usually not running. The POST grant is standing ; the listener is not.

**`ollama.internal` — allowlisted egress to the developer's own workstation.** It resolves to
the host gateway, so a POST leaves the container for the Mac. This deliberately trades "the
container cannot reach the host" for local-model work. Disable it in `domains.local.txt` when
you are not running a local model.

Neither is a defect ; both are decisions.

## Threat scenarios

### T1 — Data exfiltration via outbound network

**Vector**: Claude, prompt-injected via a malicious file, dependency, or chat content, tries to
POST `.env` (which holds a GitHub PAT), source code, or session secrets to a host it can reach.

**What mitigates it in `basic`**:
- **L1 (DNS + ipset)** — an unlisted host does not resolve, its IP is not in the ipset, and
  iptables drops the SYN. This is the whole defence, and it is real: the attacker must land on a
  host that is already on the list.
- IPv6 is dropped entirely (`init-firewall.sh:218-226`), so there is no v6 bypass.

**What does NOT mitigate it in `basic`** — and this is the honest part:
- any path on an allowlisted host accepts a POST, so `github.com/<attacker>/…`,
  `*.sentry.io/<anything>`, a Reddit or Medium endpoint from the local layer, or
  `api.anthropic.com/<anything>` are all open write channels ;
- there is **no body-size bound**, so volume is not capped ;
- archives and base64-wrapped archives are **not** detected ;
- base64 / hex / internal-path patterns in a URL are **not** detected ;
- and there is **no record**: `/var/log/mitmproxy-writes.log` is never written, so a successful
  exfiltration leaves nothing to find afterwards.

The practical reading: in `basic` the allowlist is a **reachability** control, not a
**content** control. It bounds *who* the container can talk to, and nothing about *what* it says.

### T2 — Supply chain compromise via `npm install`

**Vector**: a compromised npm package runs a malicious `postinstall`.

**Mitigations**: the npm allowlist is scoped in `domains.d/npm.txt` ; `/scan-deps` inventories
every dependency before adoption and writes a committed `domains.d/<eco>.txt`, so the change is
reviewable ; `../claude/CLAUDE-dev.md` recommends `npm install --ignore-scripts` for deps coming
from a research project ; and no cloud credentials live in the container's env.
⚠ In `basic`, the npm **path** scoping does not apply — `registry.npmjs.org` is reachable in
full, and the L4 archive detection that would inspect a postinstall's POST is not running.

### T3 — Prompt injection forcing destructive git or shell operations

**Vector**: a message or file instructs Claude to `gh pr create`, `git push --force`,
`curl evil.com`, or `rm -rf`.

**Mitigations**:
- **No `gh` write** — `gh` is installed but unauthenticated, and the Dev Containers credential
  relay answers nothing for `github.com`. PR creation goes through `/prepare-pr` → a host helper.
- **No `git push`** — no SSH key, and no POST grant for `git-receive-pack`.
  ⚠ In `basic` that second half does not hold: `github.com` accepts any path, so the firewall is
  not what stops a push. What stops it is the **absence of a credential** — except that `.env`
  now carries a PAT, which is why rotating it matters.
- **`curl evil.com`** — blocked by L1, as long as the host is not already allowlisted.
- **`rm -rf`** — not a firewall concern. The workspace is bind-mounted R/W ; the harness asks the
  user before risky tools, and `CLAUDE-dev.md` says to propose destructive commands, not run them.

### T4 — Side-channel via world-readable mitmproxy logs

**Not applicable in `basic`** — mitmproxy does not run, so the logs do not exist. In `strict`
they are `chmod 640`, `chown mitmproxy:adm`, with `node` in group `adm` so a human can read them
for debugging while unprivileged services cannot.

### T5 — Cross-pollination between projects

**Vector**: a research project for one client sees credentials from another because the
`claude-creds` volume is shared by default.

**Mitigation**: research projects set `DC_PROJECT=research-<task>`, which Compose substitutes
into every volume name, giving the container its own empty `claude-creds-research-<task>`.
Cleanup is manual — see `../host-helpers/`.

### T6 — CA-trust scope creep

**Vector**: a baked CA trusted by the OS could decrypt traffic from any tool.

**Mitigation**: the mitmproxy CA is **per-project**, in the `mitmproxy-${DC_PROJECT}` volume, so
it is only trusted inside that container. In `basic` no CA is generated or trusted at all, and
no proxy env is set — `init-firewall.sh:650-653` strips the proxy block from `/etc/environment`
and removes `/etc/profile.d/devcontainer-proxy.sh` unconditionally, re-adding it only in `strict`.

## Accepted gaps

### P1 — No drift check between the configured mode and the running one

Nothing compares the baked `/etc/devcontainer-firewall/default-mode` with the workspace
`../firewall/default-mode`. `../shell-init.sh:93-95` acknowledges it and works around it by
reading the baked copy, which is the one that was applied. Nothing asserts that the mode was
*effective* either — there is no post-boot check that mitmdump is listening when the file says
`strict`.

**Partly compensated**: `../post-start.sh:60-74` prints a loud banner at every container start
while the mode is `basic` — *"Firewall in BASIC mode (DNS allowlist only, no L7 filter)"* — with
the command to switch. `strict` is deliberately silent. So the operator is told, even when a
document is wrong.

### P2 — The gitignored local layer is baked

`domains.local.txt` is not reviewable in `git log`, yet it reaches runtime through
`Dockerfile:60`. Whatever a developer adds locally becomes part of the container's reachable set
with no audit trail. This is the intended trade (per-developer research needs), but it means the
committed allowlist is a floor, not a ceiling.

### P3 — DNS subdomain exfiltration

An attacker controlling a wildcard-allowlisted domain can have Claude emit DNS queries like
`<base64>.allowed-domain.com`. dnsmasq forwards, the attacker's NS records the payload.

**Why accepted**: it requires both control of an allowlisted domain and a successful prompt
injection. Mitigation cost (a DNS-inspection addon, or dnscrypt against a private resolver)
exceeds the marginal benefit at this level. Note the local layer widens the set of candidate
domains considerably.

### P5 — CONNECT to loopback through mitmproxy

`mitmproxy --mode regular` accepts `CONNECT 127.0.0.1:<port>`, so an in-container attacker can
reach a loopback listener through the proxy. `strict` only.

### P6 — Timing side-channel

Not mitigated. Constant-time responses on a proxy are impractical.

### P7 — Side-loading via Docker volumes

A user-mounted volume carrying an executable lets Claude run it. Out of scope beyond standard
Docker permissions. Don't mount volumes you have not verified.

## When `strict` is worth it, and what it costs

**Worth it** when the container handles anything you would mind leaking: a real credential in
`.env`, client source, anything under NDA. It is also the only mode that leaves a **record** —
without `passive_log.py` there is no post-mortem.

**What it costs**:
- a **rebuild**. The config is baked ; `../firewall/firewall-mode.sh strict` writes the flag and
  tells you so (`:120-123`) — Reload Window is not enough.
- every tool must honour `HTTPS_PROXY`. One that does not gets `REJECT`, not a clear error. Tools
  with their own TLS stack need `NODE_EXTRA_CA_CERTS` or an equivalent.
- the path scopes start being enforced, so anything reaching an undeclared path now gets a 403.
  `firewall-blocks` lists those ; expect a round of allowlist work on first switch.
- `max_body_kb` becomes real: a large upload to an endpoint with a small cap gets a 413.

**How to switch**: `bash .devcontainer/firewall/firewall-mode.sh strict`, then Rebuild Container,
then check `firewall-blocks` is empty. Reverting is the same command with `basic`.

## Security decisions log

1. **`basic` is the committed default for this tree** (`8a6ddaf`, "basic mode in dogfood"). This
   is the dogfood instance of the base image, where the work is on the container plumbing itself
   and a forced proxy gets in the way of measuring it. The decision stands ; this document is
   what was corrected to match it.
2. **POST on `api.anthropic.com` is required** for Claude Code to function. Caps declared in
   `policy.d/api.anthropic.com.yaml`.
3. **Telemetry POST kept** (`*.statsig.com` 10 kB, `*.sentry.io` 50 kB) with host-level caps, so
   that in `strict` an exfil disguised as an error report stays small. Disable with
   `!disable *.statsig.com` in `domains.local.txt`.
4. **POST git smart-pack** scoped to `/anthropics/*` for fetch and clone. Push is not granted.
5. **No wildcard-method host in `domains.txt`** — method wildcards belong in a
   `policy.local.d/<host>.yaml` override with a justification comment.
6. **CA baked, not generated at install** — only the cert lives in the per-project volume.
7. **The mode is not runtime-overridable.** It was an env var before the bake-only migration ;
   a workspace-writable mode was a bypass surface. `init-firewall.sh:128-131` records why.

## Hardening recommendations (host-side)

- **Rotate `EXT_PATCHES_TOKEN`** and keep it fine-grained, read-only and short-lived.
- **MFA + hardware key** on the GitHub account — the host holds the real credentials.
- **`gh` auth via OAuth device flow** rather than a static PAT.
- **SSH key with a passphrase + ssh-agent** — no plaintext key on disk.
- **Back up** `~/.config/gh/hosts.yml` and `~/.ssh/`, so a host compromise can be rotated from.
- **Audit `~/.claude-creds/.credentials.json`** periodically ; the OAuth refresh flow lets you
  re-authenticate without a rebuild.

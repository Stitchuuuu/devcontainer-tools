# Security — DevContainer Niveau 1 strict

> Threat model + security decisions for the Phase 3 devcontainer. Niveau 1 strict is the **default and only supported mode** for daily work. `basic` (DNS-only) and `off` (kill-switch) exist as escape hatches, not as production modes.
>
> See also: [README.md](README.md) for user-facing usage, [RUNBOOK.md](RUNBOOK.md) for operational procedures, [knowledge/INDEX.md](knowledge/INDEX.md) for internals.

## TL;DR

| Component | What | Goal |
|---|---|---|
| **DNS allowlist** (Layer 1) | dnsmasq forwards only listed hosts; iptables ipset drops the rest | block unlisted hosts entirely |
| **Path filtering** (Layer 2) | mitmproxy addon `policy_enforce.py` against `policy.compiled.yaml` | block `github.com/anyone/*` even though `github.com/anthropics/*` is allowed |
| **Method allowlist** (Layer 3) | POST allowed only on a tiny set | block exfiltration via POST body |
| **Content inspection** (Layer 4) | `format_detect.py`: magic bytes + base64-archive scan | block zip/gzip/7z/rar/xz uploads in POST body |
| **URL inspection** (Layer 5) | base64 / hex / internal-path detection in URL | block path-tunnelled exfil |
| **Size limits** (Layer 6) | `max_body_kb` per endpoint + global URL/header limits | bound exfil volume |
| **Audit trail** | `passive_log.py` logs every non-GET to `/var/log/mitmproxy-writes.log` | post-mortem investigation |
| **Trust boundary** | Host is trusted, container is untrusted | secrets stay host-side |

Accepted gaps (P3 / P5 / P6) are documented at the end and tracked for Phase 4+.

---

## Trust boundary

| Component | Trust | Why |
|---|---|---|
| **Host (macOS / Linux)** | **Trusted** | Docker daemon, gh CLI tokens, SSH keys, PAT — all host-side helpers |
| **Main container** | **Untrusted** | Claude can be prompt-injected via any input file or chat message |
| **Research container** | **Untrusted, scoped enlargement** | broader allowlist but workspace is a subset copy, secrets in `.env.local` user-filled |

The container holds only:
- `~/.claude-creds/.credentials.json` — refreshable Anthropic OAuth token, plan-scoped
- Transient secrets the user explicitly injects via `.env.local` (research only)

The container **never** holds: GitHub PAT, SSH keys, AWS credentials, production secrets.

---

## POST allowlist (the exact list)

POST is the dangerous verb (it carries a body). The strict main allows POST on **four targets only**:

| Host | Path | `max_body_kb` | Why |
|---|---|---|---|
| `api.anthropic.com` | `/v1/messages`, `/v1/files`, `/v1/usage` | 32768 (32 MB) | Claude API itself |
| `*.statsig.com` | (any) | 10 | Anthropic CLI telemetry — body bounded |
| `sentry.io` | (any) | 50 | Error reports — body bounded |
| `github.com` | `/anthropics/*.git/git-upload-pack` | 10240 | Git smart-pack fetch (clone/fetch). Path-filtered to `/anthropics/*` only |

**Everything else** (GitHub API POST, npm registry POST, generic webhooks, third-party APIs) is **blocked** at L3 by mitmproxy.

If you need POST on a host outside this list, spawn a research project with `/prepare-research` — do not extend the main allowlist. Research bundles can authorize POST on additional hosts in their own `policy.local.d/`, isolated in a separate container.

---

## Threat scenarios

### T1 — Data exfiltration via outbound network

**Vector**: Claude (prompt-injected via a malicious file, dependency, or chat content) tries to POST `.env`, source code, or session secrets to an attacker-controlled host.

**Mitigations**:
- **L1 (DNS)** — unlisted hosts don't resolve, ipset doesn't contain their IPs, iptables drops the SYN
- **L2 (path)** — `github.com/anthropics/*` is allowed but `github.com/anything-else` is dropped at mitmproxy
- **L3 (method)** — POST is allowed only on the 4 targets above
- **L4 (content)** — `format_detect.py` blocks magic bytes for `zip`, `gzip`, `7z`, `rar`, `xz`, `bzip2` in POST/PUT/PATCH bodies, **and** scans for base64-encoded archives in the first 65 kB of body
- **L5 (URL)** — `detect_base64_in_url`, `detect_hex_blob_in_url`, `detect_internal_path_leak` (any URL containing `/workspace/`, `/home/`, `/var/lib/`, `/etc/` triggers the detector)
- **L6 (size)** — `max_body_kb` per endpoint, `max_query_string_length: 256`, `max_url_total_length: 2048`, `max_header_value_length: 4096`

### T2 — Supply chain compromise via `npm install`

**Vector**: a compromised npm package runs a malicious `postinstall` script that tries to exfiltrate or pivot.

**Mitigations**:
- Main npm allowlist limited to `registry.npmjs.org/@anthropic-ai/*` and `/-/v1/search` (public registry is blocked)
- `/scan-deps` workflow inventories every dep before adoption and writes `firewall/domains.d/<eco>.txt` (committed, audit-trail in PR review)
- CLAUDE-dev.md recommends `npm install --ignore-scripts` when adding deps from a research project
- No AWS / Stripe / cloud credentials live in main container env vars
- L4 format detection on any POST a compromised postinstall might emit

### T3 — Prompt injection forcing destructive git or shell operations

**Vector**: a message or file instructs Claude `gh pr create -b "<exfil>"`, `git push --force`, `curl evil.com`, `rm -rf workspace/`.

**Mitigations**:
- **No `gh` write in main** — `gh-secure` was dropped in A3 (Phase 3 main does not COPY it into the image). PR creation goes through `/prepare-pr` → host helper (host has the trust boundary)
- **No `git push` possible** — no SSH key, no PAT mounted, and POST to `git-receive-pack` is not in the allowlist
- **`curl evil.com`** — blocked by L1 DNS (host doesn't resolve)
- **`rm -rf`** — not blocked by the firewall (workspace is bind-mounted R/W), but CLAUDE-dev.md instructs Claude to propose destructive commands rather than execute them. User authorization is required by the harness for risky tools.
- **CLAUDE-dev.md** explicitly says: tags, pushes, PR creation all go through propose-not-execute

### T4 — Side-channel via world-readable mitmproxy logs

**Vector**: a process running inside the container reads `/var/log/mitmproxy.log` or `/var/log/mitmproxy-writes.log` to recover tokens leaked in query strings.

**Mitigation** (A2): logs are `chmod 640` + `chown mitmproxy:adm`. User `node` is in group `adm` (Dockerfile `usermod -aG adm node`) so `node` can read logs for debug — but unprivileged services cannot.

### T5 — Cross-pollination between client projects

**Vector**: a research project for client A inadvertently sees credentials from client B because the `claude-creds` volume is `external: true` and shared by default.

**Mitigation**: research projects use `DC_PROJECT=research-<task>` → Docker Compose substitutes that into all volume names, giving the research container its own `claude-creds-research-<task>` volume (empty initially). Cleanup: `rm -rf ~/research-projects/<task>/ && docker volume rm research-<task>_claude-creds`. See `host-helpers/research-cleanup`.

### T6 — CA-trust scope creep

**Vector**: a baked CA trusted by the OS could decrypt traffic from any tool, not just curl/git.

**Mitigation**: the mitmproxy CA is **per-project** (stored in volume `mitmproxy-${DC_PROJECT}`, not shared). The volume is per-container, so its CA is only trusted inside that container. Reset = `docker volume rm <project>_mitmproxy-<project>` then container reopen → fresh CA at first boot.

---

## Layer 1 — DNS allowlist

```
container app outbound DNS query
        │
        ▼
  /etc/resolv.conf → 127.0.0.53 (dnsmasq, UID-restricted)
        │
        ├─ host listed → forward 8.8.8.8 + add resolved IP to ipset allowed-domains
        └─ host unlisted → NXDOMAIN

  UDP/53 outbound is owner-matched to UID=dnsmasq — apps cannot bypass via dig @8.8.8.8
```

Allowlist sources (compiled at boot by `compile-policy.py`):
- `firewall/domains.txt` — Claude-only baseline (17 hosts, committed)
- `firewall/domains.d/<eco>.txt` — per-ecosystem additive (npm.txt, ecosystem-docs.txt, …) committed, generated by `/scan-deps`
- `firewall/policy.d/<host>.yaml` — advanced rules per host (max_body, query schemas, blocked paths)
- `firewall/domains.local.txt` + `firewall/policy.local.d/` — per-dev overrides (gitignored OR committed)

See [POLICY-MODEL.md in the rollout docs](../plans/devcontainer-v2/phase3-rollout/docs/POLICY-MODEL.md) for the full syntax and precedence rules.

## Layer 2-6 — mitmproxy enforcement (strict mode only)

Apps reach mitmproxy via `HTTPS_PROXY=http://127.0.0.1:8080` (propagated through `.env` → `docker-compose env_file:` → PID 1; `/etc/environment`; `/etc/profile.d/`). The iptables outbound chain only accepts packets from UID `mitmproxy` against the ipset. Apps that bypass the proxy hit `REJECT`.

mitmproxy loads four addons (`firewall/addons/`):

1. **`policy_enforce.py`** — sequential checks: URL/header limits → host lookup with wildcard subdomain → method allowlist → `blocked_paths` regex → `endpoints` regex (path + method + body size + query schema) → URL detection (base64/hex/internal-path) → magic-byte detection.  Fail-closed: any exception returns 503.
2. **`format_detect.py`** — POST/PUT/PATCH only. Magic bytes scan on `body[:1024]`. Base64-archive scan on `body[:65536]` (bounded CPU). Configurable via `defaults.block_archive_magic` / `block_archive_in_base64`.
3. **`passive_log.py`** — appends `{ts, method, host, path, size, ct, ua}` JSON line per non-GET request to `/var/log/mitmproxy-writes.log` (mode 640, group `adm`).
4. **`stream_sse.py`** — passthrough for SSE responses (otherwise mitmproxy would buffer the whole stream).

---

## Audit trail

`/var/log/mitmproxy-writes.log` is the canonical record of every POST/PUT/PATCH/DELETE the container has emitted. Format:

```json
{"ts": 1746950400.123, "method": "POST", "host": "api.anthropic.com", "path": "/v1/messages", "size": 4523, "ct": "application/json", "ua": "anthropic-cli/2.1.119"}
```

Use cases:
- Confirm no unexpected POST leaked during a session
- Post-mortem on a suspected compromise
- Spot patterns: unusual volume, frequency, or hosts

`/var/log/mitmproxy.log` holds CONNECT events and errors (mode 640).

`firewall/domains.d/<eco>.txt` files are committed → every dependency-driven allowlist change shows up in `git log` / PR review.

---

## Accepted gaps

### P3 — DNS subdomain exfiltration

An attacker who controls a wildcard-allowlisted domain can have Claude make DNS queries like `<base64-data>.allowed-domain.com`. dnsmasq forwards the query to 8.8.8.8, the attacker's NS records the encoded content.

**Why accepted**: requires (a) control of an allowlisted domain AND (b) successful prompt injection forcing Claude to craft these queries. The set of allowlisted hosts is small (Anthropic, GitHub, marketplace.visualstudio.com…) and not under realistic attacker control. Mitigation cost (DNS-inspection addon or dnscrypt with private resolver) > marginal benefit at Niveau 1.

**Future**: addon mitmproxy DNS-inspection in Phase 4, or dnscrypt with private DoH endpoint.

### P5 — CONNECT to loopback via mitmproxy regular mode

mitmproxy in `--mode regular` accepts `CONNECT 127.0.0.1:<port>`. If a dnsmasq listener or another service exposes a port on loopback, an in-container attacker can reach it through the proxy.

**Future**: `--ignore-hosts` filter or a CONNECT-side addon.

### P6 — Timing side-channel

Response timing can leak information. Not mitigated — cost is prohibitive (constant-time responses on a proxy are impractical).

### P7 — Side-loading via Docker volumes

A user-mounted volume containing an executable binary lets Claude run that binary. Defense beyond standard Docker permissions is out of scope.

**Mitigation in usage**: don't mount unknown / unverified volumes. The workspace bind mount is R/W but it's the project sources, already under user control.

---

## Security decisions log

1. **POST `api.anthropic.com` allowed** — required for Claude API. `max_body_kb: 32768` on `/v1/messages` (aligned with context window). Body inspected by `passive_log.py`.
2. **Telemetry POST kept** (`*.statsig.com` 10 kb, `sentry.io` 50 kb) — body sizes severely bounded to prevent exfil disguised as fake error reports. Devs who want it off can `!disable *.statsig.com` in `domains.local.txt` (template line is pre-written in `domains.local.txt.example`).
3. **POST git smart-pack** (`github.com/anthropics/*.git/git-upload-pack`, `max_body_kb: 10240`) — required for fetch/clone. Path filter enforces the `anthropics` scope.
4. **No wildcard `[*]` host methods in `domains.txt`** committed — only allowed in `policy.local.d/<host>.yaml` overrides with a justification comment.
5. **Research isolation = volume isolation** (`DC_PROJECT=research-<task>` substitution). No auto-destroy after N days; cleanup is manual (`research-cleanup` helper is dry-run by default, `--apply` required).
6. **CA baked, not generated at install** — the mitmproxy binary is baked into the image (A3, ~80 MB). Only the CA cert lives in the per-project `mitmproxy-${DC_PROJECT}` volume.
7. **No telemetry to Anthropic Console** outside what the Claude binary natively emits to `*.statsig.com` and `sentry.io`. No extra layer.

---

## Hardening recommendations (host-side)

- **MFA + hardware key** on the GitHub account — host PAT and SSH keys are sensitive
- **gh CLI auth via OAuth device flow** preferable to a static PAT
- **SSH key with passphrase + ssh-agent** — no plaintext key on disk
- **Regular backup** of `~/.config/gh/hosts.yml` + `~/.ssh/` (in case the host is compromised and you need to rotate)
- **Periodic audit** of `~/.claude-creds/.credentials.json` (revoke the Anthropic token if you suspect leakage; the OAuth refresh flow lets you re-auth without rebuild)

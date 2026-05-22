# Rollout — Devcontainer Security Hardening V2

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).

## Goal

Close the DNS exfiltration channel (#9 gap P3) identified by the v1
rollout's adversarial validation (cf.
[../devcontainer-security-hardening/adversarial-report.md](../devcontainer-security-hardening/adversarial-report.md)).

**The gap** : `dnsmasq.conf` has `server=127.0.0.11` as default upstream
— so every non-allowlisted query is forwarded via Docker DNS → host DNS
→ public DNS hierarchy. The allowlist is enforced at iptables/ipset
level (not at dnsmasq), which means the DNS query itself leaks before
iptables drops the connection. An attacker controlling a public NS for
their own domain receives the payload as arbitrary subdomain queries.

**The fix** :

1. **Drop `server=127.0.0.11` catch-all** from `dnsmasq.conf`. Without
   a default server, dnsmasq returns REFUSED for unmatched queries and
   the query never leaves the container.
2. **Generalise the claude-bridge pattern** in `init-firewall.sh` : for
   each entry of `direct-tcp-allow.txt` (`host:port`), emit
   `server=/<host>/127.0.0.11` in the generated dnsmasq config (auto
   sibling-resolve via the existing source-of-truth file).
3. **No wildcard parent promotion** — a compromised npm package could
   exploit `*.parent.com` for arbitrary subdomain C2. The `*.domain.com`
   syntax stays available in `domains.txt` (already used for
   `*.statsig.com`, `*.githubusercontent.com`, etc.), opt-in only for
   vendor-trusted parents.

**Success criteria** :

- `dig non-allowlisted @127.0.0.53` → REFUSED (was : returns real IP)
- `dig allowlisted @127.0.0.53` → resolves normally
- Typical Claude Code workflows pass (file ops, Anthropic API, MCP,
  npm install if applicable)
- `test-firewall.sh` passes without regression
- PoC #9 (DNS exfil channel) empirically closed

## Navigation

| File | When to open |
|---|---|
| **[STATUS.md](STATUS.md)** | "Where are we, what's next ?" — actionable session table |
| **[LOG.md](LOG.md)** | "What was done, why, what gotchas ?" — append-only journal |
| **[EXISTING.md](EXISTING.md)** | "What does the code look like today ?" — factual inventory |
| sessions/session-NN-*.md | Prompt to paste into a new Claude chat to start session NN |

## How to use

1. **To resume work** : open [STATUS.md](STATUS.md), find the next 📋
   session, click `→ prompt` and paste into a fresh Claude Code session.
2. **To check what was done before** : read [LOG.md](LOG.md).
3. **To understand current code state** : read [EXISTING.md](EXISTING.md).

## Update convention (end of every delivered session)

Every session prompt prescribes these three updates in its DoD :

1. **STATUS.md** : flip the session row 📋 → ✅, replace the prompt link
   with `—`, bump the "Delivered" counter, refresh "Next focus".
2. **LOG.md** : append `## <Session ID> — <Title>` section dated today,
   listing files touched + What / Why / Decisions / Gotchas / Tests /
   Commit (~50–150 lines).
3. **EXISTING.md** : update if new files / structures were created.

No companion skill, no automated hook — the session itself does the work
because its prompt explicitly says so.

## Decisions (immutable unless user explicitly amends)

- **2026-05-22 — No wildcard parent promotion** : a malicious npm
  package installed in the project could otherwise create a C2 channel
  under a legitimate parent domain (e.g. `c2.anthropic.com` if
  `*.anthropic.com` were allowed). Keep `domains.txt` granular per
  subdomain. The `*.domain.com` syntax stays available for explicit
  opt-in (already used for `*.statsig.com`, `*.githubusercontent.com`,
  etc.) but no automatic promotion is performed by v2.

- **2026-05-22 — `basic` mode is NOT an alternative** : `basic` mode
  skips mitmproxy but keeps dnsmasq + ipset with the same baked config
  (catch-all `server=127.0.0.11`). The DNS exfil channel is open in
  both `strict` and `basic` modes. v2 fixes the dnsmasq config, which
  benefits both modes.

- **2026-05-22 — Reuse `direct-tcp-allow.txt` as sibling DNS source** :
  the file already lists `host:port` entries for non-HTTP direct-TCP
  bypass. Generalise the existing claude-bridge hardcoded logic
  (`init-firewall.sh:290-295`) to emit `server=/<host>/127.0.0.11`
  for every entry. Zero new source-of-truth — `claude-switch` already
  populates this file per mode (cloud/local-bridge/local-direct).

- **2026-05-22 — Scan-deps before tightening** : before serrant
  dnsmasq, run `/scan-deps` on the project to identify domains hit by
  `npm install` (postinstall hooks). Pre-allowlist these so the first
  strict-mode rebuild doesn't break package installation.

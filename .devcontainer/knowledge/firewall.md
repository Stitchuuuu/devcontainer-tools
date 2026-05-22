# Firewall — internals & strict mode (force-proxy)

## Web search & research policy

The main devcontainer firewall is **strict by default** (Niveau 1 strict —
17-host Claude-only baseline). The three modes :

| Mode | DNS filtering | L7 (mitmproxy) inspection | Outbound to non-allowlisted host |
|---|---|---|---|
| `strict` (default) | ✅ enforced | ✅ enforced | blocked |
| `basic` (escape hatch) | ✅ enforced | ❌ off | blocked at DNS — still fails |
| `off` (kill-switch) | ❌ off | ❌ off | allowed |

**Key consequence** : even in `basic`, outbound to a host outside the
baseline fails because DNS resolution is filtered. `WebFetch` / `WebSearch`
against arbitrary hosts will silently fail in strict and basic alike.

### Trigger for `/prepare-research`

When the user asks for :
- Up-to-date docs, web research, "explore X library", "read about Y"
- Reading sources outside the 17-host baseline
- Third-party API integration (POST to new hosts — Stripe, Linear, …)
- Package evaluation involving registries beyond the baseline

→ **Don't WebFetch/WebSearch silently and fail.** Surface the firewall
constraint and offer one of two sanctioned paths :

1. **Targeted allowlist addition** — propose appending the specific host(s)
   to `firewall/domains.local.txt` (+ a `policy.local.d/<host>.yaml` if POST
   needed). Suitable for : 1–2 read-only domains, ad-hoc single-session
   lookup. The user must Rebuild Container for the change to take effect.
2. **`/prepare-research`** (preferred when in doubt) — spawn a scoped
   research devcontainer with its own expanded allowlist + isolated volumes.
   Suitable for : multi-domain exploration, third-party API integration,
   package evaluation, anything multi-session.

**Default to `/prepare-research`** when the scope is unclear or non-trivial
— it's the sanctioned, audited path that avoids polluting the main
`domains.local.txt`. Reserve the targeted allowlist for genuinely
single-host, single-session reads.

---

## Firewall internals — pipeline compile

`init-firewall.sh` at boot invokes:

```
compile-policy.py --config-dir=/etc/devcontainer-firewall \
                  --out-dnsmasq=/var/run/devcontainer-firewall/dnsmasq-domains.conf \
                  --out-policy=/var/run/devcontainer-firewall/policy.compiled.yaml
```

Pipeline:
1. **Parse** `domains.txt` (Claude-only baseline, 17 hosts)
2. **Parse** `domains.d/*.txt` alphabetically (per-ecosystem; **F2**)
3. **Merge** same-host entries cross-file: methods union, paths concat
4. **Deep-merge** `policy.d/<host>.yaml` (top-level keys replace)
5. **Apply** `domains.local.txt`:
   - `!disable host` → delete from compiled output + log override
   - other line → **REDEFINE** host (wipe baseline paths, replace methods)
6. **Deep-merge** `policy.local.d/<host>.yaml`
7. **Emit atomically** (`.tmp` + `os.rename()`):
   - dnsmasq.conf — flat hosts (`server=/host/8.8.8.8` + `ipset=/host/allowed-domains`)
   - `policy.compiled.yaml` — schema `{defaults, domains, runtime}` consumed by mitmproxy addons

**`runtime._overrides_applied`** in compiled YAML is the machine-readable audit of every local override (action ∈ `{disable, disable-nop, redefine, merge}`).

### compile-policy.py modes

- `--parse-only --json <files>` — emit `{entries, errors}` for tests (no yaml dependency)
- `--list-hosts <files>` — flat dedup after `!disable`; used by `init-firewall.sh` for probes + connectivity tests
- `--compile [--config-dir DIR]` — full pipeline; requires `yaml` module (installed in container via `python3-yaml` apt)

The two-mode split exists because the parser must run on the host without yaml installed (CI tests). Only `--compile` lazy-imports yaml and fails fast if absent.

## Firewall strict mode (force-proxy)

> Renamed from `paranoid` in A4 (deprecated alias still accepted). Pairs with `basic` (renamed from `okeish`) and `off` (unchanged).

### Architecture

`strict` mode runs mitmproxy as an **explicit forward HTTP proxy** on
`127.0.0.1:8080`. Apps reach it via `HTTPS_PROXY` env var. Apps that bypass
the proxy are blocked because the iptables filter chain only ACCEPTs outbound
from the `mitmproxy` UID :

```
iptables -A OUTPUT -m owner --uid-owner mitmproxy -m set --match-set allowed-domains dst -j ACCEPT
iptables -A OUTPUT -j REJECT --reject-with icmp-admin-prohibited
```

So the only paths to the internet are :
- **App → mitmproxy → external** (via HTTPS_PROXY ; mitmproxy resolves via dnsmasq, outbound gated by ipset)
- **App → CLAUDE_CODE_FIREWALL_ALLOWED host** (direct, no UID restriction — *avoid in production*)

### Why force-proxy and not transparent REDIRECT

The original plan was **transparent mode** with `iptables -t nat REDIRECT --to-port 8080`
in OUTPUT chain. After many iterations this proved non-functional in Docker :

- `route_localnet=1` set on `all`/`lo`/`eth0` (per-interface required, `all` doesn't propagate)
- `accept_local=1` set on `lo`/`all`
- `MASQUERADE -o lo` to fix src-routing
- Counter showed 256 packets REDIRECT'd, but `mitmproxy.log` stayed empty

Root cause never fully isolated — likely a combination of Docker namespace quirks
+ kernel routing behavior with REDIRECT'd loopback packets. **Force-proxy was simpler
to implement, more robust, and gives a stronger guarantee** (apps that ignore the
proxy are *blocked*, not silently bypassed).

### HTTPS_PROXY propagation (3 layers)

Setting `HTTPS_PROXY` reliably for ALL processes (terminals, VS Code Server,
extensions, daemons) requires multiple paths because each process tree gets env
differently :

| Layer | Mechanism | Covers |
|---|---|---|
| 1 | `.env` (initialize.sh) → `docker-compose env_file:` | PID 1 + all `docker exec` children (incl. VS Code Server, extensions) |
| 2 | `/etc/environment` (init-firewall.sh) | PAM-based sessions (sudo, ssh-like, some daemons) |
| 3 | `/etc/profile.d/devcontainer-proxy.sh` (init-firewall.sh) | login shells (`bash -l`) |

`shell-init.sh` is *not* used for HTTPS_PROXY (env_file already covers terminals).
It still exports `REQUESTS_CA_BUNDLE`/`SSL_CERT_FILE`/`GIT_SSL_CAINFO` for tools
that don't read the system trust store.

### sysctls + Docker — pitfall

`docker-compose.yml` `sysctls:` accepts namespaced sysctls. **Per-interface
sysctls** (`net.ipv6.conf.eth0.disable_ipv6`) are rejected at the service level
for interfaces created by the netdriver — must use `networks.driver_opts.com.docker.network.endpoint.sysctls`
with `IFNAME` placeholder.

```yaml
services:
  app:
    sysctls:
      - net.ipv6.conf.all.disable_ipv6=1
      - net.ipv6.conf.lo.disable_ipv6=1   # 'lo' OK at service level
      # - net.ipv6.conf.eth0.disable_ipv6=1   # ❌ rejected by Docker

networks:
  default:
    driver_opts:
      com.docker.network.endpoint.sysctls: net.ipv6.conf.IFNAME.disable_ipv6=1
```

Setting `all` does NOT propagate to per-interface in Docker (contrary to bare
metal Linux). Always set `lo` + `default` + `IFNAME` explicitly.

### Diagnose

```bash
.devcontainer/tests/diagnose.sh                     # 125 PASS/FAIL checks
.devcontainer/tests/diagnose.sh <container> --verbose  # + state dumps
.devcontainer/tests/diag-a2.sh                      # mode-specific snapshot → tests/diag-a2-<mode>.log
```

Exit code 0 = all green, 1 = failures.

### Reset mitmproxy install (corrupted CA etc.)

```bash
docker volume rm mitmproxy-${DC_PROJECT}    # nuke CA (binary is baked in image since A3)
# Restart container → CA regenerated on next strict boot
```

The mitmproxy binary itself lives at `/opt/mitmproxy/` inside the image (baked
at build time, version pinned via `ARG MITM_VERSION` in the Dockerfile — see
Phase 3 A3 in the rollout log). The volume only holds the persistent CA cert.

### mitmproxy bundle: ruamel.yaml only, never PyYAML

The mitmproxy distribution we use (`mitmproxy-${MITM_VERSION}-linux-*.tar.gz`
from `downloads.mitmproxy.org`) is a **PyInstaller bundle** with its own
embedded Python interpreter (3.14 as of 12.2.3). The bundle ships:

- ✅ `ruamel.yaml` — used internally by mitmproxy for its config.
- ❌ `PyYAML` (`import yaml`) — NOT shipped, mitmproxy doesn't use it.

Any `--scripts` addon that does `import yaml` will crash at module-load with
`ModuleNotFoundError: No module named 'yaml'`, and mitmdump will exit before
binding port 8080. The official mitmproxy error message redirects to
"install mitmproxy from PyPI" — but doing that brings the runtime-pip venv
back, defeating the A3 image-bake.

**Pattern to use in `firewall/addons/*.py`:**

```python
from ruamel.yaml import YAML
_YAML = YAML(typ='safe')

with open(POLICY_PATH) as f:
    POLICY = _YAML.load(f) or {}
```

`YAML(typ='safe').load()` returns plain Python `dict` / `list` / scalars,
fully interchangeable with `yaml.safe_load()` for the schema we use in
`policy.compiled.yaml`. No external dependency, no venv, no extra image
weight — ruamel is already in the bundle because mitmproxy itself imports it.

**For `compile-policy.py`** (runs in container `python3`, not in the bundle):
keep `import yaml` — it has `python3-yaml` apt available and is faster /
simpler than ruamel for the bulk YAML dump it does. Only addons inside the
mitmproxy bundle need the swap.

**If you bump `MITM_VERSION`**, sanity-check the bundle still ships ruamel:

```bash
/opt/mitmproxy/mitmdump --scripts /tmp/probe.py --listen-port 18099 2>&1 | head
# where /tmp/probe.py is:
# from ruamel.yaml import YAML; print('ruamel ok')
```

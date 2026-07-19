#!/usr/bin/env bash
# reload-local.sh — Hot-reload the local firewall layer without rebuild.
#
# Runs in `basic` and `strict` modes (see wall-guard below). Flushes only
# the `allowed-domains-local` ipset + restarts dnsmasq (SIGHUP is NOT
# enough, it doesn't re-parse --conf-file). Baseline (`allowed-domains-base`)
# and its iptables ACCEPT rule stay untouched — active connections keep
# flowing.
#
# In strict, zero-downtime is a dual mechanism:
#   1. This script recompiles policy.compiled.yaml via atomic_write() and
#      flushes only the local ipset — baseline mitmproxy connections stay
#      alive across the reload.
#   2. mitmproxy addons (policy_enforce.py, format_detect.py) mtime-check
#      policy.compiled.yaml on every request and reload it in-place when
#      the file changes — no process restart, no SIGHUP.
#
# Root-only by design. Sudo passwordless is intentionally NOT baked into
# /etc/sudoers.d because it would let any node-uid process mutate the
# firewall allowlist. Elevation goes through docker exec -u 0 from the
# host (or `wtf firewall reload` which auto-detects the app container).
#
# Usage:  sudo .devcontainer/reload-local.sh

set -uo pipefail

# The workspace firewall dir is the source of truth (edits happen here).
# Default matches devcontainer.json's workspaceFolder — override via env
# if you mount the workspace elsewhere. Note: BASH_SOURCE heuristic would
# fail once the script is installed at /usr/local/bin/reload-local.sh via
# the base-image bake (it'd look at $SCRIPT_DIR/firewall which doesn't
# exist there), silently skipping the resync — so we hardcode the path.
WORKSPACE_FW_DIR="${WORKSPACE_FW_DIR:-/workspace/.devcontainer/firewall}"

SYSTEM_FW_DIR="/etc/devcontainer-firewall"
DEFAULT_MODE_FILE="$SYSTEM_FW_DIR/default-mode"
RUNTIME_DIR="/var/run/devcontainer-firewall"
BASE_CONF="$RUNTIME_DIR/dnsmasq-domains-base.conf"
LOCAL_CONF="$RUNTIME_DIR/dnsmasq-domains-local.conf"
SYSTEM_DNSMASQ_CONF="$SYSTEM_FW_DIR/dnsmasq.conf"

# 1. Wall-guard — accept 'basic' and 'strict', refuse everything else.
MODE=""
if [ -r "$DEFAULT_MODE_FILE" ]; then
  MODE=$(tr -d '[:space:]' < "$DEFAULT_MODE_FILE")
fi
if [ "$MODE" != "basic" ] && [ "$MODE" != "strict" ]; then
  echo "❌ reload-local.sh runs in 'basic' or 'strict' firewall mode only. Current: '$MODE'" >&2
  if [ "$MODE" = "off" ]; then
    echo "   In 'off' mode there is no firewall to reload — nothing to do." >&2
  fi
  if [ "$MODE" = "okeish" ]; then
    echo "   'okeish' is a removed legacy alias — update $DEFAULT_MODE_FILE to 'basic'." >&2
  fi
  exit 1
fi

# 2. Root required for /etc/, /var/run/, ipset, and dnsmasq restart.
# Sudo passwordless is intentionally NOT configured — that would let any
# node-uid process mutate the allowlist. Elevation MUST go through
# docker exec -u 0 from the host (or `wtf firewall reload` which wraps
# the same call — see .devcontainer/.wtfcmd.yaml).
if [ "$(id -u)" -ne 0 ]; then
  echo "❌ reload-local.sh must run as root (touches ipset + dnsmasq + $SYSTEM_FW_DIR)" >&2
  echo "   From the host:   docker exec -u 0 <container> $0" >&2
  echo "   From the host:   wtf firewall reload  (auto-detects the container)" >&2
  echo "   (sudo passwordless refused by design — see script header)" >&2
  exit 1
fi

start_ns=$(date +%s%N)

# 3. Resync workspace → system config dir.
if [ -f "$WORKSPACE_FW_DIR/domains.local.txt" ]; then
  cp -a "$WORKSPACE_FW_DIR/domains.local.txt" "$SYSTEM_FW_DIR/domains.local.txt"
fi
if [ -d "$WORKSPACE_FW_DIR/policy.local.d" ]; then
  mkdir -p "$SYSTEM_FW_DIR/policy.local.d"
  # Remove stale files then copy fresh set (handles deletions in workspace).
  find "$SYSTEM_FW_DIR/policy.local.d" -maxdepth 1 -type f -name '*.yaml' -delete 2>/dev/null || true
  find "$WORKSPACE_FW_DIR/policy.local.d" -maxdepth 1 -type f -name '*.yaml' -exec cp -a {} "$SYSTEM_FW_DIR/policy.local.d/" \; 2>/dev/null || true
fi

# 4. Recompile the split-local dnsmasq confs. We rewrite base too because
# apply_local() can redefine baseline hosts (methods/paths) — a no-op
# semantically in basic mode (paths ignored) but keeps the artifact
# coherent with the source config.
python3 /usr/local/bin/compile-policy.py \
  --config-dir "$SYSTEM_FW_DIR" \
  --split-local \
  --out-dnsmasq-base  "$BASE_CONF" \
  --out-dnsmasq-local "$LOCAL_CONF" \
  --out-policy        "$RUNTIME_DIR/policy.compiled.yaml"
chmod 644 "$BASE_CONF" "$LOCAL_CONF"

# 5. Flush only the local ipset. Base stays intact — active connections
# to baseline hosts keep working across the reload.
ipset flush allowed-domains-local

# 6. Restart dnsmasq. SIGHUP is NOT sufficient — per dnsmasq(8), SIGHUP
# only re-reads /etc/hosts, /etc/ethers, --addn-hosts, --hostsdir, and
# --dhcp-* files. It does NOT re-parse --conf-file arguments, so new
# `server=` and `ipset=` lines in the recompiled local conf would be
# ignored. Full restart is required; still stays sub-500ms in practice.
pkill -x dnsmasq 2>/dev/null || true
sleep 0.2

DNSMASQ_USER=dnsmasq
if ! id -u "$DNSMASQ_USER" >/dev/null 2>&1; then
  DNSMASQ_USER=nobody
  if ! id -u "$DNSMASQ_USER" >/dev/null 2>&1; then
    DNSMASQ_USER=""
  fi
fi

if [ -n "$DNSMASQ_USER" ]; then
  dnsmasq \
    --conf-file="$SYSTEM_DNSMASQ_CONF" \
    --conf-file="$BASE_CONF" \
    --conf-file="$LOCAL_CONF" \
    --user="$DNSMASQ_USER"
else
  dnsmasq \
    --conf-file="$SYSTEM_DNSMASQ_CONF" \
    --conf-file="$BASE_CONF" \
    --conf-file="$LOCAL_CONF"
fi
echo "  ✔ dnsmasq restarted (SIGHUP does NOT re-parse --conf-file)"

# 7. Summary — elapsed + host counts per group.
end_ns=$(date +%s%N)
elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
base_count=$(grep -c '^server=/' "$BASE_CONF" 2>/dev/null; true)
local_count=$(grep -c '^server=/' "$LOCAL_CONF" 2>/dev/null; true)
base_ips=$(ipset list allowed-domains-base  2>/dev/null | grep -cE '^([0-9]{1,3}\.){3}[0-9]{1,3}'; true)
local_ips=$(ipset list allowed-domains-local 2>/dev/null | grep -cE '^([0-9]{1,3}\.){3}[0-9]{1,3}'; true)

echo "  elapsed: ${elapsed_ms}ms  |  hosts: base=${base_count} local=${local_count}  |  ipset IPs: base=${base_ips} local=${local_ips}"
if [ "$MODE" = "strict" ]; then
  echo "  ✔ mitmproxy addons reload policy.compiled.yaml via mtime-check (zero-downtime)"
fi

#!/usr/bin/env bash
# host/test-firewall-iptables.sh — privileged firewall checks via `docker exec`.
#
# Runs from the HOST machine. Uses `docker exec -u root <container>` to
# inspect iptables / ipset, which require CAP_NET_ADMIN (not available to
# the `node` user inside the container, and only init-firewall.sh /
# test-firewall.sh are sudoers-allowed — not arbitrary iptables).
#
# Discovers the devcontainer via `docker compose ps -q app` from the
# .devcontainer/ directory.
#
# Usage (from host) :
#   bash .devcontainer/tests/host/test-firewall-iptables.sh
#   bash <repo>/templates/v2/tests/host/test-firewall-iptables.sh   # dev

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"

# Guard : refuse to run inside the container. Without this guard, the script
# would call `docker compose` from the container (where docker CLI may or
# may not be on PATH but doesn't see the host's compose project), giving a
# confusing failure mode instead of a clear "wrong place" error.
if in_container; then
  echo "${RED}✗ This script must run on the HOST, not inside the devcontainer.${RST}" >&2
  echo "  It uses 'docker exec' against the running container, which needs" >&2
  echo "  the host's Docker socket + project context." >&2
  echo "  From your host shell : bash $(dirname "${BASH_SOURCE[0]}" | sed 's|/workspace|<repo>|')/test-firewall-iptables.sh" >&2
  exit 2
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "${RED}✗ docker CLI not on PATH${RST}" >&2
  exit 2
fi

# Locate the docker-compose.yml — walk up from this script's dir until we
# find one. Handles both layouts : .devcontainer/tests/host/ → .devcontainer/,
# and templates/v2/tests/host/ → templates/v2/.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_DIR="$SCRIPT_DIR"
while [ "$COMPOSE_DIR" != "/" ] && [ ! -f "$COMPOSE_DIR/docker-compose.yml" ]; do
  COMPOSE_DIR="$(dirname "$COMPOSE_DIR")"
done
if [ ! -f "$COMPOSE_DIR/docker-compose.yml" ]; then
  echo "${RED}✗ Could not locate docker-compose.yml walking up from $SCRIPT_DIR${RST}" >&2
  exit 2
fi

CID=$(docker compose -f "$COMPOSE_DIR/docker-compose.yml" ps -q app 2>/dev/null | head -1)
if [ -z "$CID" ]; then
  echo "${RED}✗ devcontainer 'app' service not running. Start it via Reopen-in-Container or 'docker compose up -d app'.${RST}" >&2
  exit 2
fi

# Helper : run a command in the container as root, capturing stdout.
in_root() {
  docker exec -u root "$CID" "$@" 2>/dev/null
}

# -----------------------------------------------------------------------------
# Tests
# -----------------------------------------------------------------------------

test_iptables_output_has_drop() {
  # In strict / basic mode : iptables OUTPUT chain must have at least one
  # DROP rule (default-drop pattern). In off mode, this skips.
  local mode; mode=$(in_root cat /etc/devcontainer-firewall/default-mode | tr -d '[:space:]')
  case "$mode" in
    strict|basic) ;;
    off) skip_test "mode = off : iptables is flushed by design" ; return ;;
    *)   skip_test "unknown mode '$mode'" ; return ;;
  esac
  if in_root iptables -L OUTPUT -n | grep -q DROP; then
    _ok "iptables OUTPUT has DROP rules (mode=$mode)"
  else
    _nok "iptables OUTPUT has NO DROP rules (mode=$mode — firewall not applied?)"
  fi
}

test_iptables_off_mode_flushed() {
  local mode; mode=$(in_root cat /etc/devcontainer-firewall/default-mode | tr -d '[:space:]')
  [ "$mode" = "off" ] || { skip_test "mode = $mode, not off"; return; }
  if in_root iptables -L OUTPUT -n | grep -q DROP; then
    _nok "iptables OUTPUT still has DROP rules in off mode (flush didn't happen)"
  else
    _ok "iptables OUTPUT no DROP rules (off mode kill-switch effective)"
  fi
}

test_ipset_allowed_domains_present() {
  local mode; mode=$(in_root cat /etc/devcontainer-firewall/default-mode | tr -d '[:space:]')
  [ "$mode" = "off" ] && { skip_test "mode = off : ipset not used"; return; }
  if ! in_root ipset list allowed-domains >/dev/null 2>&1; then
    _nok "ipset allowed-domains missing (init-firewall.sh failure?)"
    return
  fi
  # Use ipset's own "Number of entries:" header line — more robust than
  # regex-matching the IP members (whose format varies with ipset version
  # and may include trailing tokens like 'timeout 0').
  local count
  count=$(in_root sh -c "ipset list allowed-domains 2>/dev/null | awk '/^Number of entries:/ {print \$NF}'")
  count=$(printf '%s' "${count:-0}" | tr -d '[:space:]')
  if [ "${count:-0}" -gt 0 ] 2>/dev/null; then
    _ok "ipset allowed-domains populated ($count entries)"
  else
    # Dump the first 20 lines of the actual output for diagnosis — empty
    # output here is a strong signal that docker-exec returned nothing or
    # ipset isn't actually populated, not just a parsing miss.
    local dump
    dump=$(in_root sh -c "ipset list allowed-domains 2>&1" | head -20)
    _nok "ipset allowed-domains exists but empty (DNS warm-up failed?)"
    echo "    ipset output (first 20 lines) :"
    echo "$dump" | sed 's/^/      /'
  fi
}

test_mitmproxy_listening() {
  local mode; mode=$(in_root cat /etc/devcontainer-firewall/default-mode | tr -d '[:space:]')
  [ "$mode" = "strict" ] || { skip_test "mode = $mode, mitmproxy only in strict"; return; }
  if in_root ss -ltn 2>/dev/null | grep -q :8080; then
    _ok "mitmproxy listening on :8080 (root-visible via ss)"
  else
    _nok "mitmproxy NOT listening on :8080"
  fi
}

run_tests

#!/usr/bin/env bash
# Connectivity tests — run *after* init-firewall.sh has set up iptables /
# ipset / dnsmasq / mitmproxy. Verifies that allowed domains are reachable
# through mitm and that blocked ones are dropped. Reads the probes cache
# persisted by init-firewall.sh (so subdomain discovery isn't redone).
#
# Invoked from post-create.sh (foreground, once at container creation).
# Requires root for `ipset test` — sudoers entry granted via Dockerfile.
# NOTE: connectivity tests are informational — a ❌ result highlights a real
# problem worth investigating, but should NOT fail post-create.sh (the actual
# security is enforced by ipset/iptables/mitmproxy, not by these probes). The
# script therefore exits 0 explicitly at the end ; failures are surfaced in red.
# Dropped `-e -E` + ERR trap : with backgrounded check_* jobs and `wait` the
# strict mode was triggering exit 1 even though no test explicitly failed.
set -uo pipefail
IFS=$'\n\t'

# ANSI for highlight — ❌ in bold red so real failures pop out visually.
RED=$'\033[1;31m'
RST=$'\033[0m'

# Same env-file contract as init-firewall.sh (sudo blocks env passthrough).
[ -f /tmp/.firewall-env ] && source /tmp/.firewall-env

DEBUG="${CLAUDE_CODE_FIREWALL_DEBUG:-false}"
dbg() { [ "$DEBUG" = "true" ] && echo "$@" || true; }

FIREWALL_CONFIG_DIR="${FIREWALL_CONFIG_DIR:-/etc/devcontainer-firewall}"
BLOCKED_TESTS="$FIREWALL_CONFIG_DIR/tests/blocked.txt"
PROBES_CACHE=/var/run/devcontainer-firewall/probes-cache.tsv

case "${FIREWALL_MODE:-strict}" in
  paranoid) FIREWALL_MODE=strict ;;
  okeish)   FIREWALL_MODE=basic  ;;
esac
FIREWALL_MODE="${FIREWALL_MODE:-strict}"

# off mode = firewall disabled, no tests to run.
if [ "$FIREWALL_MODE" = "off" ]; then
  echo "ℹ FIREWALL_MODE=off — skipping connectivity tests."
  exit 0
fi

if [ ! -f "$PROBES_CACHE" ]; then
  echo "${RED}❌ $PROBES_CACHE missing — run init-firewall.sh first.${RST}"
  exit 1
fi

# Resolve via Docker's internal resolver (bypassing dnsmasq) — used for
# CLAUDE_CODE_FIREWALL_ALLOWED hostnames.
resolve_via_docker() {
  local host="$1"
  { dig +short +time=3 +tries=1 @127.0.0.11 A "$host" 2>/dev/null \
      | grep -E '^([0-9]{1,3}\.){3}[0-9]{1,3}$' | head -1; } || true
}

trim() {
  local s="$1"
  s="${s%$'\r'}"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

HOST_IP=$(resolve_via_docker host.docker.internal || true)
[ -z "$HOST_IP" ] && HOST_IP=$(ip route | awk '/^default/ {print $3}')
FIREWALL_ALLOWED="${CLAUDE_CODE_FIREWALL_ALLOWED:-}"

TEST_RESULTS=$(mktemp)
trap "rm -f '$TEST_RESULTS'" EXIT

# Strict mode → curl probes go through mitm. Basic mode → direct.
PROXY_ARG=()
[ "$FIREWALL_MODE" = "strict" ] && PROXY_ARG=(-x http://127.0.0.1:8080)

tcp_probe() {
  timeout 3 bash -c "echo >/dev/tcp/$1/$2" 2>/dev/null
}

check_blocked() {
  local host="$1" label="$2"
  if curl "${PROXY_ARG[@]}" -sk -o /dev/null --max-time 3 \
       -w "%{http_code}" "https://$host/" 2>/dev/null \
       | grep -qE "^[1-5][0-9][0-9]$"; then
    echo "${RED}❌ $label (reached $host — firewall did NOT block)${RST}" >> "$TEST_RESULTS"
  else
    echo "✔ $label" >> "$TEST_RESULTS"
  fi
}

check_allowed() {
  local host="$1"
  local probe="${2:-$host}"
  local label="$host"
  [ "$probe" != "$host" ] && label="$host (via $probe)"
  local ips ip in_ipset=false ns

  ips=$({ dig +short +time=2 +tries=1 @127.0.0.53 "$probe" A 2>/dev/null \
          | grep -E '^([0-9]{1,3}\.){3}[0-9]{1,3}$'; } || true)
  if [ -z "$ips" ]; then
    ns=$({ dig +short +time=2 +tries=1 @127.0.0.53 "$probe" NS 2>/dev/null; } || true)
    if [ -n "$ns" ]; then
      echo "⚠️  $label (wildcard parent — no A on bare domain ; add probe in tests/probes.txt)" >> "$TEST_RESULTS"
    else
      echo "${RED}❌ $label (DNS resolution failed)${RST}" >> "$TEST_RESULTS"
    fi
    return
  fi
  for ip in $ips; do
    if ipset test allowed-domains "$ip" 2>/dev/null; then
      in_ipset=true
      break
    fi
  done

  if curl "${PROXY_ARG[@]}" -sk -o /dev/null --max-time 3 \
       -w "%{http_code}" "https://$probe/" 2>/dev/null \
       | grep -qE "^[1-5][0-9][0-9]$"; then
    echo "✔ $label reachable" >> "$TEST_RESULTS"
  elif $in_ipset; then
    # HTTPS probe failed but the IP is allowlisted — try a TCP-level fallback
    # for known non-HTTPS services so they still register as reachable. ollama
    # serves HTTP-only on :11434 ; the HTTPS-on-:443 probe would always fail
    # but the service is actually up. Generalize by adding more entries to the
    # case below when other non-HTTPS allowlisted services come in.
    local alt_port=""
    case "$probe" in
      ollama.internal|ollama.local) alt_port=11434 ;;
    esac
    if [ -n "$alt_port" ] && tcp_probe "$ip" "$alt_port"; then
      echo "✔ $label reachable (TCP :$alt_port)" >> "$TEST_RESULTS"
    else
      echo "⚠️  $label allowlisted in ipset but unreachable" >> "$TEST_RESULTS"
    fi
  else
    echo "${RED}❌ $label (no ipset match AND unreachable)${RST}" >> "$TEST_RESULTS"
  fi
}

echo "🔍 Running connectivity tests..."

# Bounded parallelism — launching 50+ curl probes through mitmproxy at once
# saturates the proxy (single-threaded request handling) and causes random
# 3-second timeouts on a few hosts each run, producing flaky "⚠ allowlisted
# but unreachable" warnings on hosts that ARE actually reachable. Cap at
# MAX_JOBS concurrent probes so mitmproxy keeps up. bash 4+ `wait -n`
# waits for any single background job to finish.
MAX_JOBS=8
running=0
_throttle() {
  if [ "$running" -ge "$MAX_JOBS" ]; then
    wait -n 2>/dev/null || true
    running=$((running - 1))
  fi
}

# Allowed: each (host, probe) pair from the cache built by init-firewall.sh
while IFS=$'\t' read -r host probe; do
  _throttle
  check_allowed "$host" "$probe" &
  running=$((running + 1))
done < "$PROBES_CACHE"

# Blocked: each host listed in tests/blocked.txt — must NOT be reachable
if [ -f "$BLOCKED_TESTS" ]; then
  while IFS= read -r line; do
    line="${line%%#*}"
    line=$(trim "$line")
    [ -z "$line" ] && continue
    _throttle
    check_blocked "$line" "$line blocked" &
    running=$((running + 1))
  done < "$BLOCKED_TESTS"
fi

# CLAUDE_CODE_FIREWALL_ALLOWED entries — TCP probe (may have no listener, OK)
if [ -n "$FIREWALL_ALLOWED" ]; then
  IFS=',' read -ra TEST_ENTRIES <<< "$FIREWALL_ALLOWED"
  for entry in "${TEST_ENTRIES[@]}"; do
    entry=$(echo "$entry" | tr -d ' ')
    host=$(echo "$entry" | cut -d: -f1)
    port=$(echo "$entry" | cut -d: -f2)
    if [ "$host" = "host" ]; then
      label="host ($HOST_IP):$port"
      ip="$HOST_IP"
    else
      ip=$(resolve_via_docker "$host")
      label="$host ($ip):$port"
    fi
    if [ -n "$ip" ]; then
      (
        if tcp_probe "$ip" "$port"; then
          echo "✔ $label reachable" >> "$TEST_RESULTS"
        else
          echo "⚠️  $label allowed but no service listening" >> "$TEST_RESULTS"
        fi
      ) &
    else
      echo "⚠️  $host not resolvable — skipped" >> "$TEST_RESULTS"
    fi
  done
fi
wait

cat "$TEST_RESULTS"
if grep -q "❌" "$TEST_RESULTS"; then
  echo "${RED}⚠️  Some tests failed (informational — see ❌ lines above).${RST}"
fi
exit 0

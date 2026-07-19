#!/usr/bin/env bash
# reload-basic-isolated.sh — E2E: basic-mode hot-reload + port-gate contract
#
# HOST-RUNNABLE ONLY. Spawns a scoped Docker container from the host, boots
# init-firewall.sh in basic mode (DNS + ipset only, no mitmproxy L7), drives
# direct curl through the firewall, triggers reload-local.sh, and asserts:
#   - baseline (allowlisted host) reaches upstream directly (no L7 proxy)
#   - non-allowlisted host is blocked (dnsmasq NXDOMAIN → curl 000)
#   - reload adds a new host in < 500 ms
#   - allowed-domains-base ipset preserved across reload (baseline stays)
#   - baseline still reachable post-reload
#   - portal-test:4242 direct TCP allowed (per-port ACCEPT works in basic too)
#   - portal-test:4241 blocked (RFC1918 REJECT catches non-seeded ports)
#   - host.docker.internal:HOST_PORT blocked (host isolation via RFC1918 REJECT)
#
# Setup also spawns a busybox portal-test sibling on the same network and a
# Node mini-server on host port 8765 for the host isolation check.
#
# Sibling of reload-strict-isolated.sh — same base + project image build
# pattern, same non-cached A→Z guarantee, different runtime assertions
# because basic has NO L7 layer (no method-policing, no host_not_in_policy
# 403 from an addon).
#
# NOT invoked from inside the running devcontainer — needs docker on host
# and node on host.
#
# Usage (workspace root):
#   bash .devcontainer/firewall/tests/reload-basic-isolated.sh
#
# Success: exit 0 (VALIDATION PASSED marker), "__END__" sentinel, 12 "✔"
# assertions. Any failed assertion increments a FAIL counter and the script
# exits 1 at the end — silence is not success.

set -uo pipefail

WORKSPACE_HOST_ROOT="${WORKSPACE_HOST_ROOT:-$(pwd)}"

FRESH_VERSION="e2e-$$-$(date +%s)"
FRESH_PROJECT="fw-basic-test"
BASE_TAG="claude-devcontainer-base:${FRESH_VERSION}-${FRESH_PROJECT}"
IMG_TAG="fw-reload-basic-test:$(date +%s)"
NET_NAME="fw-reload-basic-net-$$"
CONTAINER="firewall-reload-basic-$$"
PORTAL="portal-test-basic-$$"
# Two host mini-servers: 8765 stays unauthorized (must be blocked), 8766
# is seeded into direct-tcp-allow.txt as `host:8766` (must be reachable).
# Proves both directions of the host-port opt-in: default-block + explicit-allow.
HOST_PORT_BLOCKED="8765"
HOST_PORT_ALLOWED="8766"
HOST_SRV_PID_BLOCKED=""
HOST_SRV_PID_ALLOWED=""

DOMAINS_LOCAL_HOST="$WORKSPACE_HOST_ROOT/.devcontainer/firewall/domains.local.txt"
DOMAINS_LOCAL_SNAPSHOT=""

FAIL=0
fail() { echo "  ❌ $*"; FAIL=$((FAIL + 1)); }

if [ ! -d "$WORKSPACE_HOST_ROOT/.devcontainer" ]; then
  echo "❌ FATAL: no .devcontainer/ at $WORKSPACE_HOST_ROOT" >&2
  exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
  echo "❌ FATAL: docker CLI not found" >&2
  exit 1
fi
if ! command -v node >/dev/null 2>&1; then
  echo "❌ FATAL: node not found on host — needed for host.docker.internal mini-server" >&2
  exit 1
fi
if [ ! -f "$WORKSPACE_HOST_ROOT/.devcontainer/Dockerfile.base" ]; then
  echo "❌ FATAL: .devcontainer/Dockerfile.base missing — cannot A→Z build" >&2
  exit 1
fi
if [ ! -f "$WORKSPACE_HOST_ROOT/templates/v2/Dockerfile" ]; then
  echo "❌ FATAL: templates/v2/Dockerfile missing — cannot build project layer" >&2
  exit 1
fi

cleanup() {
  echo
  echo "═══ Cleanup ═══"
  [ -n "$HOST_SRV_PID_BLOCKED" ] && kill "$HOST_SRV_PID_BLOCKED" 2>/dev/null || true
  [ -n "$HOST_SRV_PID_ALLOWED" ] && kill "$HOST_SRV_PID_ALLOWED" 2>/dev/null || true
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  docker rm -f "$PORTAL"    >/dev/null 2>&1 || true
  docker network rm "$NET_NAME" >/dev/null 2>&1 || true
  docker rmi "$IMG_TAG"  >/dev/null 2>&1 || true
  docker rmi "$BASE_TAG" >/dev/null 2>&1 || true
  if [ -n "$DOMAINS_LOCAL_SNAPSHOT" ] && [ -f "$DOMAINS_LOCAL_SNAPSHOT" ]; then
    cp -a "$DOMAINS_LOCAL_SNAPSHOT" "$DOMAINS_LOCAL_HOST" 2>/dev/null || true
    rm -f "$DOMAINS_LOCAL_SNAPSHOT"
  fi
  echo "  ✔ host mini-server + containers + network + images removed, domains.local.txt restored"
}
trap 'cleanup; echo "__END__"' EXIT

if [ -f "$DOMAINS_LOCAL_HOST" ]; then
  DOMAINS_LOCAL_SNAPSHOT="$(mktemp -t domains.local.txt.snap.XXXXXX)"
  cp -a "$DOMAINS_LOCAL_HOST" "$DOMAINS_LOCAL_SNAPSHOT"
  echo "  ✔ domains.local.txt snapshot: $DOMAINS_LOCAL_SNAPSHOT"
fi

echo "═══ [Setup 1/6] Build dedicated non-cached base image ═══"
echo "  base tag: $BASE_TAG  (--no-cache, ~5-10 min on cold apt)"
docker build --no-cache --progress=plain \
  -t "$BASE_TAG" \
  -f "$WORKSPACE_HOST_ROOT/.devcontainer/Dockerfile.base" \
  --build-arg CLAUDE_CODE_VERSION="$FRESH_VERSION" \
  --build-arg TZ="${TZ:-UTC}" \
  "$WORKSPACE_HOST_ROOT/.devcontainer/" 2>&1 | tail -25 | sed 's/^/    /'
if ! docker image inspect "$BASE_TAG" >/dev/null 2>&1; then
  echo "  ❌ dedicated base build FAILED — see output above" >&2
  exit 1
fi
echo "  ✔ dedicated base built: $BASE_TAG"

echo
echo "═══ [Setup 2/6] Build project layer (non-cached) ═══"
docker build --no-cache --progress=plain \
  -t "$IMG_TAG" \
  -f "$WORKSPACE_HOST_ROOT/templates/v2/Dockerfile" \
  --build-arg CLAUDE_CODE_VERSION="$FRESH_VERSION" \
  --build-arg DC_PROJECT="$FRESH_PROJECT" \
  "$WORKSPACE_HOST_ROOT/.devcontainer/" 2>&1 | tail -20 | sed 's/^/    /'
if ! docker image inspect "$IMG_TAG" >/dev/null 2>&1; then
  echo "  ❌ project layer build FAILED — see output above" >&2
  exit 1
fi
echo "  ✔ project layer built: $IMG_TAG"

echo
echo "═══ [Setup 3/6] Create isolated Docker network ═══"
docker network create "$NET_NAME" >/dev/null
echo "  ✔ network created: $NET_NAME"

echo
echo "═══ [Setup 4/6] Start portal-test sibling (busybox httpd :4242 + :4241) ═══"
docker run -d --name "$PORTAL" \
  --network "$NET_NAME" \
  --network-alias portal-test \
  --entrypoint sh \
  busybox -c '
    mkdir -p /w4242 /w4241
    echo "portal 4242 OK" > /w4242/index.html
    echo "portal 4241 OK" > /w4241/index.html
    httpd -f -p 0.0.0.0:4242 -h /w4242 &
    httpd -f -p 0.0.0.0:4241 -h /w4241 &
    wait
  ' >/dev/null
sleep 0.5
if docker ps -q -f "name=$PORTAL" | grep -q .; then
  echo "  ✔ portal-test sibling running (aliases: portal-test:4242 + portal-test:4241)"
else
  echo "  ❌ portal-test sibling failed to start" >&2
  exit 1
fi

echo
echo "═══ [Setup 5/6] Spawn TWO host mini-servers (Node) — :$HOST_PORT_BLOCKED + :$HOST_PORT_ALLOWED ═══"
node -e "require('http').createServer((_,res)=>res.end('host mini-server $HOST_PORT_BLOCKED (unauth)\n')).listen($HOST_PORT_BLOCKED,'0.0.0.0')" >/dev/null 2>&1 &
HOST_SRV_PID_BLOCKED=$!
node -e "require('http').createServer((_,res)=>res.end('host mini-server $HOST_PORT_ALLOWED (allowed)\n')).listen($HOST_PORT_ALLOWED,'0.0.0.0')" >/dev/null 2>&1 &
HOST_SRV_PID_ALLOWED=$!
sleep 0.5
if kill -0 "$HOST_SRV_PID_BLOCKED" 2>/dev/null && kill -0 "$HOST_SRV_PID_ALLOWED" 2>/dev/null; then
  echo "  ✔ host mini-servers up (:$HOST_PORT_BLOCKED PID=$HOST_SRV_PID_BLOCKED, :$HOST_PORT_ALLOWED PID=$HOST_SRV_PID_ALLOWED)"
else
  echo "  ❌ one or both host mini-servers failed to start — port $HOST_PORT_BLOCKED or $HOST_PORT_ALLOWED already bound?" >&2
  exit 1
fi

echo
echo "═══ [Setup 6/6] Start test container in basic mode ═══"
docker run -d \
  --name "$CONTAINER" \
  --network "$NET_NAME" \
  --privileged --cap-add=NET_ADMIN --cap-add=NET_RAW \
  --add-host=host.docker.internal:host-gateway \
  -v "$WORKSPACE_HOST_ROOT":/workspace \
  --entrypoint sleep \
  "$IMG_TAG" infinity >/dev/null
echo "  ✔ container running: $CONTAINER"

dex() { docker exec -u 0 "$CONTAINER" "$@"; }

# ══════════════════════════════════════════════════════════════════════════
# Test — basic-mode hot-reload contract + port-gate contract
# ══════════════════════════════════════════════════════════════════════════

echo
echo "═══ [1] Install scripts + force basic mode + seed portal-test:4242 ═══"
dex cp /workspace/.devcontainer/init-firewall.sh              /usr/local/bin/init-firewall.sh
dex cp /workspace/.devcontainer/firewall/compile-policy.py    /usr/local/bin/compile-policy.py
dex cp /workspace/.devcontainer/reload-local.sh               /usr/local/bin/reload-local.sh
dex chmod +x /usr/local/bin/init-firewall.sh /usr/local/bin/compile-policy.py /usr/local/bin/reload-local.sh
dex sh -c 'echo basic > /etc/devcontainer-firewall/default-mode'
# Seed portal-test:4242 AND host:$HOST_PORT_ALLOWED into the CONTAINER's
# direct-tcp-allow.txt (transient — container-scoped, no workspace
# mutation, no snapshot needed). Two entries prove BOTH ways of the
# port-gate: sibling container + host gateway via the `host` alias.
dex sh -c "printf 'portal-test:4242\nhost:$HOST_PORT_ALLOWED\n' >> /etc/devcontainer-firewall/direct-tcp-allow.txt"
# Same set -e / pgrep fragility workaround carried from the strict sibling.
dex sed -i 's#| head -1)$#| head -1 || true)#' /usr/local/bin/init-firewall.sh
echo "  ✔ basic mode + scripts installed, portal-test:4242 seeded"

echo
echo "═══ [2] Boot basic-mode firewall (init-firewall.sh) ═══"
INIT_OUT=$(dex /usr/local/bin/init-firewall.sh 2>&1)
INIT_RC=$?
echo "$INIT_OUT" | tail -25 | sed 's/^/    /'
if [ $INIT_RC -ne 0 ]; then
  echo "  ⚠ non-zero exit ($INIT_RC) — inspecting state"
fi
if echo "$INIT_OUT" | grep -q 'Firewall ready (basic'; then
  echo "  ✔ basic marker present in init-firewall output"
else
  fail "basic marker MISSING — init-firewall did not complete"
fi
sleep 1

# Curl helper — direct (no proxy), 3-char code normalization.
curl_direct() {
  local url="$1"
  local code
  code=$(dex curl -sS -m 15 -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
  echo "${code:0:3}"
}

echo
echo "═══ [3] Baseline curl direct — api.anthropic.com must PASS ═══"
CODE=$(curl_direct "https://api.anthropic.com/v1/models")
if [ "$CODE" = "200" ] || [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  if [ "$CODE" = "200" ]; then
    echo "  ✔ baseline HTTP=200"
  else
    echo "  ✔ baseline reached upstream (HTTP=$CODE, treated as OK — no L7 in basic)"
  fi
else
  fail "baseline HTTP=$CODE (expected 200/401/403 upstream response)"
fi

echo
echo "═══ [4] Pre-reload — example.org must be blocked (L3 DNS NXDOMAIN) ═══"
# In basic, DNS is the only gate for non-allowlisted hosts (no L7 addon).
# dnsmasq NXDOMAINs unknown hosts → curl fails at DNS resolution → 000.
CODE=$(curl_direct "https://example.org")
case "$CODE" in
  000)
    echo "  ✔ example.org HTTP=000 (L3 DNS blocked as expected)"
    ;;
  *)
    fail "example.org HTTP=$CODE (expected 000 from L3 DNS NXDOMAIN in basic)"
    ;;
esac

echo
echo "═══ [5] Append example.org to domains.local.txt + trigger reload ═══"
dex sh -c 'printf "\nexample.org\n" >> /workspace/.devcontainer/firewall/domains.local.txt'
echo "  ✔ domains.local.txt appended"

RELOAD_OUT=$(dex /usr/local/bin/reload-local.sh 2>&1)
RELOAD_RC=$?
echo "$RELOAD_OUT" | sed 's/^/    /'
if [ $RELOAD_RC -eq 0 ]; then
  echo "  ✔ reload-local.sh exited 0"
else
  fail "reload-local.sh exit=$RELOAD_RC"
fi

echo
echo "═══ [6] Elapsed < 500ms ═══"
ELAPSED=$(echo "$RELOAD_OUT" | grep -oE 'elapsed: [0-9]+ms' | grep -oE '[0-9]+' | head -1)
if [ -n "$ELAPSED" ] && [ "$ELAPSED" -lt 500 ]; then
  echo "  ✔ elapsed=${ELAPSED}ms (< 500ms budget)"
elif [ -n "$ELAPSED" ]; then
  fail "elapsed=${ELAPSED}ms (over 500ms budget)"
else
  fail "elapsed line not parseable from reload output"
fi

echo
echo "═══ [7] Post-reload — example.org must reach upstream (200) ═══"
CODE=$(curl_direct "https://example.org")
if [ "$CODE" = "200" ]; then
  echo "  ✔ example.org HTTP=200 (DNS + ipset cooperated on reload)"
else
  fail "example.org HTTP=$CODE (expected 200 after reload)"
fi

echo
echo "═══ [8] Baseline still reachable — api.anthropic.com must PASS ═══"
CODE=$(curl_direct "https://api.anthropic.com/v1/models")
if [ "$CODE" = "200" ] || [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  echo "  ✔ baseline still reachable (HTTP=$CODE) — allowed-domains-base preserved"
else
  fail "baseline regressed after reload (HTTP=$CODE) — allowed-domains-base flushed?"
fi

# ── Port-gate + host isolation contract ────────────────────────────────────

echo
echo "═══ [9] Port-gate — portal-test:4242 must PASS (per-port ACCEPT) ═══"
CODE=$(curl_direct "http://portal-test:4242/")
if [ "$CODE" = "200" ]; then
  echo "  ✔ portal-test:4242 HTTP=200 (direct-tcp-allow ACCEPT before RFC1918 REJECT)"
else
  fail "portal-test:4242 HTTP=$CODE (expected 200 from direct-tcp-allow seed)"
fi

echo
echo "═══ [10] Port-gate — portal-test:4241 must be BLOCKED (RFC1918 REJECT) ═══"
CODE=$(curl_direct "http://portal-test:4241/")
case "$CODE" in
  000)
    echo "  ✔ portal-test:4241 HTTP=000 (RFC1918 REJECT catches non-seeded ports)"
    ;;
  *)
    fail "portal-test:4241 HTTP=$CODE (expected 000 — RFC1918 REJECT should catch)"
    ;;
esac

echo
echo "═══ [11] Host isolation — host.docker.internal:$HOST_PORT_BLOCKED must be BLOCKED ═══"
# Node mini-server on host binds 0.0.0.0:$HOST_PORT_BLOCKED — reachable via
# host.docker.internal from the container. Since host:$HOST_PORT_BLOCKED is
# NOT in direct-tcp-allow.txt, RFC1918 REJECT fires (host.docker.internal IP
# is in a private range).
CODE=$(curl_direct "http://host.docker.internal:$HOST_PORT_BLOCKED/")
case "$CODE" in
  000)
    echo "  ✔ host.docker.internal:$HOST_PORT_BLOCKED HTTP=000 (RFC1918 REJECT — host isolated)"
    ;;
  200)
    fail "host.docker.internal:$HOST_PORT_BLOCKED HTTP=200 — host mini-server REACHED, isolation broken!"
    ;;
  *)
    fail "host.docker.internal:$HOST_PORT_BLOCKED HTTP=$CODE (expected 000 — RFC1918 REJECT should catch)"
    ;;
esac

echo
echo "═══ [12] Host opt-in — host.docker.internal:$HOST_PORT_ALLOWED must PASS (host:$HOST_PORT_ALLOWED seed) ═══"
# Same setup as [11] but this port IS seeded into direct-tcp-allow.txt as
# `host:$HOST_PORT_ALLOWED`. init-firewall's L534-560 loop resolves the
# `host` keyword to host.docker.internal's IP and emits a per-port ACCEPT
# rule that fires BEFORE the RFC1918 REJECT — so this specific port
# reaches the host mini-server.
CODE=$(curl_direct "http://host.docker.internal:$HOST_PORT_ALLOWED/")
if [ "$CODE" = "200" ]; then
  echo "  ✔ host.docker.internal:$HOST_PORT_ALLOWED HTTP=200 (per-port ACCEPT for `host:` alias works)"
else
  fail "host.docker.internal:$HOST_PORT_ALLOWED HTTP=$CODE (expected 200 — direct-tcp-allow host: seed didn't punch through)"
fi

echo
echo "═══ FULL VALIDATION COMPLETE ═══"
if [ "$FAIL" -gt 0 ]; then
  echo "❌ VALIDATION FAILED — $FAIL assertion(s) failed"
  exit 1
fi
echo "✔ VALIDATION PASSED"
exit 0

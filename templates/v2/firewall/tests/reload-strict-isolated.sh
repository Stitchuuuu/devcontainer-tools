#!/usr/bin/env bash
# reload-strict-isolated.sh — E2E: strict-mode hot-reload + port-gate contract
#
# HOST-RUNNABLE ONLY. Spawns a scoped Docker container from the host, runs
# init-firewall.sh in strict, drives curl through the live mitmproxy, triggers
# reload-local.sh, and asserts the zero-downtime contract PLUS the sibling
# port-gate + host.docker.internal isolation contract.
#
# Setup builds a DEDICATED, non-cached base + project image pair per run to
# test A→Z from Dockerfile.base + templates/v2/Dockerfile. Also spawns a
# busybox portal-test sibling (httpd on :4242 + :4241) on the same docker
# network, and a Node mini-server on the host port 8765 (via host.docker.
# internal). Does not touch the developer's live claude-devcontainer-base
# tag. Cleaned up on exit.
#
# NOT invoked from inside the running devcontainer — needs docker on host
# and node on host.
#
# Usage (workspace root):
#   bash .devcontainer/firewall/tests/reload-strict-isolated.sh
#
# Success: exit 0 (VALIDATION PASSED marker), "__END__" sentinel, 14 "✔"
# assertions. Any failed assertion increments a FAIL counter and the script
# exits 1 at the end — silence is not success.

set -uo pipefail

WORKSPACE_HOST_ROOT="${WORKSPACE_HOST_ROOT:-$(pwd)}"

FRESH_VERSION="e2e-$$-$(date +%s)"
FRESH_PROJECT="fw-strict-test"
BASE_TAG="claude-devcontainer-base:${FRESH_VERSION}-${FRESH_PROJECT}"
IMG_TAG="fw-reload-strict-test:$(date +%s)"
NET_NAME="fw-reload-strict-net-$$"
CONTAINER="firewall-reload-strict-$$"
PORTAL="portal-test-$$"
HOST_PORT="8765"
HOST_SRV_PID=""

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
  [ -n "$HOST_SRV_PID" ] && kill "$HOST_SRV_PID" 2>/dev/null || true
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
echo "═══ [Setup 5/6] Spawn host mini-server on 0.0.0.0:$HOST_PORT (Node) ═══"
node -e "require('http').createServer((_,res)=>res.end('host mini-server\n')).listen($HOST_PORT,'0.0.0.0',()=>console.error('listening'))" >/dev/null 2>&1 &
HOST_SRV_PID=$!
sleep 0.5
if kill -0 "$HOST_SRV_PID" 2>/dev/null; then
  echo "  ✔ host mini-server up (PID=$HOST_SRV_PID) — reachable via host.docker.internal:$HOST_PORT"
else
  echo "  ❌ host mini-server failed to start (PID=$HOST_SRV_PID died — port $HOST_PORT already bound?)" >&2
  exit 1
fi

echo
echo "═══ [Setup 6/6] Start test container in strict mode ═══"
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
# Test — strict-mode hot-reload contract + port-gate contract
# ══════════════════════════════════════════════════════════════════════════

echo
echo "═══ [1] Install scripts + force strict mode + seed portal-test:4242 ═══"
dex cp /workspace/.devcontainer/init-firewall.sh              /usr/local/bin/init-firewall.sh
dex cp /workspace/.devcontainer/firewall/compile-policy.py    /usr/local/bin/compile-policy.py
dex cp /workspace/.devcontainer/reload-local.sh               /usr/local/bin/reload-local.sh
dex chmod +x /usr/local/bin/init-firewall.sh /usr/local/bin/compile-policy.py /usr/local/bin/reload-local.sh
dex sh -c 'echo strict > /etc/devcontainer-firewall/default-mode'
# Seed portal-test:4242 in the CONTAINER's direct-tcp-allow.txt (transient —
# container-scoped, no workspace mutation, no snapshot needed).
dex sh -c 'echo portal-test:4242 >> /etc/devcontainer-firewall/direct-tcp-allow.txt'
# Precedent workaround for a `set -e` fragility in init-firewall's pgrep
# pipeline when nothing matches. Carried verbatim.
dex sed -i 's#| head -1)$#| head -1 || true)#' /usr/local/bin/init-firewall.sh
echo "  ✔ strict mode + scripts installed, portal-test:4242 seeded"

echo
echo "═══ [2] Boot strict-mode firewall (init-firewall.sh) ═══"
INIT_OUT=$(dex /usr/local/bin/init-firewall.sh 2>&1)
INIT_RC=$?
echo "$INIT_OUT" | tail -25 | sed 's/^/    /'
if [ $INIT_RC -ne 0 ]; then
  echo "  ⚠ non-zero exit ($INIT_RC) — inspecting state"
fi
if echo "$INIT_OUT" | grep -q 'Firewall ready (strict'; then
  echo "  ✔ strict marker present in init-firewall output"
else
  fail "strict marker MISSING — init-firewall did not complete"
fi
sleep 1

echo
echo "═══ [3] Capture mitmdump PID (baseline) ═══"
PID_BEFORE=$(dex pgrep -x mitmdump | head -1 | tr -d '[:space:]')
if [ -n "$PID_BEFORE" ]; then
  echo "  ✔ mitmdump PID=$PID_BEFORE"
else
  fail "mitmdump not running — strict boot failed"
fi

curl_code() {
  local method="$1"; shift
  local url="$1"; shift
  local code
  code=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
    -w "%{http_code}" -X "$method" "$url" 2>/dev/null || echo "000")
  if [ "$code" = "000" ]; then
    sleep 2
    code=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
      -w "%{http_code}" -X "$method" "$url" 2>/dev/null || echo "000")
  fi
  echo "${code:0:3}"
}

# Direct curl (no proxy) — 3-char code normalization. For port-gate tests
# where mitmproxy is not in the path (direct-tcp-allow bypasses L7).
curl_direct() {
  local url="$1"
  local code
  code=$(dex curl -sS -m 10 -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
  echo "${code:0:3}"
}

echo
echo "═══ [4] Baseline curl through mitmproxy — api.anthropic.com must PASS ═══"
CODE=$(curl_code GET "https://api.anthropic.com/v1/models")
if [ "$CODE" = "200" ] || [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  if [ "$CODE" = "200" ]; then
    echo "  ✔ baseline HTTP=200"
  else
    echo "  ✔ baseline reached upstream (HTTP=$CODE, treated as OK)"
  fi
else
  fail "baseline HTTP=$CODE (expected 200/401/403 upstream response)"
fi

echo
echo "═══ [5] Pre-reload — example.org must be blocked (L3 DNS or L7 403) ═══"
# In strict, dnsmasq NXDOMAINs non-allowlisted hosts, so mitmproxy fails
# upstream resolution → curl reports "000" (L3 block) before the addon's
# host_not_in_policy path can fire. If mitmproxy's connection_strategy is
# lazy on a given platform, the L7 addon returns 403 instead.
CODE=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
  -w "%{http_code}" -X GET https://example.org 2>/dev/null || echo "000")
CODE="${CODE:0:3}"
case "$CODE" in
  403)
    echo "  ✔ example.org HTTP=403 (L7 addon rejected)"
    ;;
  000|502|504)
    echo "  ✔ example.org HTTP=$CODE (L3 DNS/gateway blocked)"
    ;;
  *)
    fail "example.org HTTP=$CODE (expected 000 L3, 403 L7, or 502/504 gateway)"
    ;;
esac

echo
echo "═══ [6] Append example.org to domains.local.txt + trigger reload ═══"
dex sh -c 'printf "\n[GET,HEAD] example.org\n" >> /workspace/.devcontainer/firewall/domains.local.txt'
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
echo "═══ [7] Elapsed < 500ms ═══"
ELAPSED=$(echo "$RELOAD_OUT" | grep -oE 'elapsed: [0-9]+ms' | grep -oE '[0-9]+' | head -1)
if [ -n "$ELAPSED" ] && [ "$ELAPSED" -lt 500 ]; then
  echo "  ✔ elapsed=${ELAPSED}ms (< 500ms budget)"
elif [ -n "$ELAPSED" ]; then
  fail "elapsed=${ELAPSED}ms (over 500ms budget)"
else
  fail "elapsed line not parseable from reload output"
fi

echo
echo "═══ [8] Post-reload — example.org GET must PASS (200) ═══"
CODE=$(curl_code GET "https://example.org")
if [ "$CODE" = "200" ]; then
  echo "  ✔ example.org GET HTTP=200 (DNS + ipset + addon mtime-reload cooperated)"
else
  fail "example.org GET HTTP=$CODE (expected 200 after reload)"
fi

echo
echo "═══ [9] Post-reload strict L7 — example.org POST must 403 (method) ═══"
CODE=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
  -w "%{http_code}" -X POST https://example.org 2>/dev/null || echo "000")
CODE="${CODE:0:3}"
if [ "$CODE" = "403" ]; then
  echo "  ✔ example.org POST HTTP=403 (strict L7 method-policing fired)"
else
  fail "example.org POST HTTP=$CODE (expected 403 method:POST)"
fi

echo
echo "═══ [10] Baseline still reachable — api.anthropic.com must PASS ═══"
CODE=$(curl_code GET "https://api.anthropic.com/v1/models")
if [ "$CODE" = "200" ] || [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  echo "  ✔ baseline still reachable (HTTP=$CODE) — no regression"
else
  fail "baseline regressed after reload (HTTP=$CODE)"
fi

echo
echo "═══ [11] Zero-downtime — mitmdump PID unchanged ═══"
PID_AFTER=$(dex pgrep -x mitmdump | head -1 | tr -d '[:space:]')
if [ -n "$PID_AFTER" ] && [ "$PID_BEFORE" = "$PID_AFTER" ]; then
  echo "  ✔ mitmdump PID=$PID_AFTER (unchanged — addon mtime-reload in-place)"
else
  fail "mitmdump PID_BEFORE=$PID_BEFORE PID_AFTER=$PID_AFTER (process restarted)"
fi

# ── Port-gate + host isolation contract ────────────────────────────────────

echo
echo "═══ [12] Port-gate — portal-test:4242 must PASS (per-port ACCEPT) ═══"
CODE=$(curl_direct "http://portal-test:4242/")
if [ "$CODE" = "200" ]; then
  echo "  ✔ portal-test:4242 HTTP=200 (direct-tcp-allow ACCEPT before RFC1918 REJECT)"
else
  fail "portal-test:4242 HTTP=$CODE (expected 200 from direct-tcp-allow seed)"
fi

echo
echo "═══ [13] Port-gate — portal-test:4241 must be BLOCKED (RFC1918 REJECT) ═══"
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
echo "═══ [14] Host isolation — host.docker.internal:$HOST_PORT must be BLOCKED ═══"
# Node mini-server on host binds 0.0.0.0:$HOST_PORT — reachable via
# host.docker.internal from the container. Since host:$HOST_PORT is NOT in
# direct-tcp-allow.txt, RFC1918 REJECT fires (host.docker.internal IP is
# in a private range). The ipset ACCEPT for allowed-domains never gets
# to trigger (would come after the REJECT anyway).
CODE=$(curl_direct "http://host.docker.internal:$HOST_PORT/")
case "$CODE" in
  000)
    echo "  ✔ host.docker.internal:$HOST_PORT HTTP=000 (RFC1918 REJECT — host isolated)"
    ;;
  200)
    fail "host.docker.internal:$HOST_PORT HTTP=200 — host mini-server REACHED, isolation broken!"
    ;;
  *)
    fail "host.docker.internal:$HOST_PORT HTTP=$CODE (expected 000 — RFC1918 REJECT should catch)"
    ;;
esac

echo
echo "═══ FULL VALIDATION COMPLETE ═══"
if [ "$FAIL" -gt 0 ]; then
  echo "❌ VALIDATION FAILED — $FAIL assertion(s) failed"
  exit 1
fi
echo "✔ VALIDATION PASSED"
exit 0

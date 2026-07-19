#!/usr/bin/env bash
# reload-strict-isolated.sh — E2E: strict-mode hot-reload preserves mitmdump
#
# HOST-RUNNABLE ONLY. Spawns a scoped Docker container from the host (docker
# CLI on the outside), runs init-firewall.sh in strict, drives curl through
# the live mitmproxy, triggers reload-local.sh, and asserts the zero-downtime
# contract: same mitmdump PID before/after, sub-500ms reload, no baseline
# regression, and L7 method-policing still fires on the newly added host.
#
# NOT invoked from inside the running devcontainer — it needs docker to spawn
# a peer container next to the caller.
#
# Builds a DEDICATED, non-cached base + project image pair per run to test
# A→Z from Dockerfile.base + templates/v2/Dockerfile. Does not touch the
# developer's live claude-devcontainer-base tag. Cleaned up on exit.
#
# Usage (workspace root):
#   bash .devcontainer/firewall/tests/reload-strict-isolated.sh
#
# Success: exit 0 (VALIDATION PASSED marker), "__END__" sentinel, 11 "✔"
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
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  docker network rm "$NET_NAME" >/dev/null 2>&1 || true
  docker rmi "$IMG_TAG"  >/dev/null 2>&1 || true
  docker rmi "$BASE_TAG" >/dev/null 2>&1 || true
  if [ -n "$DOMAINS_LOCAL_SNAPSHOT" ] && [ -f "$DOMAINS_LOCAL_SNAPSHOT" ]; then
    cp -a "$DOMAINS_LOCAL_SNAPSHOT" "$DOMAINS_LOCAL_HOST" 2>/dev/null || true
    rm -f "$DOMAINS_LOCAL_SNAPSHOT"
  fi
  echo "  ✔ container + network + images removed, domains.local.txt restored"
}
trap 'cleanup; echo "__END__"' EXIT

if [ -f "$DOMAINS_LOCAL_HOST" ]; then
  DOMAINS_LOCAL_SNAPSHOT="$(mktemp -t domains.local.txt.snap.XXXXXX)"
  cp -a "$DOMAINS_LOCAL_HOST" "$DOMAINS_LOCAL_SNAPSHOT"
  echo "  ✔ domains.local.txt snapshot: $DOMAINS_LOCAL_SNAPSHOT"
fi

echo "═══ [Setup 1/4] Build dedicated non-cached base image ═══"
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
echo "═══ [Setup 2/4] Build project layer (non-cached) ═══"
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
echo "═══ [Setup 3/4] Create isolated Docker network ═══"
docker network create "$NET_NAME" >/dev/null
echo "  ✔ network created: $NET_NAME"

echo
echo "═══ [Setup 4/4] Start container in strict mode ═══"
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
# Test — strict-mode hot-reload contract
# ══════════════════════════════════════════════════════════════════════════

echo
echo "═══ [1] Install scripts + force strict mode ═══"
dex cp /workspace/.devcontainer/init-firewall.sh              /usr/local/bin/init-firewall.sh
dex cp /workspace/.devcontainer/firewall/compile-policy.py    /usr/local/bin/compile-policy.py
dex cp /workspace/.devcontainer/reload-local.sh               /usr/local/bin/reload-local.sh
dex chmod +x /usr/local/bin/init-firewall.sh /usr/local/bin/compile-policy.py /usr/local/bin/reload-local.sh
dex sh -c 'echo strict > /etc/devcontainer-firewall/default-mode'
# Precedent workaround for a `set -e` fragility in init-firewall's pgrep
# pipeline when nothing matches. Carried verbatim.
dex sed -i 's#| head -1)$#| head -1 || true)#' /usr/local/bin/init-firewall.sh
echo "  ✔ strict mode + scripts installed"

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
  echo "$code"
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
# lazy on a given platform, the L7 addon returns 403 instead. Either
# outcome proves the pre-reload blocked state.
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

echo
echo "═══ FULL VALIDATION COMPLETE ═══"
if [ "$FAIL" -gt 0 ]; then
  echo "❌ VALIDATION FAILED — $FAIL assertion(s) failed"
  exit 1
fi
echo "✔ VALIDATION PASSED"
exit 0

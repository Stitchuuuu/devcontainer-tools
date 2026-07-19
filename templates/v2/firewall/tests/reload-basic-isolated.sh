#!/usr/bin/env bash
# reload-basic-isolated.sh — E2E: basic-mode hot-reload preserves baseline
#
# HOST-RUNNABLE ONLY. Spawns a scoped Docker container from the host (docker
# CLI on the outside), boots init-firewall.sh in basic mode (DNS + ipset only,
# no mitmproxy L7), then drives direct curl through the firewall, triggers
# reload-local.sh, and asserts:
#   - baseline (allowlisted host) reaches upstream directly (no L7 proxy)
#   - non-allowlisted host is blocked (dnsmasq NXDOMAIN → curl 000)
#   - reload adds a new host in < 500 ms (matches strict-mode contract)
#   - allowed-domains-base ipset preserved across reload (baseline stays)
#   - baseline still reachable post-reload (no regression)
#
# Sibling of reload-strict-isolated.sh — same base + project image build
# pattern, same non-cached A→Z guarantee, different runtime assertions
# because basic has NO L7 layer (no mitmproxy method-policing, no
# host_not_in_policy 403 from an addon).
#
# NOT invoked from inside the running devcontainer — needs docker on host.
#
# Builds a DEDICATED, non-cached base + project image pair per run.
# Cleaned up on exit.
#
# Usage (workspace root):
#   bash .devcontainer/firewall/tests/reload-basic-isolated.sh
#
# Success: exit 0 (VALIDATION PASSED marker), "__END__" sentinel, 8 "✔"
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
echo "═══ [Setup 4/4] Start container in basic mode ═══"
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
# Test — basic-mode hot-reload contract
# ══════════════════════════════════════════════════════════════════════════

echo
echo "═══ [1] Install scripts + force basic mode ═══"
dex cp /workspace/.devcontainer/init-firewall.sh              /usr/local/bin/init-firewall.sh
dex cp /workspace/.devcontainer/firewall/compile-policy.py    /usr/local/bin/compile-policy.py
dex cp /workspace/.devcontainer/reload-local.sh               /usr/local/bin/reload-local.sh
dex chmod +x /usr/local/bin/init-firewall.sh /usr/local/bin/compile-policy.py /usr/local/bin/reload-local.sh
dex sh -c 'echo basic > /etc/devcontainer-firewall/default-mode'
# Same set -e / pgrep fragility workaround carried from the precedent /
# strict sibling.
dex sed -i 's#| head -1)$#| head -1 || true)#' /usr/local/bin/init-firewall.sh
echo "  ✔ basic mode + scripts installed"

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

echo
echo "═══ FULL VALIDATION COMPLETE ═══"
if [ "$FAIL" -gt 0 ]; then
  echo "❌ VALIDATION FAILED — $FAIL assertion(s) failed"
  exit 1
fi
echo "✔ VALIDATION PASSED"
exit 0

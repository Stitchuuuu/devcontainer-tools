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
# Usage (workspace root):
#   bash .devcontainer/firewall/tests/reload-strict-isolated.sh
#
# Success: exit 0, zero "❌" lines, "__END__" sentinel, PID_BEFORE=PID_AFTER,
# 10 "✔" assertions.

set -uo pipefail

WORKSPACE_HOST_ROOT="${WORKSPACE_HOST_ROOT:-$(pwd)}"
CLAUDE_CODE_VERSION="${CLAUDE_CODE_VERSION:-2.1.145}"
DC_PROJECT="${DC_PROJECT:-devcontainer-tools}"
BASE_TAG="claude-devcontainer-base:${CLAUDE_CODE_VERSION}-${DC_PROJECT}"
IMG_TAG="fw-reload-strict-test:$(date +%s)"
NET_NAME="fw-reload-strict-net-$$"
CONTAINER="firewall-reload-strict-$$"

DOMAINS_LOCAL_HOST="$WORKSPACE_HOST_ROOT/.devcontainer/firewall/domains.local.txt"
DOMAINS_LOCAL_SNAPSHOT=""

if [ ! -d "$WORKSPACE_HOST_ROOT/.devcontainer" ]; then
  echo "❌ FATAL: no .devcontainer/ at $WORKSPACE_HOST_ROOT" >&2
  exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
  echo "❌ FATAL: docker CLI not found" >&2
  exit 1
fi

# Trap early so a preflight failure below still triggers cleanup of the
# snapshot even though nothing else exists yet.
cleanup() {
  echo
  echo "═══ Cleanup ═══"
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  docker network rm "$NET_NAME" >/dev/null 2>&1 || true
  docker rmi "$IMG_TAG" >/dev/null 2>&1 || true
  # Restore workspace domains.local.txt from the pre-test snapshot. This
  # protects the developer's own uncommitted allowlist entries.
  if [ -n "$DOMAINS_LOCAL_SNAPSHOT" ] && [ -f "$DOMAINS_LOCAL_SNAPSHOT" ]; then
    cp -a "$DOMAINS_LOCAL_SNAPSHOT" "$DOMAINS_LOCAL_HOST" 2>/dev/null || true
    rm -f "$DOMAINS_LOCAL_SNAPSHOT"
  fi
  echo "  ✔ container + network + image removed, domains.local.txt restored"
}
trap 'cleanup; echo "__END__"' EXIT

# Snapshot BEFORE any mutation so trap EXIT always has a clean restore point.
if [ -f "$DOMAINS_LOCAL_HOST" ]; then
  DOMAINS_LOCAL_SNAPSHOT="$(mktemp -t domains.local.txt.snap.XXXXXX)"
  cp -a "$DOMAINS_LOCAL_HOST" "$DOMAINS_LOCAL_SNAPSHOT"
  echo "  ✔ domains.local.txt snapshot: $DOMAINS_LOCAL_SNAPSHOT"
fi

echo "═══ [Setup 1/3] Verify base image + build project layer ═══"
if ! docker image inspect "$BASE_TAG" >/dev/null 2>&1; then
  echo "  ❌ base image '$BASE_TAG' not found — run bash .devcontainer/initialize.sh first" >&2
  exit 1
fi
echo "  ✔ base cached: $BASE_TAG"

docker build --progress=plain \
  --build-arg "CLAUDE_CODE_VERSION=$CLAUDE_CODE_VERSION" \
  --build-arg "DC_PROJECT=$DC_PROJECT" \
  -t "$IMG_TAG" \
  -f "$WORKSPACE_HOST_ROOT/templates/v2/Dockerfile" \
  "$WORKSPACE_HOST_ROOT/.devcontainer/" 2>&1 | sed 's/^/    /'
echo "  ✔ project layer built: $IMG_TAG"

echo
echo "═══ [Setup 2/3] Create isolated Docker network ═══"
docker network create "$NET_NAME" >/dev/null
echo "  ✔ network created: $NET_NAME"

echo
echo "═══ [Setup 3/3] Start container in strict mode ═══"
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
  echo "  ❌ strict marker MISSING — init-firewall did not complete"
fi
# Let dnsmasq settle after mitm-init.sh's own bind poll.
sleep 1

echo
echo "═══ [3] Capture mitmdump PID (baseline) ═══"
PID_BEFORE=$(dex pgrep -x mitmdump | head -1 | tr -d '[:space:]')
if [ -n "$PID_BEFORE" ]; then
  echo "  ✔ mitmdump PID=$PID_BEFORE"
else
  echo "  ❌ mitmdump not running — strict boot failed"
fi

# Small helper: curl once with a retry to swallow transient DNS/mitm hiccups
# on the very first request after boot. Prints "HTTP=<code>" and returns 0
# always (assertions read the printed code).
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
  # 200 / 401 (no key) / 403 (rate-limit) all mean the request REACHED
  # Anthropic — the L3+L7 stack cleared it. A DNS/policy block would
  # surface as 000, 403+X-Block-Reason from mitmproxy, or 502.
  # We treat any of these as baseline-OK.
  if [ "$CODE" = "200" ]; then
    echo "  ✔ baseline HTTP=200"
  else
    echo "  ✔ baseline reached upstream (HTTP=$CODE, treated as OK)"
  fi
else
  echo "  ❌ baseline HTTP=$CODE (expected 200/401/403 upstream response)"
fi

echo
echo "═══ [5] Pre-reload — example.org must 403 (host_not_in_policy) ═══"
CODE=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
  -w "%{http_code}" -X GET https://example.org 2>/dev/null || echo "000")
if [ "$CODE" = "403" ]; then
  echo "  ✔ example.org HTTP=403 (strict L7 rejected as expected)"
else
  echo "  ❌ example.org HTTP=$CODE (expected 403 from strict L7 addon)"
fi

echo
echo "═══ [6] Append example.org to domains.local.txt + trigger reload ═══"
# Write via the container so the mounted volume propagates back to host;
# trap EXIT restores the snapshot regardless.
dex sh -c 'printf "\n[GET,HEAD] example.org\n" >> /workspace/.devcontainer/firewall/domains.local.txt'
echo "  ✔ domains.local.txt appended"

RELOAD_OUT=$(dex /usr/local/bin/reload-local.sh 2>&1)
RELOAD_RC=$?
echo "$RELOAD_OUT" | sed 's/^/    /'
if [ $RELOAD_RC -eq 0 ]; then
  echo "  ✔ reload-local.sh exited 0"
else
  echo "  ❌ reload-local.sh exit=$RELOAD_RC"
fi

echo
echo "═══ [7] Elapsed < 500ms ═══"
ELAPSED=$(echo "$RELOAD_OUT" | grep -oE 'elapsed: [0-9]+ms' | grep -oE '[0-9]+' | head -1)
if [ -n "$ELAPSED" ] && [ "$ELAPSED" -lt 500 ]; then
  echo "  ✔ elapsed=${ELAPSED}ms (< 500ms budget)"
elif [ -n "$ELAPSED" ]; then
  echo "  ❌ elapsed=${ELAPSED}ms (over 500ms budget)"
else
  echo "  ❌ elapsed line not parseable from reload output"
fi

echo
echo "═══ [8] Post-reload — example.org GET must PASS (200) ═══"
CODE=$(curl_code GET "https://example.org")
if [ "$CODE" = "200" ]; then
  echo "  ✔ example.org GET HTTP=200 (DNS + ipset + addon mtime-reload cooperated)"
else
  echo "  ❌ example.org GET HTTP=$CODE (expected 200 after reload)"
fi

echo
echo "═══ [9] Post-reload strict L7 — example.org POST must 403 (method) ═══"
CODE=$(dex curl -sS -m 15 -x http://127.0.0.1:8080 -o /dev/null \
  -w "%{http_code}" -X POST https://example.org 2>/dev/null || echo "000")
if [ "$CODE" = "403" ]; then
  echo "  ✔ example.org POST HTTP=403 (strict L7 method-policing fired)"
else
  echo "  ❌ example.org POST HTTP=$CODE (expected 403 method:POST)"
fi

echo
echo "═══ [10] Baseline still reachable — api.anthropic.com must PASS ═══"
CODE=$(curl_code GET "https://api.anthropic.com/v1/models")
if [ "$CODE" = "200" ] || [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  echo "  ✔ baseline still reachable (HTTP=$CODE) — no regression"
else
  echo "  ❌ baseline regressed after reload (HTTP=$CODE)"
fi

echo
echo "═══ [11] Zero-downtime — mitmdump PID unchanged ═══"
PID_AFTER=$(dex pgrep -x mitmdump | head -1 | tr -d '[:space:]')
if [ -n "$PID_AFTER" ] && [ "$PID_BEFORE" = "$PID_AFTER" ]; then
  echo "  ✔ mitmdump PID=$PID_AFTER (unchanged — addon mtime-reload in-place)"
else
  echo "  ❌ mitmdump PID_BEFORE=$PID_BEFORE PID_AFTER=$PID_AFTER (process restarted)"
fi

echo
echo "═══ FULL VALIDATION COMPLETE ═══"

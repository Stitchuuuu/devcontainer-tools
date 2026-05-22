#!/usr/bin/env bash
# firewall-mode.sh — flip firewall mode (host-side OR container-side).
#
# Usage :
#   ./firewall-mode.sh off          # disable firewall completely (kill-switch)
#   ./firewall-mode.sh basic        # DNS allowlist only (Phase 1)
#   ./firewall-mode.sh strict       # DNS + mitmproxy force-proxy (Phase 2 + A2 enforcement)
#
# Deprecated aliases (still accepted with a stderr WARN — A4 rename) :
#   okeish    → basic
#   paranoid  → strict
#
# What it does :
#   1. Updates `<repo>/.devcontainer/.configured-firewall-mode` (single source of truth)
#   2. Syncs `<repo>/.devcontainer/.env` proxy/CA vars — otherwise PID 1 keeps a
#      stale HTTPS_PROXY after rebuild (Docker re-reads env_file but doesn't
#      remove vars dropped from the file; we have to clear them explicitly).
#   3. Tells you to rebuild the container in VS Code to apply.
#
# No docker CLI required: this script only edits files. Works identically
# from the host or from inside the container.

set -euo pipefail

MODE="${1:-}"

# Translate deprecated aliases to canonical names.
case "$MODE" in
  okeish)
    echo "WARN: 'okeish' is a deprecated alias since A4 — use 'basic'." >&2
    MODE=basic ;;
  paranoid)
    echo "WARN: 'paranoid' is a deprecated alias since A4 — use 'strict'." >&2
    MODE=strict ;;
esac

case "$MODE" in
  off|basic|strict) ;;
  -h|--help|"")
    sed -n '2,24p' "$0"; exit 0 ;;
  *)
    echo "Invalid mode: $MODE (expected: off / basic / strict)" >&2
    exit 1 ;;
esac

# Flag file + .env live next to this script (always in .devcontainer/).
# $BASH_SOURCE resolves correctly on both host and container (the path is
# meaningful in both contexts because of the workspace bind-mount).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLAG_FILE="$SCRIPT_DIR/.configured-firewall-mode"
ENV_FILE="$SCRIPT_DIR/.env"

# Sync .env proxy/CA vars to match the new mode. Without this sync, after a
# rebuild: if the user flipped strict → off, PID 1 still inherits
# HTTPS_PROXY=http://127.0.0.1:8080 because Docker re-reads env_file but
# doesn't *remove* vars dropped from it — we have to clear them explicitly.
sync_env_var() {
  local key="$1" value="$2"
  touch "$ENV_FILE"
  if [ -s "$ENV_FILE" ] && [ "$(tail -c 1 "$ENV_FILE" | od -An -c | tr -d ' ')" != "\n" ]; then
    echo "" >> "$ENV_FILE"
  fi
  if grep -q "^${key}=" "$ENV_FILE"; then
    local tmp; tmp=$(mktemp)
    awk -v k="$key" -v v="$value" '
      $0 ~ "^"k"=" { print k"="v; next }
      { print }
    ' "$ENV_FILE" > "$tmp" && mv "$tmp" "$ENV_FILE"
  else
    echo "${key}=${value}" >> "$ENV_FILE"
  fi
}
unsync_env_var() {
  local key="$1"
  [ -f "$ENV_FILE" ] || return 0
  local tmp; tmp=$(mktemp)
  grep -v "^${key}=" "$ENV_FILE" > "$tmp" || true
  mv "$tmp" "$ENV_FILE"
}

if [ "$MODE" = "strict" ]; then
  sync_env_var "HTTPS_PROXY"          "http://127.0.0.1:8080"
  sync_env_var "HTTP_PROXY"           "http://127.0.0.1:8080"
  sync_env_var "NO_PROXY"             "localhost,127.0.0.0/8,host.docker.internal,.local"
  sync_env_var "NODE_EXTRA_CA_CERTS"  "/var/lib/mitmproxy/mitmproxy-ca-cert.pem"
else
  unsync_env_var "HTTPS_PROXY"
  unsync_env_var "HTTP_PROXY"
  unsync_env_var "NO_PROXY"
  unsync_env_var "NODE_EXTRA_CA_CERTS"
fi

PREVIOUS="$(cat "$FLAG_FILE" 2>/dev/null || echo '(unset, default strict)')"
echo "$MODE" > "$FLAG_FILE"

echo "Flag : $FLAG_FILE"
echo "       $PREVIOUS  ->  $MODE"
echo ".env : $ENV_FILE — proxy/CA vars $([ "$MODE" = "strict" ] && echo "set" || echo "cleared")"
echo
echo "Mode = $MODE written. To apply : rebuild the container."
echo "(new .env vars are only picked up at container re-creation, not on reload.)"
echo
case "$MODE" in
  off)
    echo "Direct internet, no filter. Re-enable with:"
    echo "    $0 strict    # or basic" ;;
  basic)
    echo "DNS allowlist only (Phase 1). No mitmproxy." ;;
  strict)
    echo "DNS + mitmproxy force-proxy + A2 addons enforcement (sécu max baseline)." ;;
esac

#!/bin/bash
# onCreateCommand — first lifecycle hook to run inside the container, BEFORE
# vscode-server installs the extensions listed in customizations.vscode.extensions.
# VS Code blocks on this hook, so by the time extensions start downloading the
# firewall is fully up (mitmproxy + iptables + DNS allowlist).
#
# Runs only once at container creation. postStartCommand re-runs init-firewall.sh
# at restarts if /tmp/.firewall-early-initialized is missing. Connectivity
# validation is now a separate concern handled by test-firewall.sh, invoked
# from post-create.sh after VS Code has kicked off the extension install.
set -eEu -o pipefail

# === Lifecycle logging === (mirror initialize.sh — see comment block there).
# Always-on : .log via tee. DEBUG=1 only : .trace (xtrace).
mkdir -p /workspace/.devcontainer/logs 2>/dev/null || true
TS=$(date +%Y%m%d-%H%M%S)
LOG=/workspace/.devcontainer/logs/on-create-${TS}.log
TRACE=/workspace/.devcontainer/logs/on-create-${TS}.trace
exec > >(tee -a "$LOG") 2>&1
if [ "${DEBUG:-0}" = "1" ]; then
  exec 19>>"$TRACE"
  export BASH_XTRACEFD=19
  PS4='+ ${BASH_SOURCE##*/}:${LINENO}: '
  set -x
  TRACE_LOC="${TRACE#/workspace/}"
else
  rm -f "$TRACE"
  TRACE_LOC="(disabled — set DEBUG=1 in .env to enable xtrace)"
fi
trap 'rc=$?; [ $rc -ne 0 ] && echo "✗ FAIL at ${BASH_SOURCE##*/}:${LINENO} (exit $rc): $BASH_COMMAND" >&2' ERR
echo "=== on-create $(date) ==="
echo "  log:   ${LOG#/workspace/}"
echo "  trace: $TRACE_LOC"

# Firewall — only debug toggle is still env-passed (informational, non-security).
# FIREWALL_MODE + CLAUDE_CODE_FIREWALL_ALLOWED are baked since session 1.
FW_MODE=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]')
FW_MODE="${FW_MODE:-strict}"
echo "CLAUDE_CODE_FIREWALL_DEBUG=${CLAUDE_CODE_FIREWALL_DEBUG:-}" > /tmp/.firewall-env

EARLY_FLAG=/tmp/.firewall-early-initialized
rm -f "$EARLY_FLAG"

# init-firewall.sh output flows through the global tee → $LOG (no longer a
# dedicated /tmp file). Dev Containers panel still sees progress live.
if sudo /usr/local/bin/init-firewall.sh 2>&1; then
  touch "$EARLY_FLAG"
  echo "✓ firewall up at onCreate (mode=$FW_MODE) — VS Code can DL extensions"
else
  echo "⚠ onCreate firewall init FAILED — see ${LOG#/workspace/} ; postStartCommand will retry"
fi

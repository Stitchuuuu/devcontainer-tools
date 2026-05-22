#!/bin/bash
# onCreateCommand — first lifecycle hook to run inside the container, BEFORE
# vscode-server installs the extensions listed in customizations.vscode.extensions.
# VS Code blocks on this hook, so by the time extensions start downloading the
# firewall is fully up (mitmproxy + iptables + DNS allowlist).
#
# Runs only once at container creation. postStartCommand re-invokes
# init-firewall.sh at every container start ; init-firewall.sh's own kernel-
# state guard decides whether to skip (firewall up) or re-init (netns wiped
# by restart). Connectivity validation is handled by test-firewall.sh,
# invoked from post-create.sh after VS Code kicks off the extension install.
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

FW_MODE=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]')
FW_MODE="${FW_MODE:-strict}"
FW_DEBUG_ARG=""
[ "${CLAUDE_CODE_FIREWALL_DEBUG:-}" = "true" ] && FW_DEBUG_ARG="--debug"

# init-firewall.sh output flows through the global tee → $LOG (no longer a
# dedicated /tmp file). Dev Containers panel still sees progress live.
if sudo /usr/local/bin/init-firewall.sh $FW_DEBUG_ARG 2>&1; then
  echo "✓ firewall up at onCreate (mode=$FW_MODE) — VS Code can DL extensions"
else
  echo "⚠ onCreate firewall init FAILED — see ${LOG#/workspace/} ; postStartCommand will retry"
fi

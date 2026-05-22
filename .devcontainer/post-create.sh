#!/bin/bash
# Post-create: symlink the correct CLAUDE.md based on chosen mode + run firewall tests
# set -eE : exit on first non-zero, propagate ERR trap into functions
set -eE

# === Lifecycle logging === (mirror initialize.sh — see comment block there).
# Always-on : .log via tee. DEBUG=1 only : .trace (xtrace).
mkdir -p /workspace/.devcontainer/logs 2>/dev/null || true
TS=$(date +%Y%m%d-%H%M%S)
LOG=/workspace/.devcontainer/logs/post-create-${TS}.log
TRACE=/workspace/.devcontainer/logs/post-create-${TS}.trace
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
echo "=== post-create $(date) ==="
echo "  log:   ${LOG#/workspace/}"
echo "  trace: $TRACE_LOC"

MODE_FLAG="/workspace/.devcontainer/.configured-claude-mode"
ENV_FILE="/workspace/.devcontainer/.env"

# Two orthogonal signals decide which CLAUDE.md to symlink :
#   1. claude-switch mode (set in .env via host-helper) — local / local-proxy
#      use CLAUDE-local-dev.md ; cloud falls through to signal 2.
#   2. .configured-claude-mode flag — picks the cloud variant (CLAUDE-dev.md
#      vs CLAUDE-reviewer.md).
# Without checking signal 1, every rebuild would clobber a claude-switch
# selection back to the cloud variant. claude-switch updates the symlink
# directly, but post-create runs after rebuild and re-writes blindly.
if grep -qE '^ANTHROPIC_BASE_URL=http://(ollama\.internal:11434|claude-bridge)' "$ENV_FILE" 2>/dev/null; then
	CLAUDE_FILE="CLAUDE-local-dev.md"
elif [ -f "$MODE_FLAG" ]; then
	CLAUDE_FILE=$(cat "$MODE_FLAG")
else
	CLAUDE_FILE="CLAUDE-dev.md"
fi

ln -sf ".devcontainer/claude/$CLAUDE_FILE" /workspace/CLAUDE.md
echo "✓ CLAUDE.md → $CLAUDE_FILE"

# Warn if test-root has sudo access (the real risk is the sudoers entry, not the file)
if sudo -l 2>/dev/null | grep -q "test-root"; then
	echo ""
	echo "⚠️  test-root.sh has sudo access! Remove the test RUN line in Dockerfile before committing."
fi

# Connectivity validation — runs after VS Code has already kicked off the
# extension install in parallel, so blocking here doesn't delay anything the
# user sees. ipset test inside test-firewall.sh requires root.
if [ -x /usr/local/bin/test-firewall.sh ]; then
	sudo /usr/local/bin/test-firewall.sh
fi

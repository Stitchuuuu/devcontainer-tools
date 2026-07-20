#!/usr/bin/env bash
# @name claude-creds-sync
# @phase post-start
# @required true
# @description Sync Claude .credentials.json between the shared volume (/home/node/.claude-creds) and per-container config (/home/node/.claude). Delegated to dedicated script (reused by shell-init + Claude hooks). Required because Claude Code depends on .credentials.json for auth; without it, every terminal fails.

set -eE

SYNC_CREDS="/workspace/.devcontainer/claude/sync-creds.sh"
if [ -x "$SYNC_CREDS" ]; then
  VERBOSE=1 "$SYNC_CREDS"
else
  echo "⚠️  $SYNC_CREDS missing — skipping credentials sync."
fi

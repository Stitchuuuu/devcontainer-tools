#!/usr/bin/env bash
# @name settings-backup
# @phase post-start
# @required false
# @description Timestamped copy of ~/.claude/settings.json into .devcontainer/claude/settings-backups/ before 75-skills-sync rewrites it (the v2 loader fork did this after each merge). A copy is taken only when the file differs from the newest backup.

set -eE

SETTINGS="${DEVC_CLAUDE_HOME:-/home/node/.claude}/settings.json"
BAK_DIR="${DEVC_CONFIG_DIR:-/workspace/.devcontainer}/claude/settings-backups"

[ -f "$SETTINGS" ] || exit 0
mkdir -p "$BAK_DIR"
last="$(ls -1 "$BAK_DIR"/settings.*.bak 2>/dev/null | sort | tail -1 || true)"
if [ -n "$last" ] && cmp -s "$SETTINGS" "$last"; then
  echo "- settings.json unchanged since ${last##*/}"
  exit 0
fi
dest="$BAK_DIR/settings.$(date +%Y%m%d-%H%M%S).bak"
cp "$SETTINGS" "$dest"
echo "✓ settings.json backed up to ${dest#/workspace/}"

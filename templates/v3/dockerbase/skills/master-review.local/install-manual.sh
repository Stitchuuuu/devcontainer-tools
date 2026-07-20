#!/usr/bin/env bash
# install-manual.sh — Standalone installer for the master-review skill.
#
# Use this script when you DON'T have devcontainer-tools available and want to
# install master-review by hand. If you do have devcontainer-tools, prefer:
#   bash <devcontainer-tools>/add-skill.sh master-review
#
# What this does (single idempotent code path):
#   1. Resolves the user's home (HOME or ~) and the skill folder this script
#      ships in.
#   2. Copies master-review.skill.md to $HOME/.claude/commands/master-review.md
#      (Claude Code reads commands from there at user scope).
#   3. Merges hooks.json into $HOME/.claude/settings.json under .hooks.Stop,
#      idempotently (basename match dedupe + atomic write + timestamped backup).
#   4. Cleans up legacy entries pointing at the old paths
#      (.devcontainer/hooks/{suggest-fresh-session,log-review-session}.sh)
#      so a migration from the pre-skill layout doesn't leave double-firing
#      hooks behind.
#
# Re-running the script is a no-op (final state matches first-run state).
#
# Requires: jq.
#
# Usage:
#   bash install-manual.sh
#   bash install-manual.sh --dry-run    # show what would change, write nothing
#   HOME=/tmp/fakehome bash install-manual.sh   # test mode

set -euo pipefail

DRY_RUN=false
case "${1:-}" in
	--dry-run) DRY_RUN=true ;;
	"") ;;
	*) echo "unknown arg: $1 (only --dry-run is supported)" >&2; exit 2 ;;
esac

# --- Locate self ---
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_MD="$SKILL_DIR/master-review.skill.md"
HOOKS_JSON="$SKILL_DIR/hooks.json"

if [ ! -f "$SKILL_MD" ] || [ ! -f "$HOOKS_JSON" ]; then
	echo "✗ Could not locate master-review.skill.md or hooks.json next to install-manual.sh" >&2
	echo "  (looked in $SKILL_DIR)" >&2
	exit 1
fi

# --- Verify jq ---
if ! command -v jq >/dev/null 2>&1; then
	echo "✗ jq is required but not installed." >&2
	echo "  Install: apt install jq  /  brew install jq  /  apk add jq" >&2
	exit 1
fi

# --- Resolve target ---
TARGET_HOME="${HOME:?HOME must be set}"
COMMANDS_DIR="$TARGET_HOME/.claude/commands"
SETTINGS="$TARGET_HOME/.claude/settings.json"
DEST_CMD="$COMMANDS_DIR/master-review.md"

echo "Installing master-review to:"
echo "  command : $DEST_CMD"
echo "  settings: $SETTINGS"
echo "  dry-run : $DRY_RUN"
echo

# --- 1. Copy skill.md → commands/<name>.md (strip .skill) ---
if [ "$DRY_RUN" = "false" ]; then
	mkdir -p "$COMMANDS_DIR"
	cp "$SKILL_MD" "$DEST_CMD"
fi
echo "✓ master-review.md → $DEST_CMD"

# --- 2. Merge hooks.json into settings.json ---
# 2a. Backup
if [ -f "$SETTINGS" ] && [ "$DRY_RUN" = "false" ]; then
	cp "$SETTINGS" "$SETTINGS.bak.$(date +%s)"
fi

# 2b. Read existing or seed default
EXISTING=$(jq '.' "$SETTINGS" 2>/dev/null || echo '{"hooks":{}}')

# 2c. Filter: drop any Stop hook command ending in our 2 script names
#     Also strips legacy paths (suggest-fresh-session.sh / log-review-session.sh)
#     so the first run cleans up any pre-skill installation.
FILTERED=$(printf '%s' "$EXISTING" | jq '
    .hooks.Stop = ((.hooks.Stop // []) | map(
        .hooks |= map(select(
            (.command | test("/master-review-suggest-fresh\\.sh$") | not) and
            (.command | test("/master-review-log-session\\.sh$") | not) and
            (.command | test("/suggest-fresh-session\\.sh$") | not) and
            (.command | test("/log-review-session\\.sh$") | not)
        ))
    ) | map(select(.hooks | length > 0)))
')

# 2d. Merge our hooks.json under .hooks.Stop
MERGED=$(printf '%s' "$FILTERED" | jq --slurpfile new "$HOOKS_JSON" '
    .hooks.Stop = ((.hooks.Stop // []) + ($new[0].Stop // []))
')

# 2e. Atomic write
if [ "$DRY_RUN" = "false" ]; then
	mkdir -p "$(dirname "$SETTINGS")"
	printf '%s' "$MERGED" | jq '.' > "$SETTINGS.tmp"
	mv "$SETTINGS.tmp" "$SETTINGS"
fi

NEW_COUNT=$(printf '%s' "$MERGED" | jq '[.hooks.Stop[].hooks[] | select((.command | test("master-review-")))] | length')
echo "✓ hooks merged ($NEW_COUNT master-review hooks now active under .hooks.Stop)"

if [ "$DRY_RUN" = "true" ]; then
	echo
	echo "(dry-run: nothing was written. Re-run without --dry-run to apply.)"
	exit 0
fi

echo
echo "Done. Run /master-review <pr_number> to start. First run on a project"
echo "without review-config.md will trigger the interactive bootstrap."

#!/bin/bash
# Orchestrator for vscode-macos/*.py
#
# Runs the primary patch(es) against the installed VS Code bundle.
# Fallback scripts (see below) are opt-in and skipped here — invoke them
# manually if the primary doesn't unblock cross-Space fullscreen focus.
#
# Per-script exit status is captured and logged so it's trivial to see
# which regex broke after a VS Code auto-update.
#
# Always exits 0 (host post-install won't fail on cosmetic patches).
#
# Usage
# -----
#   sudo bash run-all.sh [APP_BUNDLE_PATH]
#
# If APP_BUNDLE_PATH is omitted each script auto-discovers via
# _common.resolve_app_bundle() — /Applications/Visual Studio Code.app,
# then Insiders / Cursor / Windsurf as fallback.

set -u

DIR="$(cd "$(dirname "$0")" && pwd)"
APP_ARG="${1:-}"

GREEN='\033[32m'
RED='\033[31m'
YELLOW='\033[33m'
BOLD='\033[1m'
RESET='\033[0m'

SKIP=(
    "_common.py"
    # Opt-in fallback — see README for manual invocation.
    "focus-unconditional-fallback.py"
)

is_skipped() {
    local name="$1"
    for s in "${SKIP[@]}"; do
        [ "$name" = "$s" ] && return 0
    done
    return 1
}

failed=()
ok=()
total=0

shopt -s nullglob
for py in "$DIR"/*.py; do
    name="${py##*/}"
    if is_skipped "$name"; then
        continue
    fi
    total=$((total + 1))
    echo ""
    printf '%b→ %s%b\n' "$BOLD" "$name" "$RESET"
    if python3 "$py" $APP_ARG; then
        ok+=("$name")
    else
        rc=$?
        failed+=("$name (exit $rc)")
    fi
done

echo ""
printf '%b═══ vscode-macos summary (%d script%s) ═══%b\n' \
    "$BOLD" "$total" "$([ "$total" -eq 1 ] && echo '' || echo 's')" "$RESET"
if [ "${#ok[@]}" -gt 0 ]; then
    for n in "${ok[@]}"; do
        printf '  %bOK%b  %s\n' "$GREEN" "$RESET" "$n"
    done
fi
if [ "${#failed[@]}" -gt 0 ]; then
    for n in "${failed[@]}"; do
        printf '  %bFAILED%b  %s\n' "$RED" "$RESET" "$n"
    done
    printf '%bNote%b: orchestrator stays green — these are cosmetic host patches.\n' \
        "$YELLOW" "$RESET"
fi

exit 0

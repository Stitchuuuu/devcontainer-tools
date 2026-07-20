#!/bin/bash
# Orchestrator for vscode-ext-patchs/*.py
#
# Runs each patch script (alphabetical order, _common.py excluded) against
# the given Claude Code extension directory. Per-script exit status is
# captured and logged, so build logs make it trivial to see WHICH feature's
# regex broke after a Claude Code version bump.
#
# Always exits 0 (build stays green). Cosmetic patches must not fail the
# container build.
#
# Usage
# -----
#   run-all.sh [EXT_DIR]
#
# If EXT_DIR is omitted, each script auto-discovers via _common.resolve_ext_dir().

set -u

DIR="$(cd "$(dirname "$0")" && pwd)"
EXT_DIR="${1:-}"

GREEN='\033[32m'
RED='\033[31m'
YELLOW='\033[33m'
BOLD='\033[1m'
RESET='\033[0m'

failed=()
ok=()
total=0

shopt -s nullglob
for py in "$DIR"/*.py; do
    name="${py##*/}"
    [ "$name" = "_common.py" ] && continue
    total=$((total + 1))
    echo ""
    printf '%b→ %s%b\n' "$BOLD" "$name" "$RESET"
    if python3 "$py" $EXT_DIR; then
        ok+=("$name")
    else
        rc=$?
        failed+=("$name (exit $rc)")
    fi
done

echo ""
printf '%b═══ vscode-ext-patchs summary (%d script%s) ═══%b\n' \
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
    printf '%bNote%b: orchestrator stays green — these are cosmetic patches.\n' \
        "$YELLOW" "$RESET"
fi

exit 0

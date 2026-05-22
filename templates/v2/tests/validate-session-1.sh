#!/usr/bin/env bash
# validate-session-1.sh — multi-rebuild orchestrator for session 1 validation.
#
# Each invocation :
#   1. Reads the currently baked firewall mode from /etc/devcontainer-firewall/default-mode.
#   2. Runs the integration test suite.
#   3. Appends the result to .devcontainer/logs/session-1-validation.log
#      (gitignored, persists across rebuilds via workspace bind mount).
#   4. Tells you what to do next (flip mode + rebuild, or commit if done).
#
# Usage : bash .devcontainer/tests/validate-session-1.sh
#
# State file format (one line per validated mode) :
#   <ISO8601-timestamp> <mode> <pass|fail> <pass-count>/<fail-count>/<skip-count>
#
# Reset progress : rm .devcontainer/logs/session-1-validation.log

set +e
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STATE_DIR=/workspace/.devcontainer/logs
STATE="$STATE_DIR/session-1-validation.log"
mkdir -p "$STATE_DIR"

BOLD=$'\033[1m'; RED=$'\033[1;31m'; GREEN=$'\033[1;32m'
YELLOW=$'\033[1;33m'; CYAN=$'\033[1;36m'; RST=$'\033[0m'

if [ ! -f /.dockerenv ] && [ -z "${REMOTE_CONTAINERS:-}" ]; then
  echo "${RED}This script must run inside the rebuilt devcontainer, not on the host.${RST}" >&2
  exit 2
fi

if [ ! -d /etc/devcontainer-firewall ]; then
  echo "${RED}/etc/devcontainer-firewall missing — wrong image?${RST}" >&2
  exit 2
fi

# Detect bake state.
post_bake=1
if command -v findmnt >/dev/null 2>&1; then
  findmnt -n /etc/devcontainer-firewall >/dev/null 2>&1 && post_bake=0
else
  awk '$5 == "/etc/devcontainer-firewall" {found=1} END {exit !found}' /proc/self/mountinfo 2>/dev/null && post_bake=0
fi
if [ $post_bake -eq 0 ]; then
  echo "${RED}Container is pre-bake (bind mount still active). Rebuild first.${RST}" >&2
  echo "  Cmd+Shift+P → Dev Containers: Rebuild Container" >&2
  exit 2
fi

mode=$(cat /etc/devcontainer-firewall/default-mode 2>/dev/null | tr -d '[:space:]')
case "$mode" in
  strict|basic|off) ;;
  *) echo "${RED}Unknown baked mode: '$mode'${RST}" >&2; exit 2 ;;
esac

echo "${BOLD}${CYAN}== Session 1 validation — currently baked mode: $mode ==${RST}"
echo

# Run the integration suite.
out=$(bash "$HERE/run.sh" --integration 2>&1)
echo "$out"
echo

# Parse aggregate. Strip ANSI first (same as run.sh).
agg=$(echo "$out" | sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' | grep -E "^\s*Aggregate" | tail -1)
p=$(echo "$agg"  | sed -nE 's/.*[^0-9]([0-9]+) pass.*/\1/p')
f=$(echo "$agg" | sed -nE 's/.*[^0-9]([0-9]+) fail.*/\1/p')
s=$(echo "$agg" | sed -nE 's/.*[^0-9]([0-9]+) skip.*/\1/p')
p=${p:-0}; f=${f:-0}; s=${s:-0}

status="pass"
[ "$f" -gt 0 ] && status="fail"

# Append to state file.
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "$ts $mode $status $p/$f/$s" >> "$STATE"

echo "${BOLD}── State so far ──${RST}"
cat "$STATE"
echo

# Determine progress : which of {strict, basic, off} have a *passing* run.
have_strict=0; have_basic=0; have_off=0
while read -r _ts m st _counts; do
  [ "$st" != "pass" ] && continue
  case "$m" in
    strict) have_strict=1 ;;
    basic)  have_basic=1  ;;
    off)    have_off=1    ;;
  esac
done < "$STATE"

remaining=()
[ $have_strict -eq 0 ] && remaining+=("strict")
[ $have_basic  -eq 0 ] && remaining+=("basic")
[ $have_off    -eq 0 ] && remaining+=("off")

if [ "$f" -gt 0 ]; then
  echo "${RED}✗ Current run FAILED ($f assertion(s)). Don't proceed — investigate.${RST}"
  exit 1
fi

if [ ${#remaining[@]} -eq 0 ]; then
  echo "${GREEN}${BOLD}✅ All 3 modes validated (strict, basic, off).${RST}"
  echo
  echo "Ready to commit. Recommended :"
  echo "  1. Flip back to strict if you're currently in basic/off :"
  echo "     bash .devcontainer/firewall-mode.sh strict && Rebuild Container"
  echo "  2. Stage + commit the bake changes (see the proposed commit message in chat)."
  echo
  echo "Optional next : claude-switch mini-matrix (see TEST-PLAN.md)."
  exit 0
fi

next="${remaining[0]}"
echo "${YELLOW}${BOLD}Next step :${RST} flip to mode '$next' and rebuild."
echo
echo "  bash .devcontainer/firewall-mode.sh $next"
echo "  → Cmd+Shift+P → Dev Containers: Rebuild Container"
echo "  → Once back inside : bash .devcontainer/tests/validate-session-1.sh"
echo
echo "Remaining modes to validate : ${remaining[*]}"
exit 0

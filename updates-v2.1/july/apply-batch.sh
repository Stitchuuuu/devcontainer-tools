#!/usr/bin/env bash
# apply-batch.sh — idempotent applicator for the july batch.
#
# For each .patch in this directory (sorted alphabetically):
#   - if `git apply --reverse --check` succeeds, the patch is already
#     applied → skip with a note.
#   - else run `git apply --check <patch>` : on success, apply ; on
#     failure, print the error and stop (caller decides).
#
# Run from the repo root that contains .devcontainer/ (i.e. your
# downstream project). The script does not commit — that's on you
# after each successful apply, or once at the end.
#
# Usage :
#   bash updates-v2.1/july/apply-batch.sh
#   bash updates-v2.1/july/apply-batch.sh --dry-run    # check-only
#   bash updates-v2.1/july/apply-batch.sh --continue   # keep going on error

set -uo pipefail

DRY_RUN=0
CONTINUE=0
for arg in "$@"; do
	case "$arg" in
		--dry-run)  DRY_RUN=1 ;;
		--continue) CONTINUE=1 ;;
		-h|--help)
			sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
			exit 0
			;;
		*)
			echo "unknown arg: $arg" >&2
			exit 2
			;;
	esac
done

BATCH_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$(git rev-parse --show-toplevel)" || {
	echo "not in a git repo" >&2
	exit 1
}

applied=0
skipped=0
failed=0

for patch in "$BATCH_DIR"/*.patch; do
	name="$(basename "$patch")"
	printf '\n\033[1m==> %s\033[0m\n' "$name"

	if git apply --reverse --check "$patch" >/dev/null 2>&1; then
		echo "   skip (already applied — reverse-check passed)"
		skipped=$((skipped + 1))
		continue
	fi

	if ! git apply --check "$patch" 2>/tmp/apply-check.err; then
		echo "   FAIL — patch does not apply cleanly :"
		sed 's/^/     /' /tmp/apply-check.err
		failed=$((failed + 1))
		[ "$CONTINUE" -eq 1 ] || {
			echo
			echo "stopped at $name. Re-run with --continue to skip and keep going."
			break
		}
		continue
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "   would apply (dry-run)"
		applied=$((applied + 1))
		continue
	fi

	if git apply "$patch"; then
		echo "   applied"
		applied=$((applied + 1))
	else
		echo "   FAIL during apply (state may be partial — inspect \`git status\`)"
		failed=$((failed + 1))
		[ "$CONTINUE" -eq 1 ] || break
	fi
done

printf '\n\033[1msummary\033[0m — applied: %d  skipped: %d  failed: %d\n' \
	"$applied" "$skipped" "$failed"

exit "$failed"

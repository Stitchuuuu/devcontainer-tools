#!/usr/bin/env bash
# gen-kickoff-prompt.sh — generate copy-pasteable kickoff prompt for resuming a review.
#
# Usage:  gen-kickoff-prompt.sh <pr_number> [recap_path]
#
# Reads PR-<N>-recap.md (or the explicit path) and prints a fenced markdown
# block to stdout containing:
#   - PR number, current round, tier guess
#   - count of fixed findings, count of open findings
#   - surfaces marked covered (A-G)
#   - last 4 commit hashes (current branch)
#   - resume command line (/master-review --resume --surfaces=...)
#
# Exits 0 on success, 1 if recap file missing, 2 on bad args.

set -uo pipefail

PR_NUMBER="${1:-}"
RECAP_PATH="${2:-}"

if [[ -z "$PR_NUMBER" ]]; then
	echo "usage: gen-kickoff-prompt.sh <pr_number> [recap_path]" >&2
	exit 2
fi

# Default recap path if not provided. Check workspace root then cwd.
if [[ -z "$RECAP_PATH" ]]; then
	for candidate in "/workspace/PR-${PR_NUMBER}-recap.md" "PR-${PR_NUMBER}-recap.md" "./PR-${PR_NUMBER}-recap.md"; do
		if [[ -f "$candidate" ]]; then
			RECAP_PATH="$candidate"
			break
		fi
	done
fi

if [[ -z "$RECAP_PATH" || ! -f "$RECAP_PATH" ]]; then
	echo "error: recap file not found for PR ${PR_NUMBER} (looked at /workspace/PR-${PR_NUMBER}-recap.md and ./PR-${PR_NUMBER}-recap.md)" >&2
	exit 1
fi

# Count fixed findings: rows in decisions tables marked Fixed (case-insensitive,
# must match the decision column, not random "fixed" mentions in prose).
# Recap tables use:  | R<N>-<M> | desc | **Fixed** ... | hash |
#                or  | <num>    | desc | **Fixed** ... | hash |
fixed_count=$(grep -cE '^\|[^|]+\|[^|]+\|[^|]*\*?\*?[Ff]ixed' "$RECAP_PATH" || true)

# Count open findings: rows marked Open (LOW/MED/HIGH labels permitted).
open_count=$(grep -cE '^\|[^|]+\|[^|]+\|[^|]*\*?\*?[Oo]pen' "$RECAP_PATH" || true)

# Latest round seen in the recap (highest "Round N decisions" or "round N").
last_round=$(grep -ioE 'round[ -]+[0-9]+' "$RECAP_PATH" | grep -oE '[0-9]+' | sort -n | tail -1)
last_round="${last_round:-?}"

# Detect surfaces covered: lines like "[A scope ✓]" or "[A] ✓" or
# "Surface A — ...covered" or "A scope ✓". Conservative: scan for letter
# tokens A through G followed by ✓ in the same line, OR " A," / " B," in a
# "Surfaces" / "Surfaces couvertes" line.
surfaces=$(grep -iE 'surface' "$RECAP_PATH" | head -20 | grep -oE '\b[A-G]\b' | sort -u | tr '\n' ',' | sed 's/,$//')
if [[ -z "$surfaces" ]]; then
	# Fallback: scan headers / bullets for "Surface X" or "[X scope]"
	surfaces=$(grep -oE '\[[A-G][^]]*\]' "$RECAP_PATH" | grep -oE '^\[[A-G]' | grep -oE '[A-G]' | sort -u | tr '\n' ',' | sed 's/,$//')
fi
surfaces="${surfaces:-?}"

# Last 4 commit hashes from current branch — short SHAs.
if git rev-parse --git-dir >/dev/null 2>&1; then
	commits=$(git log --pretty=format:'%h %s' -n 4 2>/dev/null | sed 's/^/  - /')
	branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
else
	commits="  - (not in a git repo)"
	branch="?"
fi
commits="${commits:-  - (no commits)}"

# Tier guess: scan recap for "Tier", "T1"-"T4+", "tier:" header.
tier=$(grep -ioE '\b[Tt]ier[: ]+T?[0-9]\+?' "$RECAP_PATH" | head -1 | grep -oE 'T[0-9]\+?')
if [[ -z "$tier" ]]; then
	tier=$(grep -oE '\bT[0-9]\+?\b' "$RECAP_PATH" | head -1)
fi
tier="${tier:-?}"

# Output the kickoff block.
cat <<EOF
=========================================================================
NOUVELLE SESSION RECOMMANDÉE — copie-colle ce prompt dans la session neuve :
─────────────────────────────────────────────────────────────────────────
Reprise review PR #${PR_NUMBER} — round ${last_round} (tier ${tier}).

État courant :
- Branch : ${branch}
- Findings fixés : ${fixed_count}
- Findings open : ${open_count}
- Surfaces déjà couvertes : ${surfaces}
- 4 derniers commits :
${commits}

Avant tout autre prompt :
1. cat PR-${PR_NUMBER}-recap.md
2. cat PR-${PR_NUMBER}-review.md
3. Lance /master-review ${PR_NUMBER} --resume --surfaces=${surfaces}
=========================================================================
EOF

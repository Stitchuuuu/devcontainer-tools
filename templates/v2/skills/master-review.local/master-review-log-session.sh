#!/usr/bin/env bash
# log-review-session.sh — Claude Code Stop hook (E).
#
# Appends one TSV line to ~/.claude/review-sessions.log per review session.
# Only logs when a PR-*-review.md or PR-*-recap.md is currently modified in
# `git status` — outside a review context, exits silently.
#
# Format (tab-separated):
#   timestamp  pr_number  tier  round  session_id  jsonl_lines  user_prompts \
#   new_findings  fixed_findings  surfaces_checked  duration_min
#
# Detection:
#   - pr_number       : PR-<N>-review.md / PR-<N>-recap.md filename, fallback
#                       to branch name if it carries the number
#   - tier            : grep "T<N>" or "Tier:" header in review.md (best-effort)
#   - round           : highest "Round N" / "round-N" / "R<N>-" in review.md
#   - session_id      : transcript_path basename (UUID without .jsonl) or
#                       hook stdin field
#   - jsonl_lines     : wc -l on transcript
#   - user_prompts    : count of user turns (type==user, userType==external)
#   - new_findings    : count of "R<round>-<M>" entries appearing in the
#                       *unstaged* diff of recap.md (=added this session)
#   - fixed_findings  : count of "Fixed" rows added in unstaged diff of recap.md
#   - surfaces_checked: comma-joined surface letters (A-G) seen in review.md
#   - duration_min    : transcript duration via first/last timestamps

set -uo pipefail

LOG_FILE="${REVIEW_SESSIONS_LOG:-${HOME}/.claude/review-sessions.log}"

# Read hook stdin (Stop hook contract). Best-effort, non-fatal.
input=""
if [ ! -t 0 ]; then
	input=$(cat 2>/dev/null || true)
fi

transcript_path=""
session_id=""
if [[ -n "$input" ]]; then
	if command -v jq >/dev/null 2>&1; then
		transcript_path=$(echo "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)
		session_id=$(echo "$input" | jq -r '.session_id // empty' 2>/dev/null || true)
	elif command -v python3 >/dev/null 2>&1; then
		read -r transcript_path session_id <<<$(echo "$input" | python3 -c '
import sys, json
try:
  d = json.load(sys.stdin)
  print(d.get("transcript_path",""), d.get("session_id",""))
except: pass' 2>/dev/null || true)
	fi
fi

# Test/CI override.
if [[ -n "${CLAUDE_SESSION_FILE:-}" ]]; then
	transcript_path="$CLAUDE_SESSION_FILE"
fi
if [[ -n "${CLAUDE_SESSION_ID:-}" ]]; then
	session_id="$CLAUDE_SESSION_ID"
fi

# Derive session_id from transcript filename if missing.
if [[ -z "$session_id" && -n "$transcript_path" ]]; then
	session_id=$(basename "$transcript_path" .jsonl)
fi
session_id="${session_id:-unknown}"
# Truncate to short id (8 chars) for compactness.
session_id_short=$(echo "$session_id" | cut -c1-8)

# Locate the modified PR review/recap file. If none, this isn't a review
# session — bail silently.
review_file=""
recap_file=""
if git rev-parse --git-dir >/dev/null 2>&1; then
	mapfile -t pr_files < <(git status --porcelain 2>/dev/null \
		| awk '{ for (i=2;i<=NF;i++) print $i }' \
		| grep -E '(^|/)PR-[0-9]+-(review|recap)\.md$' || true)
	for f in "${pr_files[@]}"; do
		case "$f" in
			*review.md) review_file="$f" ;;
			*recap.md)  recap_file="$f" ;;
		esac
	done
fi

if [[ -z "$review_file" && -z "$recap_file" ]]; then
	exit 0
fi

# Prefer review.md as the round/tier source; fall back to recap.md.
primary="${review_file:-$recap_file}"
pr_number=$(echo "$primary" | grep -oE 'PR-[0-9]+' | head -1 | grep -oE '[0-9]+')
pr_number="${pr_number:-?}"

# If only one of review.md/recap.md is *modified*, the sibling may still
# exist on disk (committed) and is useful for tier/round/surface inference.
# Prefer the modified-file path when present, else fall back to the on-disk
# sibling at the same directory.
review_dir=$(dirname "$primary")
if [[ -z "$review_file" && "$pr_number" != "?" ]]; then
	candidate="${review_dir}/PR-${pr_number}-review.md"
	[[ -f "$candidate" ]] && review_file="$candidate"
fi
if [[ -z "$recap_file" && "$pr_number" != "?" ]]; then
	candidate="${review_dir}/PR-${pr_number}-recap.md"
	[[ -f "$candidate" ]] && recap_file="$candidate"
fi
primary="${review_file:-$recap_file}"

# Tier — best effort, scan both review.md and recap.md.
tier="?"
for src in "$review_file" "$recap_file"; do
	[[ -z "$src" || ! -f "$src" ]] && continue
	candidate=$(grep -ioE '\b[Tt]ier[: ]+T?[0-9]\+?' "$src" | head -1 | grep -oE 'T[0-9]\+?')
	if [[ -z "$candidate" ]]; then
		candidate=$(grep -oE '\bT[0-9]\+?\b' "$src" | head -1)
	fi
	if [[ -n "$candidate" ]]; then
		tier="$candidate"
		break
	fi
done

# Round — highest seen in review.md, fall back to recap.md, fall back to "?".
round="?"
for src in "$review_file" "$recap_file"; do
	[[ -z "$src" || ! -f "$src" ]] && continue
	r=$(grep -ioE 'round[ -]+[0-9]+|R[0-9]+-' "$src" | grep -oE '[0-9]+' | sort -n | tail -1)
	if [[ -n "$r" ]]; then
		round="$r"
		break
	fi
done

# JSONL stats.
jsonl_lines=0
user_prompts=0
duration_min=0
if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
	jsonl_lines=$(wc -l <"$transcript_path" | tr -d ' ')
	read -r user_prompts ts_first ts_last <<<$(python3 -c "
import json, sys
n = 0
ts_first = ts_last = ''
try:
  with open(sys.argv[1]) as f:
    for line in f:
      try:
        d = json.loads(line)
      except Exception:
        continue
      ts = d.get('timestamp','')
      if ts:
        if not ts_first: ts_first = ts
        ts_last = ts
      if d.get('type') == 'user' and d.get('userType') == 'external' and not d.get('isSidechain'):
        n += 1
except Exception:
  pass
print(n, ts_first or '-', ts_last or '-')
" "$transcript_path" 2>/dev/null)

	if [[ -n "$ts_first" && -n "$ts_last" && "$ts_first" != "-" && "$ts_last" != "-" ]]; then
		duration_min=$(python3 -c "
from datetime import datetime
import sys
try:
  a = datetime.fromisoformat(sys.argv[1].replace('Z','+00:00'))
  b = datetime.fromisoformat(sys.argv[2].replace('Z','+00:00'))
  print(int(round((b-a).total_seconds()/60)))
except Exception:
  print(0)
" "$ts_first" "$ts_last" 2>/dev/null || echo 0)
	fi
fi

# new_findings: R<round>-<M> patterns added in unstaged diff of recap.md.
# fixed_findings: rows containing "Fixed" added in unstaged diff of recap.md.
new_findings=0
fixed_findings=0
if [[ -n "$recap_file" && -f "$recap_file" ]] && git rev-parse --git-dir >/dev/null 2>&1; then
	# Git diff for added lines starts with "+", excluding "+++" headers.
	added=$(git diff -- "$recap_file" 2>/dev/null | grep -E '^\+' | grep -vE '^\+\+\+')
	if [[ "$round" =~ ^[0-9]+$ ]]; then
		new_findings=$(echo "$added" | grep -cE "R${round}-[0-9]+" || true)
	else
		new_findings=$(echo "$added" | grep -cE 'R[0-9]+-[0-9]+' || true)
	fi
	fixed_findings=$(echo "$added" | grep -ciE '\*\*[Ff]ixed\*\*|\| *[Ff]ixed' || true)
fi

# Surfaces checked — pull A-G letters from review.md "Surfaces" lines.
surfaces=""
if [[ -n "$review_file" && -f "$review_file" ]]; then
	surfaces=$(grep -iE 'surface' "$review_file" | grep -oE '\b[A-G]\b' | sort -u | tr '\n' ',' | sed 's/,$//')
fi
surfaces="${surfaces:-?}"

# Compose and append.
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

mkdir -p "$(dirname "$LOG_FILE")"
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
	"$timestamp" \
	"$pr_number" \
	"$tier" \
	"$round" \
	"$session_id_short" \
	"$jsonl_lines" \
	"$user_prompts" \
	"$new_findings" \
	"$fixed_findings" \
	"$surfaces" \
	"$duration_min" \
	>>"$LOG_FILE"

exit 0

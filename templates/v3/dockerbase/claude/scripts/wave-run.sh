#!/usr/bin/env bash
# wave-run.sh — run several rollout sessions in parallel, one git worktree
# each, headless.
#
#   .devcontainer/claude/scripts/wave-run.sh plans/<plan>/sessions/session-*.md
#
# Generalised from a one-off wave runner written for a single rollout.
# Everything operational here was learned there — the dependency symlinks, the
# probe, the headless preamble, the progress markers. What is new is that the
# session list is the argument list and the model comes from each prompt's own
# "Model : Tier X" line, so this works for any plan without editing the script.
#
# Progress markers, for the watch-log Monitor pattern :
#   PREFLIGHT ok | FATAL <reason>
#   [<slug>] START model=<id>
#   [<slug>] DONE rc=0 commits=<n> in <mm:ss>
#   [<slug>] FAIL rc=<n> commits=<n> in <mm:ss>
#   PROGRESS <n>/<total>
#   WAVE COMPLETE | WAVE PARTIAL failed=<n>
#   __END__
#
# ── Why this script exists and who runs it ────────────────────────────────
# It launches `claude` with --dangerously-skip-permissions, because a headless
# session that stops on a permission prompt just hangs until it is killed.
# That flag is a HUMAN decision, so this script REFUSES to run without a
# terminal : an automated caller (an agent's tool call, a cron, a pipe) gets
# exit 3. WAVE_RUN_FORCE=1 removes the check — for the human who means it.
#
# The blast radius is the devcontainer : no docker, an outbound firewall, the
# worktrees under /tmp. Nothing pushes ; merging is a separate human step.
set -uo pipefail
trap 'echo "__END__"' EXIT

ROOT=$(git rev-parse --show-toplevel)
WT_BASE="${WAVE_RUN_ROOT:-/tmp/wave}"
BRANCH_PREFIX="${WAVE_RUN_BRANCH_PREFIX:-wave}"
CONC="${WAVE_RUN_CONC:-3}"
TMO="${WAVE_RUN_TIMEOUT:-4h}"
RUNLOG="$WT_BASE/runs"
# Repo-relative directories to symlink from the main checkout into each
# worktree. `git worktree add` gives a clean tree with no installed
# dependencies, and a session that cannot lint or test is the single most
# expensive thing to discover late. Space-separated ; a path the main checkout
# does not have is skipped, so the default covers a plain npm repo and a
# monorepo names its own (`WAVE_RUN_LINK='node_modules services/api/node_modules'`).
LINK_DIRS="${WAVE_RUN_LINK:-node_modules}"
# The probe command, run in ONE worktree before N sessions are spent against a
# setup that cannot test. Project-specific by nature — keep it narrow.
PROBE_CMD="${WAVE_RUN_PROBE_CMD:-}"

BOLD=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31;1m'; GREEN=$'\033[32;1m'; YELLOW=$'\033[33;1m'; RESET=$'\033[0m'

die() { echo "FATAL $*"; exit 1; }

if [ $# -eq 0 ]; then
	cat <<EOF
${BOLD}wave-run.sh${RESET} — parallel rollout sessions, one worktree each

  wave-run.sh <session-prompt.md>...

Each prompt becomes a worktree under ${WT_BASE}/<slug> on branch
${BRANCH_PREFIX}/<slug>, and a headless claude session working it. The model
comes from the prompt's own "Model : Tier X" line (A→fable-5.1, B→opus-5,
C→sonnet-5, D→haiku) ; WAVE_RUN_MODEL overrides for all.

  ${DIM}wave-run.sh plans/<plan>/sessions/session-{7,9}-*.md${RESET}

Env : WAVE_RUN_ROOT · WAVE_RUN_BRANCH_PREFIX · WAVE_RUN_CONC (${CONC})
      WAVE_RUN_TIMEOUT (${TMO}) · WAVE_RUN_MODEL · WAVE_RUN_FORCE
      WAVE_RUN_LINK (${LINK_DIRS}) — dirs symlinked into each worktree
      WAVE_RUN_PROBE_CMD — a narrow test run in one worktree first
EOF
	exit 2
fi

# The human gate. An agent's Bash call has no controlling terminal, so this is
# a structural check rather than a promise in a comment. It stops an ACCIDENT
# — a cron, a pipe, an agent — not a determined bypass ; the real boundary is
# the harness that gates the agent's own tool calls.
if [ ! -t 0 ] && [ "${WAVE_RUN_FORCE:-}" != "1" ]; then
	printf '%s\n' "${RED}wave-run.sh needs a terminal.${RESET}" >&2
	printf '%s\n' "  It starts claude sessions with permission checks disabled — a decision a human takes." >&2
	printf '%s\n' "  Run it yourself from a shell, or set WAVE_RUN_FORCE=1 if you are that human and mean it." >&2
	exit 3
fi

# Tier → model. The tiers are the plan's (ROLLOUT.md § Model tiers) ; only the
# primary is used, because a fallback ladder needs a human watching.
model_for() {
	local prompt_file=$1 tier
	[ -n "${WAVE_RUN_MODEL:-}" ] && { printf '%s' "$WAVE_RUN_MODEL"; return; }
	tier=$(grep -m1 -oE 'Tier[[:space:]]*[A-D]' "$prompt_file" | grep -oE '[A-D]$')
	case "$tier" in
		A) printf 'claude-fable-5-1' ;;
		B) printf 'claude-opus-5' ;;
		C) printf 'claude-sonnet-5' ;;
		D) printf 'claude-haiku-4-5-20251001' ;;
		*) printf 'claude-opus-5' ;;
	esac
}

# ── Preflight ───────────────────────────────────────────────────────────
echo "=== preflight ==="
cd "$ROOT" || die "cannot cd $ROOT"
command -v claude >/dev/null || die "claude CLI not found"

declare -a SLUGS=() PROMPTS=() MODELS=()
for prompt_file in "$@"; do
	[ -f "$prompt_file" ] || die "no such prompt file : $prompt_file"
	SLUGS+=("$(basename "$prompt_file" .md)")
	PROMPTS+=("$(realpath "$prompt_file")")
	MODELS+=("$(model_for "$prompt_file")")
done
TOTAL=${#SLUGS[@]}
echo "  $TOTAL session prompt(s) present"

FREE_G=$(df -BG --output=avail /tmp | tail -1 | tr -dc '0-9')
echo "  free disk: ${FREE_G}G"
[ "${FREE_G:-0}" -lt 5 ] && die "less than 5G free on /tmp"

HEAD_SHA=$(git rev-parse --short HEAD)
echo "  base: $(git rev-parse --abbrev-ref HEAD) @ $HEAD_SHA"

DIRTY=$(git status --porcelain | grep -vE '^\?\? (docs/reviews/|plans/)' || true)
[ -n "$DIRTY" ] && { echo "  ${YELLOW}⚠ uncommitted, absent from the worktrees:${RESET}";
                     echo "$DIRTY" | sed 's/^/      /'; }

mkdir -p "$WT_BASE" "$RUNLOG"

# Worktrees are created SERIALLY : concurrent `git worktree add` races on
# .git/worktrees. Dependencies are symlinked (see LINK_DIRS), without which a
# session cannot run lint or tests at all.
echo "  creating $TOTAL worktree(s)…"
for i in "${!SLUGS[@]}"; do
	slug=${SLUGS[$i]}; wt="$WT_BASE/$slug"; br="$BRANCH_PREFIX/$slug"
	git worktree remove --force "$wt" 2>/dev/null
	git branch -D "$br" 2>/dev/null
	git worktree add -q -b "$br" "$wt" HEAD || die "worktree add failed for $slug"
	for rel in $LINK_DIRS; do
		# A path the main checkout does not have is not an error : the default
		# covers a plain npm repo, and a monorepo overrides it wholesale.
		[ -e "$ROOT/$rel" ] || continue
		mkdir -p "$(dirname "$wt/$rel")"
		ln -sfn "$ROOT/$rel" "$wt/$rel"
	done
done

# The probe : one worktree runs a narrow real test before N sessions are spent
# against a setup that cannot test. Off unless WAVE_RUN_PROBE_CMD names one,
# because only the project knows which test is both fast and representative.
if [ -n "$PROBE_CMD" ]; then
	echo "  probing a worktree (the real check)…"
	probe_wt="$WT_BASE/${SLUGS[0]}"
	if (cd "$probe_wt" && eval "$PROBE_CMD") >"$RUNLOG/probe.log" 2>&1; then
		echo "  probe OK"
	else
		tail -25 "$RUNLOG/probe.log" | sed 's/^/      /'
		die "a worktree with symlinked dependencies cannot run \`$PROBE_CMD\` — fix before launching $TOTAL sessions"
	fi
fi
echo "PREFLIGHT ok"
echo

# ── Headless run context, prepended to every prompt ──────────────────────
# Session prompts end with "do NOT commit without explicit user confirmation".
# Headless, that stalls. This lifts it for this run only, leaving the prompt
# files valid for a manual re-run.
read -r -d '' OVERRIDE <<'PROMPTEOF' || true
# RUN CONTEXT — read this first, it overrides part of the prompt below

You are running HEADLESS. No human can answer you in this session, so
anything you would normally ask, you decide and write down.

1. COMMIT AUTHORISATION. The user has explicitly authorised you to commit
   this session's work. This overrides any "do NOT commit without explicit
   user confirmation" line below. Commit on the CURRENT branch once your
   gate is green. Do NOT push, do NOT merge, do NOT touch another branch.
2. GATE BEFORE COMMIT, no exception. If the session's DoD names a gate, it
   must be green before you commit. A failing gate is a not-done state :
   record what failed and stop, rather than committing and "addressing it
   in follow-up".
3. WRITE YOUR REPORT, NOT THE SHARED FILES. Sibling sessions are running
   right now against the same absolute paths. Do NOT edit your plan's
   STATUS.md, LOG.md or EXISTING.md — you would overwrite them. Write ONE
   file instead, under your plan directory :
       <plan>/wave-reports/<this session's slug>.md
   First line exactly `STATUS: done` | `STATUS: partial` | `STATUS: blocked`,
   then the LOG.md section your DoD describes, then a `## EXISTING.md delta`
   heading with what you would have put there.
4. STAY IN YOUR LANE. Edit only the files your prompt says you own — even
   something obviously broken elsewhere belongs to a sibling session and
   your edit will be lost or will conflict. Note it in your report.
5. PARTIAL BEATS OVERCLAIMED. If you cannot finish, finish what you can,
   commit that, and mark `STATUS: partial` with what is left and why.
   Never silently drop an item.
6. DO NOT run any `wtf claude-live *` command — it drives the human's
   browser, and other sessions are running.

---

PROMPTEOF

# ── Launch ──────────────────────────────────────────────────────────────
echo "=== launching (concurrency $CONC, timeout $TMO/session) ==="

launch_one() {
	local slug="$1" prompt="$2" model="$3"
	local wt="$WT_BASE/$slug" start=$SECONDS
	local pf="$RUNLOG/$slug.prompt.md"
	{ printf '%s\n' "$OVERRIDE"; cat "$prompt"; } >"$pf"

	echo "[$slug] START model=$model"
	# NOTIFY_SUPPRESS_SESSION : these sessions inherit the notify-queue hooks
	# like any other, and N of them would bury the human's own banners under
	# thousands of tool events. This script reports progress itself.
	( cd "$wt" && AI_AGENT=1 NOTIFY_SUPPRESS_SESSION="$BRANCH_PREFIX/$slug" \
			timeout "$TMO" claude -p "$(cat "$pf")" \
			--model "$model" --add-dir "$ROOT" --dangerously-skip-permissions \
	) >"$RUNLOG/$slug.out" 2>"$RUNLOG/$slug.err"
	local rc=$?

	local secs=$((SECONDS-start)) dur commits
	dur=$(printf '%02d:%02d' $((secs/60)) $((secs%60)))
	commits=$(git -C "$wt" rev-list --count "$HEAD_SHA..HEAD" 2>/dev/null || echo '?')
	echo "$rc" >"$RUNLOG/$slug.rc"

	if [ "$rc" -eq 0 ]; then echo "[$slug] DONE rc=0 commits=$commits in $dur"
	else                     echo "[$slug] FAIL rc=$rc commits=$commits in $dur"; fi
	echo "PROGRESS $(ls "$RUNLOG"/*.rc 2>/dev/null | wc -l | tr -d ' ')/$TOTAL"
}

for i in "${!SLUGS[@]}"; do
	while [ "$(jobs -rp | wc -l)" -ge "$CONC" ]; do sleep 5; done
	launch_one "${SLUGS[$i]}" "${PROMPTS[$i]}" "${MODELS[$i]}" &
	sleep 2
done
wait

# ── Summary ─────────────────────────────────────────────────────────────
echo
echo "=== summary ==="
FAILED=0
for i in "${!SLUGS[@]}"; do
	slug=${SLUGS[$i]}; wt="$WT_BASE/$slug"
	rc=$(cat "$RUNLOG/$slug.rc" 2>/dev/null || echo '?')
	[ "$rc" != "0" ] && FAILED=$((FAILED+1))
	served=$(grep -o '"model":"[^"]*"' "$RUNLOG/$slug.out" 2>/dev/null | head -1 | cut -d'"' -f4)
	c=$(git -C "$wt" rev-list --count "$HEAD_SHA..HEAD" 2>/dev/null || echo '?')
	printf '  %-24s rc=%-3s commits=%-3s asked=%-26s served=%s\n' \
		"$slug" "$rc" "$c" "${MODELS[$i]}" "${served:-(not in output)}"
done
echo "  worktrees: $WT_BASE (kept — 'git worktree list', remove once merged)"
echo "  logs     : $RUNLOG/<slug>.out, <slug>.err"

if [ "$FAILED" -eq 0 ]; then echo "WAVE COMPLETE"; else echo "WAVE PARTIAL failed=$FAILED"; fi

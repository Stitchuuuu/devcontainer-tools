---
description: Multi-agent PR code review with PR analysis, tier scoring, surface coverage, adversarial framings, and metrics. TRIGGER on "review the PR", "review PR XXXX", "fais une review de la PR XXXX", "regarde la PR", "ultrareview", "master review", "deep PR review", or any explicit ask for a multi-pass code review of a pull request. Inspires from the official `/review` for the 5-agent generic core, then wraps it in a project-aware flow that classifies the PR (tier T1-T4+), checks distance from main, dispatches custom domain agents from `review-config.md`, writes local round + recap files, posts a GitHub comment, lints fix-commit hygiene, and appends a TSV metrics row for plateau detection. Skip for trivial doc-only diffs the user explicitly says to merge unreviewed.
argument-hint: "[<pr_number>] [--resume] [--surfaces=A,B,...] [--tier-only] [--read-only] [--config=<path>]"
allowed-tools: Bash(gh issue view:*), Bash(gh search:*), Bash(gh issue list:*), Bash(gh pr comment:*), Bash(gh pr diff:*), Bash(gh pr view:*), Bash(gh pr list:*), Bash(gh pr create:*), Bash(gh pr close:*), Bash(gh api:*), Bash(git:*), Bash(grep:*), Bash(rg:*), Bash(awk:*), Bash(sed:*), Bash(tr:*), Bash(cut:*), Bash(sort:*), Bash(paste:*), Bash(diff:*), Bash(wc:*), Bash(head:*), Bash(tail:*), Bash(jq:*), Bash(date:*), Bash(printf:*), Bash(echo:*), Bash(cat:*), Bash(ls:*), Bash(find:*), Bash(stat:*), Bash(basename:*), Bash(dirname:*), Bash(touch:*), Bash(test:*), Bash(xargs:*), Bash(cp:*), Bash(ln:*), Bash(rm:*), Bash(mv:*), Bash(mkdir:-p /tmp/*), Bash(mkdir:-p .devcontainer/local/*), Bash(mkdir:-p .devcontainer/*), Bash(.:*), Bash(source:*), Bash(update_agent_status:*), Bash(update_scoring_status:*), Read, Write, Edit, Agent, AskUserQuestion
disable-model-invocation: false
---

# Master Review

Generic multi-agent PR review. Step 0 analyses the PR (saturation, distance from main, tier scoring, config parsing, resume detection, dispatch decision). Steps 1-7 inherit the validated core of the upstream `/review` (eligibility, context gather, 5-agent parallel review, Haiku confidence scoring, GitHub comment) — agents 1-5 are reused VERBATIM from the upstream prompt because they have demonstrably worked. Step 7 lints commit hygiene. Step 8 appends a metrics row and emits a kickoff prompt if the session is saturated.

This command is **project-agnostic**. All domain-specific knowledge (custom agents, surfaces, tier weights, framings, output paths, commit-hygiene regex) lives in `review-config.md` per project. If absent, the command runs in vanilla mode (agents 1-5 + base tier weights only) and prints `note: no review-config.md found, running in vanilla mode (generic agents 1-5 only).`

To do this, follow these steps precisely. Make a todo list first.

---

## Argument parsing

Before Step 0, parse `$ARGUMENTS` with a small case loop. Extract:
- `ARG_PR` — first positional integer (PR number); if omitted, derive via `gh pr view --json number -q .number`.
- `ARG_RESUME` — `true` if `--resume` is passed.
- `ARG_SURFACES` — comma list after `--surfaces=`.
- `ARG_TIER_ONLY` — `true` if `--tier-only` is passed.
- `ARG_READ_ONLY` — `true` if `--read-only` is passed (skip gh pr comment, skip writing recap.md, write all output to `/tmp/master-review-readonly-<pr>.md` only).
- `ARG_CONFIG_PATH` — path after `--config=`.

Export all `ARG_*` to env so the bash blocks below can read them.

---

## Step 0 — PR analysis (project-aware preamble)

Step 0 is the wrapper that makes this command different from a vanilla `/review`. It runs **before** any Sonnet agent. Six sub-steps in order: 0f (saturation gate) → 0a (distance from main) → 0c (config parsing) → 0b (tier scoring) → 0e (resume detection) → 0d (dispatch decision).

### Step 0f — Saturation gate

Run this **first**. If the current session JSONL exceeds 800 lines, do NOT auto-abort but request user confirmation before proceeding (saturated sessions historically produce non-deterministic blindspots).

```bash
# Step 0f — Saturation gate
STASH=/tmp/master-review-step0.env
: > "$STASH"
# Self-describe the stash path so subsequent bash blocks can recover it
# after sourcing $STASH (each Bash tool call is a fresh subshell — env
# vars set in this block do not persist across invocations).
printf 'STASH=/tmp/master-review-step0.env\n' >> "$STASH"

JSONL_DIR="${CLAUDE_PROJECTS_DIR:-$HOME/.claude/projects/-workspace}"
JSONL=$(ls -t "$JSONL_DIR"/*.jsonl 2>/dev/null | head -1)

if [ -z "$JSONL" ] || [ ! -f "$JSONL" ]; then
    printf 'JSONL_LINES=0\nSATURATED=false\n' >> "$STASH"
else
    LINES=$(wc -l < "$JSONL" | tr -d ' ')
    printf 'JSONL_LINES=%s\n' "$LINES" >> "$STASH"
    if [ "$LINES" -gt 800 ]; then
        printf 'SATURATED=true\n' >> "$STASH"
        cat <<EOF
> [!WARNING]
> **Session saturated (${LINES} JSONL lines, threshold 800).**
>
> A T3+ multi-agent review in a saturated session has historically produced
> non-deterministic blindspots (cache misses, context drops, panic-cascade).
>
> **Recommended:** open a fresh session, then re-invoke this command. A kickoff
> prompt for the new session will be generated automatically (see Step 8).
>
> **To proceed anyway in this session**, reply: \`continue saturated\`.
> Any other reply aborts the review.
EOF
        exit 0
    else
        printf 'SATURATED=false\n' >> "$STASH"
    fi
fi
```

### Step 0a — Distance from main

Detects three failure modes that cost tokens before any agent runs: (1) branch behind main, (2) leakage (files appearing in 2-dot diff that vanish in 3-dot diff = parallel-branch merge contamination), (3) main merged into the branch (vs rebased). Never auto-rebases.

```bash
. /tmp/master-review-step0.env

BASE_REMOTE=origin
BASE_REF=
PR_BASE_REF_NAME=
if [ -n "$ARG_PR" ]; then
    PR_BASE_REF_NAME=$(gh pr view "$ARG_PR" --json baseRefName -q .baseRefName 2>/dev/null)
fi
if [ -z "$PR_BASE_REF_NAME" ]; then
    PR_BASE_REF_NAME=$(gh pr view --json baseRefName -q .baseRefName 2>/dev/null)
fi
if [ -n "$PR_BASE_REF_NAME" ] && git ls-remote --exit-code --heads "$BASE_REMOTE" "$PR_BASE_REF_NAME" >/dev/null 2>&1; then
    BASE_REF="$BASE_REMOTE/$PR_BASE_REF_NAME"
elif git ls-remote --exit-code --heads "$BASE_REMOTE" main >/dev/null 2>&1; then
    BASE_REF="$BASE_REMOTE/main"
elif git ls-remote --exit-code --heads "$BASE_REMOTE" master >/dev/null 2>&1; then
    BASE_REF="$BASE_REMOTE/master"
else
    printf 'STEP0A_DECISION=STOP_NO_BASE\n' >> "$STASH"
    printf '> [!WARNING]\n> Cannot resolve a base ref (no --pr arg, no origin/main, no origin/master). Aborting.\n'
    exit 1
fi
git fetch "$BASE_REMOTE" "${BASE_REF#$BASE_REMOTE/}" --quiet

MERGE_BASE=$(git merge-base HEAD "$BASE_REF")
BEHIND=$(git rev-list --count HEAD.."$BASE_REF")
AHEAD=$(git rev-list --count "$BASE_REF"..HEAD)
LAST_COMMON_REL=$(git log -1 --format=%cr "$MERGE_BASE")
LAST_COMMON_DAYS=$(( ( $(date +%s) - $(git log -1 --format=%ct "$MERGE_BASE") ) / 86400 ))

FILES_3DOT=$(git diff "$BASE_REF...HEAD" --name-only | wc -l | tr -d ' ')
FILES_2DOT=$(git diff "$BASE_REF..HEAD" --name-only | wc -l | tr -d ' ')
LEAKAGE=$(( FILES_2DOT - FILES_3DOT ))
MERGES_INTO_BRANCH=$(git log --merges "$BASE_REF..HEAD" --oneline 2>/dev/null | wc -l | tr -d ' ')

DECISION=
if [ "$MERGES_INTO_BRANCH" -gt 0 ]; then
    DECISION=STOP_MERGED_IN
elif [ "$LEAKAGE" -gt 5 ]; then
    DECISION=STOP_LEAKAGE
elif [ "$BEHIND" -eq 0 ]; then
    DECISION=OK_PROCEED
elif [ "$BEHIND" -le 5 ] && [ "$LAST_COMMON_DAYS" -lt 3 ]; then
    DECISION=OK_MENTION
elif [ "$BEHIND" -gt 5 ] || [ "$LAST_COMMON_DAYS" -gt 7 ]; then
    DECISION=STOP_REBASE
else
    DECISION=OK_MENTION
fi

{
    printf 'BASE_REF=%s\n'           "$BASE_REF"
    printf 'BEHIND=%s\n'             "$BEHIND"
    printf 'AHEAD=%s\n'              "$AHEAD"
    printf "LAST_COMMON_REL='%s'\n"  "$LAST_COMMON_REL"
    printf 'LAST_COMMON_DAYS=%s\n'   "$LAST_COMMON_DAYS"
    printf 'FILES_3DOT=%s\n'         "$FILES_3DOT"
    printf 'FILES_2DOT=%s\n'         "$FILES_2DOT"
    printf 'LEAKAGE=%s\n'            "$LEAKAGE"
    printf 'MERGES_INTO_BRANCH=%s\n' "$MERGES_INTO_BRANCH"
    printf 'STEP0A_DECISION=%s\n'    "$DECISION"
} >> "$STASH"

case "$DECISION" in
OK_PROCEED) printf 'Branch up to date with %s. Proceeding.\n' "$BASE_REF" ;;
OK_MENTION) printf 'Branch is %s commits behind %s (last common: %s). Proceeding, will mention in review.\n' "$BEHIND" "$BASE_REF" "$LAST_COMMON_REL" ;;
STOP_REBASE)
    EST_TOKENS=$(( (FILES_2DOT - FILES_3DOT > 0 ? FILES_2DOT - FILES_3DOT : BEHIND) * 200 ))
    cat <<EOF
> [!WARNING]
> **Branch behind ${BASE_REF}: ${BEHIND} commits, last common ancestor ${LAST_COMMON_REL}.**
>
> Recommended before review:
> \`\`\`
> git fetch ${BASE_REMOTE} && git rebase ${BASE_REF}
> \`\`\`
>
> Estimated token saving: ~15-30% (stale base inflates diff by ${BEHIND} commits' worth of churn, ~${EST_TOKENS} tokens).
>
> Reply \`rebased\` after rebasing, or \`force\` to review against the stale base anyway.
EOF
    exit 0
    ;;
STOP_LEAKAGE)
    cat <<EOF
> [!WARNING]
> **Leakage detected: ${FILES_2DOT} files in 2-dot diff vs ${FILES_3DOT} in 3-dot diff (delta ${LEAKAGE}).**
>
> Symptom: a parallel branch was merged into ${BASE_REF} between this branch's
> branch-point and now, and its files appear in your diff although they don't
> belong to this PR. A rebase dissolves the apparent scope creep.
>
> Recommended:
> \`\`\`
> git fetch ${BASE_REMOTE} && git rebase ${BASE_REF}
> \`\`\`
>
> Reply \`leaked-files\` to list the offending paths, or \`rebased\` after rebasing.
EOF
    exit 0
    ;;
STOP_MERGED_IN)
    cat <<EOF
> [!WARNING]
> **${BASE_REF#$BASE_REMOTE/} has been merged INTO this branch (${MERGES_INTO_BRANCH} merge commits found).**
>
> A blind rebase will likely conflict. Recommended:
> \`\`\`
> git rebase -i --onto ${BASE_REF} <branch-point>
> \`\`\`
> or human consultation.
>
> Reply \`force\` to review the branch as-is (expect noisy diff).
EOF
    exit 0
    ;;
esac
```

### Step 0c — Config parsing

Resolves `review-config.md` through a 4-path fallback (--config flag, `.devcontainer/claude/`, `.claude/`, repo root), parses each section into env stash + per-agent prompt files. Pure bash + awk; no jq for markdown.

```bash
. /tmp/master-review-step0.env

CONFIG_STASH=/tmp/master-review-config.env
: > "$CONFIG_STASH"
rm -f /tmp/master-review-agent-*.prompt 2>/dev/null

CONFIG_PATH=
for candidate in \
    "$ARG_CONFIG_PATH" \
    ".devcontainer/skills/master-review.local/review-config.md" \
    ".devcontainer/claude/review-config.md" \
    ".claude/review-config.md" \
    "review-config.md"
do
    [ -z "$candidate" ] && continue
    if [ -f "$candidate" ]; then
        CONFIG_PATH="$candidate"
        break
    fi
done

if [ -z "$CONFIG_PATH" ]; then
    SENTINEL=".devcontainer/skills/master-review.local/.skip-bootstrap"
    if [ -f "$SENTINEL" ]; then
        printf 'CONFIG_MODE=vanilla\nCONFIG_PATH=\n' >> "$CONFIG_STASH"
        printf 'note: vanilla mode (bootstrap suppressed by %s — rm to re-enable).\n' "$SENTINEL"
        return 0 2>/dev/null || true
    fi
    printf 'CONFIG_MODE=needs-bootstrap\nCONFIG_PATH=\n' >> "$CONFIG_STASH"
    return 0 2>/dev/null || true
fi
printf 'CONFIG_MODE=custom\nCONFIG_PATH=%s\n' "$CONFIG_PATH" >> "$CONFIG_STASH"

extract_section() {
    local heading="${1}" file="${2}"
    awk -v F0=0 -v h="## $heading" '
        $F0 == h       { inside = 1; next }
        /^## /         { if (inside) exit }
        inside         { print }
    ' "$file"
}

parse_table() {
    awk -v F0=0 '
        /^\|/ && !/^\| *-+/ {
            line = $F0
            sub(/^\| */, "", line); sub(/ *\| *$/, "", line)
            # Protect markdown-escaped pipes \| from the column splitter
            gsub(/\\\|/, "\001", line)
            n = split(line, cells, / *\| */)
            for (i = 1; i <= n; i++) gsub(/\001/, "|", cells[i])
            if (header_seen) {
                out = cells[1]
                for (i = 2; i <= n; i++) out = out "\t" cells[i]
                print out
            } else {
                header_seen = 1
            }
        }
    '
}

# --- Project Meta ---
META=$(extract_section "Project Meta" "$CONFIG_PATH")
if [ -n "$META" ]; then
    NAME=$(printf '%s\n' "$META" | awk '/^- \*\*Name\*\*: */     {sub(/^- \*\*Name\*\*: */,""); print; exit}')
    CONV_DOC=$(printf '%s\n' "$META" | awk '/^- \*\*Conventions doc\*\*/ {sub(/^- \*\*Conventions doc\*\*: */,""); print; exit}')
    DEV_DOC=$(printf '%s\n' "$META"  | awk '/^- \*\*Dev doc\*\*/         {sub(/^- \*\*Dev doc\*\*: */,""); print; exit}')
    # Use %q (bash builtin shell-escape) so values containing spaces or
    # quotes survive `. /tmp/master-review-config.env` sourcing intact.
    # Example: `Portal42 POS` → `Portal42\ POS` (instead of being split
    # into PROJECT_NAME=Portal42 + a stray `POS` command lookup).
    {
        printf 'PROJECT_NAME=%q\n'     "${NAME:-unknown}"
        printf 'CONVENTIONS_DOC=%q\n'  "$CONV_DOC"
        printf 'DEV_DOC=%q\n'          "$DEV_DOC"
    } >> "$CONFIG_STASH"
fi

# --- Tier Scoring Overrides ---
TIER_FILE=/tmp/master-review-tier-weights.tsv
TIER_BLOCKING_FILE=/tmp/master-review-tier-blocking.tsv
: > "$TIER_FILE"; : > "$TIER_BLOCKING_FILE"
TIER_TABLE=$(extract_section "Tier Scoring Overrides" "$CONFIG_PATH")
if [ -n "$TIER_TABLE" ]; then
    printf '%s\n' "$TIER_TABLE" | parse_table | while IFS=$'\t' read -r signal weight rest; do
        [ -z "$signal" ] && continue
        if printf '%s' "$weight" | grep -qi 'BLOCKING'; then
            printf '%s\t%s\n' "$signal" "$weight" >> "$TIER_BLOCKING_FILE"
        else
            num=$(printf '%s' "$weight" | grep -oE '[-+]?[0-9]+' | head -1)
            [ -n "$num" ] && printf '%s\t%s\n' "$signal" "$num" >> "$TIER_FILE"
        fi
    done
fi

# --- Surfaces ---
SURFACES_FILE=/tmp/master-review-surfaces.tsv
: > "$SURFACES_FILE"
SURF_TABLE=$(extract_section "Surfaces" "$CONFIG_PATH")
if [ -n "$SURF_TABLE" ]; then
    printf '%s\n' "$SURF_TABLE" | parse_table > "$SURFACES_FILE"
fi

# --- Custom Agents ---
# Walk the agents/ subdir colocated with the skill (one file per agent,
# frontmatter + body). Falls back to the dir next to $CONFIG_PATH for
# projects that ship review-config.md outside the skill folder.
AGENTS_INDEX=/tmp/master-review-agents.tsv
: > "$AGENTS_INDEX"
AGENTS_DIR=
for cand in \
    ".devcontainer/skills/master-review.local/agents" \
    "$(dirname "$CONFIG_PATH")/agents"
do
    if [ -d "$cand" ]; then AGENTS_DIR="$cand"; break; fi
done
if [ -n "$AGENTS_DIR" ]; then
    for f in "$AGENTS_DIR"/agent-*.md; do
        [ -f "$f" ] || continue
        id=$(awk '/^id:/{sub(/^id: */,""); print; exit}' "$f")
        trigger=$(awk '/^trigger:/{sub(/^trigger: */,""); print; exit}' "$f")
        tools=$(awk '/^tools:/{sub(/^tools: */,""); print; exit}' "$f")
        if [ -z "$id" ]; then
            printf 'warning: %s missing id — skipped\n' "$f" >&2
            continue
        fi
        promptfile="/tmp/master-review-agent-${id}.prompt"
        # Body = lines after the second `---`, with leading blank line(s) skipped.
        # `-v F0=0` indirection so the slash-command harness pre-pass does not
        # substitute `$0` with $ARGUMENTS positional 0 (PR number) — this was
        # the S7B class of bug that S7I caught hiding here.
        awk -v F0=0 '
            BEGIN { n = 0; started = 0 }
            /^---$/ && n < 2 { n++; next }
            n == 2 {
                if (!started && $F0 ~ /^[[:space:]]*$/) next
                started = 1
                print
            }
        ' "$f" > "$promptfile"
        printf '%s\t%s\t%s\t%s\n' "$id" "$trigger" "$tools" "$promptfile" >> "$AGENTS_INDEX"
    done
fi

# --- Tactical Framings ---
FRAMINGS_FILE=/tmp/master-review-framings.tsv
: > "$FRAMINGS_FILE"
FRAM_TABLE=$(extract_section "Tactical Framings" "$CONFIG_PATH")
[ -n "$FRAM_TABLE" ] && printf '%s\n' "$FRAM_TABLE" | parse_table > "$FRAMINGS_FILE"

# --- Output Paths ---
OUT_PATHS=$(extract_section "Output Paths" "$CONFIG_PATH")
if [ -n "$OUT_PATHS" ]; then
    printf '%s\n' "$OUT_PATHS" | awk -v F0=0 '
        /^- *Round file:/        { sub(/^- *Round file: */, "");        printf "OUTPUT_ROUND_FILE=%s\n", $F0 }
        /^- *Recap file:/        { sub(/^- *Recap file: */, "");        printf "OUTPUT_RECAP_FILE=%s\n", $F0 }
        /^- *Surfaces matrix:/   { sub(/^- *Surfaces matrix: */, "");   printf "OUTPUT_SURFACES_FILE=%s\n", $F0 }
        /^- *gh pr comment:/     { sub(/^- *gh pr comment: */, "");     printf "OUTPUT_GH_COMMENT=%s\n", $F0 }
    ' >> "$CONFIG_STASH"
fi
grep -q '^OUTPUT_ROUND_FILE='    "$CONFIG_STASH" || printf 'OUTPUT_ROUND_FILE=PR-${PR}-review.md\n'    >> "$CONFIG_STASH"
grep -q '^OUTPUT_RECAP_FILE='    "$CONFIG_STASH" || printf 'OUTPUT_RECAP_FILE=PR-${PR}-recap.md\n'     >> "$CONFIG_STASH"
grep -q '^OUTPUT_SURFACES_FILE=' "$CONFIG_STASH" || printf 'OUTPUT_SURFACES_FILE=PR-${PR}-surfaces.md\n' >> "$CONFIG_STASH"
grep -q '^OUTPUT_GH_COMMENT='    "$CONFIG_STASH" || printf 'OUTPUT_GH_COMMENT=enabled\n'               >> "$CONFIG_STASH"

# --- Commit Hygiene Regex ---
COMMIT_REGEX_FILE=/tmp/master-review-commit-regex.txt
: > "$COMMIT_REGEX_FILE"
COMMIT_RX=$(extract_section "Commit Hygiene Regex" "$CONFIG_PATH")
if [ -n "$COMMIT_RX" ]; then
    printf '%s\n' "$COMMIT_RX" | awk '/^- *`/ {sub(/^- *`/,""); sub(/`.*$/,""); print}' > "$COMMIT_REGEX_FILE"
else
    cat <<'DEFAULTS' > "$COMMIT_REGEX_FILE"
(?i)round\s*\d+
(?i)R\d+-\d+
(?i)review\s+(fix(es)?|polish|done|round)
(?i)PR-?\d+-(review|recap)
DEFAULTS
fi

# --- Special-case Files ---
BLOCK_FILES=/tmp/master-review-block-files.tsv
: > "$BLOCK_FILES"
SPECIAL=$(extract_section "Special-case Files" "$CONFIG_PATH")
if [ -n "$SPECIAL" ]; then
    printf '%s\n' "$SPECIAL" | awk -v F0=0 '
        /^- *`[^`]+`/ {
            line = $F0
            match(line, /`[^`]+`/); path = substr(line, RSTART+1, RLENGTH-2)
            reason = line; sub(/^[^)]*\(/, "", reason); sub(/\).*$/, "", reason)
            block = (line ~ /[Bb]lock review/) ? "BLOCK" : "WARN"
            printf "%s\t%s\t%s\n", path, block, reason
        }
    ' > "$BLOCK_FILES"
fi

# --- Override Threshold ---
OV=$(extract_section "Override Threshold" "$CONFIG_PATH")
if [ -n "$OV" ]; then
    printf '%s\n' "$OV" | awk -v F0=0 '
        /^- *Default *[0-9]+/ { match($F0, /[0-9]+/); printf "THRESHOLD_DEFAULT=%s\n", substr($F0, RSTART, RLENGTH); next }
        /^- *[0-9]+ *for agents? +/ {
            match($F0, /[0-9]+/); thr = substr($F0, RSTART, RLENGTH)
            sub(/^[^a-zA-Z]+for agents? +/, "")
            gsub(/[^0-9,-]/, "")
            printf "THRESHOLD_AGENTS_%s=%s\n", $F0, thr
        }
    ' >> "$CONFIG_STASH"
fi
grep -q '^THRESHOLD_DEFAULT=' "$CONFIG_STASH" || printf 'THRESHOLD_DEFAULT=80\n' >> "$CONFIG_STASH"

# --- GitHub Review Threads ---
GH_THREADS=$(extract_section "GitHub Review Threads" "$CONFIG_PATH")
GH_THREADS_ENABLED=true
if [ -n "$GH_THREADS" ]; then
    case "$GH_THREADS" in *"enabled: false"*) GH_THREADS_ENABLED=false;; esac
fi
printf 'GH_THREADS_ENABLED=%s\n' "$GH_THREADS_ENABLED" >> "$CONFIG_STASH"
```

After parsing, launch a Haiku **validator** agent that reads the config file + the parser warnings (stderr) + the list of detected sections, and emits strict JSON of shape `{status, warnings, missing_required, skipped_agents, summary}`. If `status=fatal` and `--force` was not passed, abort. Otherwise print the summary line and continue.

### Step 0c.5 — Interactive bootstrap (when CONFIG_MODE=needs-bootstrap)

If Step 0c stamped `CONFIG_MODE=needs-bootstrap` (no `review-config.md` was found in any lookup path AND no `.skip-bootstrap` sentinel exists), this step asks the user whether they want to bootstrap a starter config in 5 questions, fall through to vanilla mode, or skip-and-remember (write a sentinel so the prompt never re-fires).

```bash
. /tmp/master-review-step0.env
. /tmp/master-review-config.env

if [ "$CONFIG_MODE" != "needs-bootstrap" ]; then
    return 0 2>/dev/null || true
fi
```

**Then ask via `AskUserQuestion`**:

> **Question**: "No `review-config.md` found. Bootstrap one in 5 questions?"
>
> **Options**:
> - `Yes — 5 questions` → run the Q&A, write a starter config, exit (no review this run).
> - `No — vanilla this run` → fall through to vanilla mode for this session only (next run will prompt again).
> - `Skip and remember` → write `.devcontainer/skills/master-review.local/.skip-bootstrap`, fall through to vanilla mode permanently (until the user removes the sentinel).

**Branch handling**:

- **Yes** branch: ask the 5 questions one by one (one `AskUserQuestion` per turn — do NOT batch). Then write the starter config to the most-specific writable path:

  ```bash
  TARGET=
  for cand in \
      ".devcontainer/skills/master-review.local/review-config.md" \
      ".devcontainer/claude/review-config.md" \
      "review-config.md"
  do
      parent=$(dirname "$cand")
      if [ -d "$parent" ] || [ "$parent" = "." ]; then
          TARGET="$cand"; break
      fi
  done
  mkdir -p "$(dirname "$TARGET")"
  ```

  Then write the file content (template below, with Q1-Q5 substituted), print:

  > review-config.md generated at `<TARGET>`. Surfaces are a commented placeholder — copy from `.devcontainer/skills/master-review.local/review-config.example.md` (the shipped reference) and adapt to your codebase. To wire up custom agents, see the `agents/` subdir colocated with the skill (`agents/agent-NN-<slug>.md`, frontmatter + body) and the `## Custom Agents` pointer paragraph in the example. Re-run `/master-review` when ready.

  Then **exit** (don't proceed to Step 0b — let the user finalize their config first).

- **No** branch: stamp `CONFIG_MODE=vanilla` in the stash and continue:
  ```bash
  sed -i 's/^CONFIG_MODE=needs-bootstrap$/CONFIG_MODE=vanilla/' "$CONFIG_STASH"
  printf 'note: continuing in vanilla mode for this session (generic agents 1-5 only).\n'
  ```

- **Skip and remember** branch:
  ```bash
  mkdir -p .devcontainer/skills/master-review
  touch .devcontainer/skills/master-review.local/.skip-bootstrap
  sed -i 's/^CONFIG_MODE=needs-bootstrap$/CONFIG_MODE=vanilla/' "$CONFIG_STASH"
  printf 'sentinel written: .devcontainer/skills/master-review.local/.skip-bootstrap (rm to re-enable bootstrap prompts later).\n'
  ```

**The 5 questions** (one `AskUserQuestion` per turn):

| # | Header | Question | Format |
|---|---|---|---|
| Q1 | `Stack` | "Primary stack? Pick one or 'Other' to free-form." | multipleChoice: `PHP`, `Node.js`, `Python`, `Go`, `Ruby`, `Other` |
| Q2 | `Critical paths` | "Comma-separated paths that need extra-careful review (e.g. `api/payment, auth/, migrations/`). Press enter for none." | freeText |
| Q3 | `Conventions doc` | "Path to your conventions doc (project's CLAUDE.md or equivalent). Default: `CLAUDE.md`." | freeText, default `CLAUDE.md` |
| Q4 | `Output paths` | "Where should `PR-N-review.md`/`PR-N-recap.md`/`PR-N-surfaces.md` be written? Default: repo root." | freeText, default `.` |
| Q5 | `Threshold` | "Confidence threshold (0-100). 75 = more findings (calibrated for novel projects), 80 = balanced default, 90 = very conservative." | freeText, default `80` |

If the user picks `Other` for Q1, immediately ask a follow-up `AskUserQuestion` (free text) for the stack name.

**Starter config template** (substitute `{{Q1}}`..`{{Q5}}` from answers):

```markdown
## Project Meta

- **Name**: {{Q1}} project
- **Stack**: {{Q1}}
- **Conventions doc**: {{Q3}}
- **Critical paths**: {{Q2}}

## Tier Scoring Overrides

<!-- TODO: customize for your stack — see review-config.example.md for the Portal42 reference. -->
<!-- Format: markdown table with columns | Signal (regex on file paths) | Weight (+N or "BLOCKING — abort review") | -->

## Surfaces

<!-- TODO: copy the surface matrix (A/B/C/D/E/F/G…) from review-config.example.md and adapt to your codebase. -->
<!-- Format: markdown table with columns | ID | Surface | Trigger patterns | Agent assigned | -->

## Custom Agents

<!-- Custom agents live one-per-file in `agents/` next to this config. -->
<!-- Each agent is a markdown file with YAML frontmatter (id, name, trigger, tools) + body. -->
<!-- See review-config.example.md and agents/README.md for the format. -->
<!-- To add a new agent, copy an existing agents/agent-NN-<slug>.md and adapt it. -->

## Tactical Framings

| ID | Framing | Target surfaces | Used by agents |
|---|---|---|---|
| F1 | Imagine the fatal/OOM case: function dies mid-write, transaction half-committed, signal fires during shutdown. What's the post-mortem at 3am? | Lifecycle / atomicity | Custom |
| F2 | Generalize the bug. Where else in the codebase is the SAME pattern (acquired-state-without-try-finally) sitting unflagged? | All surfaces | All adversarial agents |
| F3 | Gap analysis: list the inputs the agent could affect, then for each input ask "is this validated, escaped, bounded?". The first NO is a finding. | Security / input handling | #6 (security) |
| F4 | Diff-exact framing: don't analyze the codebase, analyze the DIFF. What did this PR add that wasn't there yesterday, and what could go wrong with that addition specifically? | All surfaces | All agents |
| F5 | Timing/ordering: what if step A runs after step B? What if step B fires twice? What if both fire concurrently? | Lifecycle / atomicity / async | #7 (lifecycle) |

## Output Paths

- Round file: {{Q4}}/PR-${PR}-review.md
- Recap file: {{Q4}}/PR-${PR}-recap.md
- Surfaces matrix: {{Q4}}/PR-${PR}-surfaces.md
- gh pr comment: enabled

## Commit Hygiene Regex

- `(?i)round\s*\d+`
- `(?i)R\d+-\d+`
- `(?i)review\s+(fix(es)?|polish|done|round)`
- `(?i)PR-?\d+-(review|recap)`

## Override Threshold

- Default {{Q5}}
```

After writing the starter config, exit (the user has work to do customizing it).

### Step 0b — Tier scoring

Pulls PR metadata via `gh pr view`, applies default weights, then layers config overrides additively. If any blocking trigger from config matches the diff, aborts with a human-review request.

```bash
. /tmp/master-review-step0.env
. /tmp/master-review-config.env

PR="${ARG_PR}"
if [ -z "$PR" ]; then
    PR=$(gh pr view --json number -q .number 2>/dev/null)
fi
if [ -z "$PR" ]; then
    printf '> [!WARNING]\n> No PR number provided and no PR associated with current branch.\n'
    exit 1
fi
printf 'PR=%s\n' "$PR" >> "$STASH"

PR_JSON=$(gh pr view "$PR" --json files,additions,deletions,changedFiles,headRefName,baseRefName,headRefOid)
ADDITIONS=$(printf '%s' "$PR_JSON" | jq -r '.additions')
DELETIONS=$(printf '%s' "$PR_JSON" | jq -r '.deletions')
CHANGED=$(printf '%s' "$PR_JSON" | jq -r '.changedFiles')
HEAD_REF=$(printf '%s' "$PR_JSON" | jq -r '.headRefName')
BASE_REF_PR=$(printf '%s' "$PR_JSON" | jq -r '.baseRefName')
HEAD_OID=$(printf '%s' "$PR_JSON" | jq -r '.headRefOid')
PR_FILES=$(printf '%s' "$PR_JSON" | jq -r '.files[].path')

{
    printf 'PR_ADDITIONS=%s\n' "$ADDITIONS"
    printf 'PR_DELETIONS=%s\n' "$DELETIONS"
    printf 'PR_CHANGED=%s\n'   "$CHANGED"
    printf 'PR_HEAD_REF=%s\n'  "$HEAD_REF"
    printf 'PR_BASE_REF=%s\n'  "$BASE_REF_PR"
    printf 'PR_HEAD_OID=%s\n'  "$HEAD_OID"
} >> "$STASH"

if [ -s /tmp/master-review-tier-blocking.tsv ]; then
    while IFS=$'\t' read -r signal weight; do
        if printf '%s\n' "$PR_FILES" | grep -Eq "$signal" 2>/dev/null; then
            cat <<EOF
> [!WARNING]
> **Blocking trigger matched: \`${signal}\`**
>
> Config (\`${CONFIG_PATH}\`) marks this signal as **BLOCKING — abort review**.
> Diff touching these files requires explicit human review.
>
> Reply \`force\` to override (the trigger reason is recorded in the recap).
EOF
            printf 'STEP0B_BLOCKING=%s\n' "$signal" >> "$STASH"
            exit 0
        fi
    done < /tmp/master-review-tier-blocking.tsv
fi

score=0
total_lines=$(( ADDITIONS + DELETIONS ))
if   [ "$ADDITIONS" -gt 1500 ] || [ "$CHANGED" -gt 15 ]; then score=$(( score + 2 ))
elif [ "$ADDITIONS" -gt 500  ] || [ "$CHANGED" -gt 8  ]; then score=$(( score + 1 ))
fi
non_doc=$(printf '%s\n' "$PR_FILES" | grep -Ev '\.(md|txt|json)$|^assets/' | wc -l | tr -d ' ')
[ "$non_doc" -eq 0 ] && [ "$CHANGED" -gt 0 ] && score=$(( score - 2 ))
[ "$CHANGED" -eq 1 ] && score=$(( score - 1 ))

if [ -s /tmp/master-review-tier-weights.tsv ]; then
    while IFS=$'\t' read -r signal weight; do
        if printf '%s\n' "$PR_FILES" | grep -Eq "$signal" 2>/dev/null; then
            score=$(( score + weight ))
        fi
    done < /tmp/master-review-tier-weights.tsv
fi

if   [ "$score" -le 0 ]; then TIER=T1
elif [ "$score" -le 3 ]; then TIER=T2
elif [ "$score" -le 6 ]; then TIER=T3
elif [ "$score" -le 9 ]; then TIER=T4
else                          TIER=T4plus
fi

SHOULD_REBASE=false
case "$STEP0A_DECISION" in
    STOP_REBASE|STOP_LEAKAGE|STOP_MERGED_IN) SHOULD_REBASE=true ;;
esac

SURFACES_REQUIRED=
if [ -s /tmp/master-review-surfaces.tsv ] && [ "$TIER" != "T1" ] && [ "$TIER" != "T2" ]; then
    SURFACES_REQUIRED=$(cut -f1 /tmp/master-review-surfaces.tsv | paste -sd, -)
fi

{
    printf 'TIER_SCORE=%s\n'         "$score"
    printf 'TIER=%s\n'               "$TIER"
    printf 'SHOULD_REBASE=%s\n'      "$SHOULD_REBASE"
    printf 'SURFACES_REQUIRED=%s\n'  "$SURFACES_REQUIRED"
} >> "$STASH"

printf 'PR #%s tiered as **%s** (score %s, +%s/-%s in %s files).\n' \
    "$PR" "$TIER" "$score" "$ADDITIONS" "$DELETIONS" "$CHANGED"
```

### Step 0e — Resume detection

```bash
. /tmp/master-review-step0.env
. /tmp/master-review-config.env

RECAP_PATH=$(printf '%s' "$OUTPUT_RECAP_FILE" | sed "s/\${PR}/$PR/g")

RESUME_AVAILABLE=false
SURFACES_COVERED=
SURFACES_REMAINING=

if [ -f "$RECAP_PATH" ]; then
    COVERED=$(awk -v F0=0 '
        /^## (Surfaces couvertes|Surfaces covered)/ { in_block = 1; next }
        /^## /                                       { in_block = 0 }
        in_block && /^- *\[[xX]\]/                   {
            match($F0, /\[[xX]\] *[A-Z]/)
            if (RSTART) {
                s = substr($F0, RSTART, RLENGTH)
                gsub(/[^A-Z]/, "", s)
                printf "%s\n", s
            }
        }
        in_block && /\[checked\]/ {
            match($F0, /^- *[A-Z]\b/)
            if (RSTART) {
                s = substr($F0, RSTART, RLENGTH)
                gsub(/[^A-Z]/, "", s)
                printf "%s\n", s
            }
        }
    ' "$RECAP_PATH" | sort -u | paste -sd, -)

    if [ -n "$COVERED" ]; then
        RESUME_AVAILABLE=true
        SURFACES_COVERED="$COVERED"
        SURFACES_REMAINING=$(printf '%s' "$SURFACES_REQUIRED" | tr ',' '\n' \
            | grep -vxF -f <(printf '%s' "$COVERED" | tr ',' '\n') \
            | paste -sd, -)
    fi
fi

{
    printf 'RECAP_PATH=%s\n'           "$RECAP_PATH"
    printf 'RESUME_AVAILABLE=%s\n'     "$RESUME_AVAILABLE"
    printf 'SURFACES_COVERED=%s\n'     "$SURFACES_COVERED"
    printf 'SURFACES_REMAINING=%s\n'   "$SURFACES_REMAINING"
} >> "$STASH"

if [ "$ARG_RESUME" = "true" ] && [ "$RESUME_AVAILABLE" = "false" ]; then
    cat <<EOF
> [!WARNING]
> \`--resume\` was passed but no usable recap was found at \`${RECAP_PATH}\`.
> Falling back to a full review.
EOF
    printf 'RESUME_FALLBACK=true\n' >> "$STASH"
fi
```

### Step 0d — Dispatch decision

```bash
. /tmp/master-review-step0.env
. /tmp/master-review-config.env

GENERICS=
CUSTOMS=
DISPATCH_MODE=fresh
[ "$ARG_RESUME" = "true" ] && [ "$RESUME_AVAILABLE" = "true" ] && DISPATCH_MODE=resume

if [ "$DISPATCH_MODE" = "fresh" ]; then
    case "$TIER" in
        T1)            GENERICS="1,2" ;;
        T2)            GENERICS="1,2,3" ;;
        T3|T4|T4plus)  GENERICS="1,2,3,4,5" ;;
    esac
fi

if [ "$GH_THREADS_ENABLED" = "false" ] && [ -n "$GENERICS" ]; then
    GENERICS=$(printf '%s' "$GENERICS" | tr ',' '\n' | grep -v '^4$' | paste -sd, -)
fi

tier_rank() { case "${1}" in T1)echo 1;;T2)echo 2;;T3)echo 3;;T4)echo 4;;T4plus)echo 5;;*)echo 0;;esac; }
CUR_RANK=$(tier_rank "$TIER")

if [ -s /tmp/master-review-agents.tsv ]; then
    while IFS=$'\t' read -r id trigger tools promptfile; do
        req=$(printf '%s' "$trigger" | grep -oE 'T[1-4]\+?' | head -1)
        case "$req" in T4+) req=T4plus;; esac
        REQ_RANK=$(tier_rank "${req:-T1}")
        if [ "$CUR_RANK" -ge "$REQ_RANK" ]; then
            CUSTOMS="${CUSTOMS:+$CUSTOMS,}$id"
        fi
    done < /tmp/master-review-agents.tsv
fi

# --surfaces filter
if [ -n "$ARG_SURFACES" ] && [ -n "$CUSTOMS" ] && [ -s /tmp/master-review-surfaces.tsv ]; then
    KEEP=
    for sid in $(printf '%s' "$ARG_SURFACES" | tr ',' ' '); do
        ag=$(awk -F'\t' -v F1=1 -v F4=4 -v s="$sid" '$F1==s {print $F4}' /tmp/master-review-surfaces.tsv | grep -oE '#[0-9]+' | tr -d '#')
        KEEP="${KEEP:+$KEEP }$ag"
    done
    NEW=
    for c in $(printf '%s' "$CUSTOMS" | tr ',' ' '); do
        printf '%s\n' "$KEEP" | tr ' ' '\n' | grep -qx "$c" && NEW="${NEW:+$NEW,}$c"
    done
    CUSTOMS="$NEW"
fi

# --resume narrows to remaining surfaces
if [ "$DISPATCH_MODE" = "resume" ] && [ -n "$SURFACES_REMAINING" ] && [ -s /tmp/master-review-surfaces.tsv ]; then
    KEEP=
    for sid in $(printf '%s' "$SURFACES_REMAINING" | tr ',' ' '); do
        ag=$(awk -F'\t' -v F1=1 -v F4=4 -v s="$sid" '$F1==s {print $F4}' /tmp/master-review-surfaces.tsv | grep -oE '#[0-9]+' | tr -d '#')
        KEEP="${KEEP:+$KEEP }$ag"
    done
    NEW=
    for c in $(printf '%s' "$CUSTOMS" | tr ',' ' '); do
        printf '%s\n' "$KEEP" | tr ' ' '\n' | grep -qx "$c" && NEW="${NEW:+$NEW,}$c"
    done
    CUSTOMS="$NEW"
fi

if [ "$ARG_TIER_ONLY" = "true" ]; then
    cat <<EOF
**Tier:** ${TIER} (score ${TIER_SCORE})
**Would launch generics:** ${GENERICS:-none}
**Would launch customs:**  ${CUSTOMS:-none}
**Surfaces required:**     ${SURFACES_REQUIRED:-none}
**Surfaces covered:**      ${SURFACES_COVERED:-none}
**Surfaces remaining:**    ${SURFACES_REMAINING:-${SURFACES_REQUIRED:-none}}
EOF
    exit 0
fi

{
    printf 'LAUNCH_GENERICS=%s\n' "$GENERICS"
    printf 'LAUNCH_CUSTOMS=%s\n'  "$CUSTOMS"
    printf 'READ_ONLY=%s\n'       "${ARG_READ_ONLY:-false}"
    printf 'DISPATCH_MODE=%s\n'   "$DISPATCH_MODE"
} >> "$STASH"
```

---

## Step 1 — Eligibility (inspired by `/review`)

Use a Haiku agent to check if the pull request (a) is closed, (b) is a draft, (c) does not need a code review (e.g. automated PR, very simple and obviously OK), or (d) already has a code review from you from earlier. If so, do not proceed.

If `review-config.md` declared a "Conventions doc" path, also pass it to the eligibility Haiku — a project may want to gate the command on a token in its conventions doc (e.g. `grep -q 'ProjectName' <Conventions doc>`); the Haiku decides.

If `STEP0B_BLOCKING` is set in the env stash and `--force` was not passed, eligibility is **blocked**: emit the trigger reason and stop.

---

## Step 2 — Context gather (inspired + extended)

Use a Haiku agent to give a list of file paths to (but not the contents of) any relevant `CLAUDE.md` files: the root `CLAUDE.md` if one exists, any per-directory `CLAUDE.md` in the directories the PR modified.

If `CONFIG_MODE=custom`, also pass to the Haiku the `CONVENTIONS_DOC` and `DEV_DOC` paths from `review-config.md` (when set). The Haiku returns the union of paths.

Then use another Haiku to view the pull request and return a short summary of the change.

---

## Step 3.0 — Run dir setup (observability)

Before launching any agent, create the local observability dir for this run. This dir lives outside git (`.devcontainer/local/master-review/runs/` is `.gitignore`d) and is the live observation point for `tail -f` during a multi-agent review. Skipped in `--tier-only` mode (no agents launched).

```bash
. /tmp/master-review-step0.env       # exposes TIER, BEHIND, AHEAD, LEAKAGE, PR
. /tmp/master-review-config.env 2>/dev/null || true

RUN_TS=$(date +%Y-%m-%dT%H-%M-%S)
RUN_DIR=".devcontainer/local/master-review/runs/${RUN_TS}-PR${PR}"
mkdir -p "$RUN_DIR/step0" "$RUN_DIR/agents" "$RUN_DIR/final"

# Snapshot Step 0 outputs (already populated in /tmp by Step 0a..0e)
cp -f /tmp/master-review-step0.env  "$RUN_DIR/step0/env"        2>/dev/null || true
cp -f /tmp/master-review-config.env "$RUN_DIR/step0/config.env" 2>/dev/null || true

# Rotate symlink to point at this run
ln -sfn "$(basename "$RUN_DIR")" ".devcontainer/local/master-review/runs/current"

# Init status.md (updated at each transition during Step 3)
cat > "$RUN_DIR/status.md" <<EOF
# Master Review run — PR ${PR}

Started: ${RUN_TS}
Tier: ${TIER:-?}
Distance: behind=${BEHIND:-?} ahead=${AHEAD:-?} leakage=${LEAKAGE:-?}
Run dir: ${RUN_DIR}/

## Agents

| Slot | Name | Status | Started | Returned | Findings |
| --- | --- | --- | --- | --- | --- |
EOF

# Persist reusable helpers for Step 3 (per-agent updates) and Step 4 (scoring section).
# Single-quoted heredoc so $N / ${RUN_DIR} are not expanded at write time —
# they expand when the consumer block sources this file.
cat > /tmp/master-review-helpers.sh <<'HELPERS'
# Update an agent's row in status.md after the Agent returns.
# Usage: update_agent_status <slot_num> <slot_name> <returned_HHMMSS> <findings_count>
update_agent_status() {
    local slot="${1}" name="${2}" returned="${3}" findings="${4}"
    awk -v F0=0 -v slot="${slot}" -v name="${name}" -v ret="${returned}" -v cnt="${findings}" '
        BEGIN { pat = "^\\| " slot " \\| " name " \\| launched \\|" }
        $F0 ~ pat {
            n = split($F0, parts, /\|/)
            parts[4] = " returned "
            parts[6] = " " ret " "
            parts[7] = " " cnt " "
            out = parts[1]
            for (i = 2; i <= n; i++) out = out "|" parts[i]
            print out
            next
        }
        { print }
    ' "${RUN_DIR}/status.md" > "${RUN_DIR}/status.md.tmp" && mv "${RUN_DIR}/status.md.tmp" "${RUN_DIR}/status.md"
}

# Append (idempotent) or finalize the scoring section.
# Usage: update_scoring_status started
#        update_scoring_status done <kept> <total>
update_scoring_status() {
    local state="${1}" kept="${2:-}" total="${3:-}"
    if [ "${state}" = "started" ]; then
        grep -q '^## Scoring' "${RUN_DIR}/status.md" \
            || printf '\n## Scoring\nStatus: pending\n' >> "${RUN_DIR}/status.md"
    elif [ "${state}" = "done" ]; then
        sed -i "s|^Status: pending\$|Status: done — ${kept} kept / ${total} total|" "${RUN_DIR}/status.md"
    fi
}
HELPERS

# Persist RUN_DIR for subsequent steps (each bash block runs in its own subshell)
echo "RUN_DIR=$RUN_DIR" >> /tmp/master-review-step0.env
```

`.devcontainer/local/master-review/runs/current/` is the canonical "live" pointer for `tail -f` during a run.

---

## Step 3 — Multi-agent parallel review

Launch agents in parallel. **Generic agents 1-5 are inherited verbatim from the upstream `/review`** (their prompts have empirically caught most of the standard classes; do not modify them in source). The launch list is `LAUNCH_GENERICS` (comma list of integers in `{1..5}`) plus `LAUNCH_CUSTOMS` (comma list of integer agent IDs whose prompt files are at `/tmp/master-review-agent-<id>.prompt`).

Each agent runs as a Sonnet sub-agent and returns a list of issues with the reason each was flagged. The harness composes each agent's prompt with a uniform observability hook (live streaming + final report mirror) before invoking the Agent tool — see "Composing the streaming block" below.

### Generic agents (verbatim from upstream `/review`)

Run only those whose ID appears in `LAUNCH_GENERICS`. Slot naming for `$RUN_DIR/agents/`:
- Agent #1 → `1-claude-md`
- Agent #2 → `2-shallow-bug`
- Agent #3 → `3-blame-history`
- Agent #4 → `4-pr-comments`
- Agent #5 → `5-code-comments`

Verbatim prompts (preserved from upstream — do NOT modify in source; the streaming hook is appended at composition time, not stored here):

- **Agent #1** — Audit the changes to make sure they comply with the CLAUDE.md. Note that CLAUDE.md is guidance for Claude as it writes code, so not all instructions will be applicable during code review.
- **Agent #2** — Read the file changes in the pull request, then do a shallow scan for obvious bugs. Avoid reading extra context beyond the changes, focusing just on the changes themselves. Focus on large bugs, and avoid small issues and nitpicks. Ignore likely false positives.

  **Perf hot-path sweep** (Portal42 extension — appended after the verbatim upstream text). For each function that touches I/O (`fread`, `filesize`, `opendir`, `glob`, `readfile`, `scandir`, `file_get_contents` on > 100KB files, JSON decode of > 100KB strings), grep its callers via `grep -rn`:
    1. Identify if any caller is in a `*.php` at docroot OR an API endpoint OR called from inside a `setInterval` / poll / `register_shutdown_function` loop.
    2. If yes AND no caching layer (no static memoization, no Redis check, no mtime-based cache), flag as perf hot-path.
    3. Severity: `medium` if O(filesize) per request, `high` if O(filesize × N requests/min).
  Do not flag pre-existing patterns unless the diff touches the caller or the I/O function.

- **Agent #3** — Read the git blame and history of the code modified, to identify any bugs in light of that historical context.
- **Agent #4** — Read previous pull requests that touched these files, and check for any comments on those pull requests that may also apply to the current pull request.
- **Agent #5** — Read code comments in the modified files, and make sure the changes in the pull request comply with any guidance in the comments.

### Custom agents (loaded from `agents/agent-<id>-<slug>.md`)

For each ID in `LAUNCH_CUSTOMS`, read `/tmp/master-review-agent-<id>.prompt` (the prompt body extracted from `agents/agent-<id>-<slug>.md` at Step 0c). The custom prompt **already includes** the `## Streaming progress (live observability)` block — it is part of the source in the agent file, not appended at composition time. Slot naming for `$RUN_DIR/agents/`:
- Agent #6 → `6-security`
- Agent #7 → `7-lifecycle`
- Agent #8 → `8-adversarial`
- Agent #N (custom user-added, ID > 8) → `N-<slug>` derived from the heading title (lowercase, spaces → `-`, drop diacritics)

Pass the agent the resolved Conventions doc path, Dev doc path, and the PR diff. The agent's `Tools` line declared in config is advisory — the harness enforces tool permissions via this command's own `allowed-tools` frontmatter. If `READ_ONLY=true`, instruct each agent to return findings only, no Edit/Write **of source code** (Write/Edit to `$REVIEW_AGENT_*_PATH` is still allowed — observability files are not source).

When `--resume` is in effect, generic agents 1-5 are NOT relaunched (their prior output is read from the existing recap file); only the custom agents on remaining surfaces run.

### Composing the streaming block (per agent, before Agent tool invocation)

For **every** agent (1-8) about to launch, the harness:

1. Determines slot name (mapping above) and computes concrete paths:
   ```bash
   . /tmp/master-review-step0.env  # exposes RUN_DIR
   LIVE="$RUN_DIR/agents/<N>-<slot>.live"
   FINAL="$RUN_DIR/agents/<N>-<slot>.final.md"
   touch "$LIVE"
   echo "[$(date +%H:%M:%S)] launched" >> "$LIVE"
   ```
   Update `$RUN_DIR/status.md` to add a row: `| <N> | <slot> | launched | HH:MM:SS | | |`.

2. Reads source prompt:
   - For agents 1-5: the bullet text from "Generic agents" above (verbatim from upstream).
   - For agents 6-8 (and custom ID > 8): contents of `/tmp/master-review-agent-<id>.prompt` (already contains the `## Streaming progress` block from `review-config.md`).

3. Composes final prompt:
   - **Prepends a `PR REF CONTEXT` block at the very top** (above the agent's identity/body). This block is the permanent fix for the wrong-diff regression observed on PR-1298 7F.0 (5/7 agents reviewed the orchestrator's branch instead of the PR head because their recipe said `git diff origin/main...HEAD` and `HEAD` resolved to the orchestrator's checked-out branch). The block is added for **every** agent (1-8 and any custom ID > 8), not just the customs — even though generics 1-5 don't currently spell out `git diff … HEAD`, prepending an explicit ref-pair pre-empts any future drift.
   - For 1-5: also appends the streaming block below (separated by `\n\n---\n`). The PR REF CONTEXT prepend is layered on top of the verbatim core; the verbatim core itself stays byte-identical to upstream.
   - For 6-8 and custom ID > 8: skips the streaming append (block already inline in the body file).
   - In all cases, substitutes the placeholders. Use `sed` with a backslash before the dollar so the shell does not expand the placeholder names before sed sees them:
     ```bash
     sed -e "s|\$REVIEW_AGENT_LIVE_PATH|$LIVE|g" \
         -e "s|\$REVIEW_AGENT_FINAL_PATH|$FINAL|g" \
         -e "s|\${PR_BASE_REF}|$PR_BASE_REF|g" \
         -e "s|\${PR_HEAD_REF}|$PR_HEAD_REF|g" \
         -e "s|\${PR_HEAD_OID}|$PR_HEAD_OID|g" \
         -e "s|\${PR_CHANGED}|$PR_CHANGED|g" \
         -e "s|\${PR_ADDITIONS}|$PR_ADDITIONS|g" \
         -e "s|\${PR_DELETIONS}|$PR_DELETIONS|g" \
         -e "s|\${PR}|$PR|g"
     ```
     (`PR_BASE_REF`, `PR_HEAD_REF`, `PR_HEAD_OID`, `PR_CHANGED`, `PR_ADDITIONS`, `PR_DELETIONS`, `PR` all come from `/tmp/master-review-step0.env` via `. /tmp/master-review-step0.env`.)

   The PR REF CONTEXT block to prepend (placeholders are substituted by the sed command above):

   ```
   PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
   - PR number: ${PR}
   - PR base ref:  origin/${PR_BASE_REF}
   - PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
   - For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
   - For reading files at PR head: `git show origin/${PR_HEAD_REF}:<path>`
   - For history: `git log origin/${PR_BASE_REF}..origin/${PR_HEAD_REF} -- <path>` and `git blame origin/${PR_HEAD_REF} -- <path>`
   - NEVER `git diff <X>...HEAD` — `HEAD` resolves to the orchestrator's
     current branch, not the PR head, which produces a wrong-diff
     regression (5/7 agents hit this on PR-1298 S7F.0 pre-validation).
   - Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
     If your `git diff` reports a different shape, you are pointed at
     wrong refs — abort and request the orchestrator to fix the prompt.
   ---
   ```

4. Streaming block to append for agents 1-5 (this block is the **runtime side-effect**; the source prompts of agents 1-5 stay verbatim from upstream):

   ```
   ---
   STREAMING: As you work, append a progress line to the file at $REVIEW_AGENT_LIVE_PATH each time you start scanning a new file or report a finding. Format:
     [HH:MM:SS] action=<scanning|finding|done> details=<short>
   When you finish, also write your full final response to $REVIEW_AGENT_FINAL_PATH using the Write tool — this is your report, mirrored from the message you would otherwise return.
   ```

5. Invokes the Agent tool with the composed prompt.

6. **After the Agent returns** (mandatory — do NOT skip these steps; the orchestrator must execute them between every Agent return and the next one):
   - Capture the response message → write it to `$FINAL` via the Write tool (safety net even if the agent already wrote).
   - Append `[$(date +%H:%M:%S)] finished` to `$LIVE` via Bash.
   - Run a Bash block that sources the helpers and updates the row in `status.md`:
     ```bash
     . /tmp/master-review-step0.env       # exposes RUN_DIR
     . /tmp/master-review-helpers.sh      # exposes update_agent_status
     update_agent_status <N> <slot> "$(date +%H:%M:%S)" <findings_count>
     ```
     `<findings_count>` comes from the agent's response (count `BLOCKING + N MINOR + ...` headers, or count bullet items in the final report). If the agent failed entirely, pass `0`.

7. Status.md transitions during the rest of the flow:
   - When Step 4 (scoring) starts: append `\n## Scoring\nStatus: pending\n`.
   - When Step 4 finishes: replace the scoring section with `Status: done — N kept / M total`.

### Examples of false positives (verbatim from upstream `/review`)

- Pre-existing issues
- Something that looks like a bug but is not actually a bug
- Pedantic nitpicks that a senior engineer wouldn't call out
- Issues that a linter, typechecker, or compiler would catch
- General code quality issues unless explicitly required in CLAUDE.md
- Issues that are called out in CLAUDE.md but explicitly silenced in the code (e.g. lint ignore comment)
- Changes in functionality that are likely intentional or directly related to the broader change
- Real issues, but on lines that the user did not modify in this pull request

---

## Step 4 — Confidence scoring (Haiku, rubric verbatim from `/review`)

**Before spawning the Haiku scorers** (mandatory), run a Bash block to mark scoring as started in `status.md`:

```bash
. /tmp/master-review-step0.env       # exposes RUN_DIR
. /tmp/master-review-helpers.sh      # exposes update_scoring_status
update_scoring_status started
```

For each issue found in Step 3, launch a parallel Haiku agent that takes the PR, the issue description, and the list of CLAUDE.md / Conventions doc paths from Step 2, and returns a confidence score 0-100. For issues flagged due to CLAUDE.md or Conventions doc, the Haiku must double-check that the doc actually calls out that issue specifically.

Rubric (give to the Haiku verbatim):

- **0** — Not confident at all. False positive that doesn't stand up to light scrutiny, or a pre-existing issue.
- **25** — Somewhat confident. Might be a real issue, but may also be a false positive. Could not verify. If stylistic, not explicitly called out in the relevant CLAUDE.md.
- **50** — Moderately confident. Verified real, but might be a nitpick or rare in practice. Not very important relative to the rest of the PR.
- **75** — Highly confident. Double-checked; very likely a real issue that will be hit in practice. The existing approach in the PR is insufficient. Important and will directly impact functionality, OR explicitly mentioned in CLAUDE.md.
- **100** — Absolutely certain. Double-checked; definitely real, will happen frequently in practice. Direct evidence.

Filter: keep issues with `score >= THRESHOLD_DEFAULT` (default 80). For agents whose ID has a `THRESHOLD_AGENTS_*` override in env, apply the override (e.g. 75 for project-specific custom agents that are calibrated to err on the side of false positives). If no issues remain, do not proceed to Step 6 — the PR is clean by the rubric.

**After all Haiku scorers have returned** and the threshold filter has been applied (mandatory), run a Bash block to finalize the scoring section in `status.md`:

```bash
. /tmp/master-review-step0.env       # exposes RUN_DIR
. /tmp/master-review-helpers.sh      # exposes update_scoring_status
update_scoring_status done <kept_count> <total_count>
```

Re-run the eligibility check from Step 1 (Haiku) before Step 6 to catch any state that changed during the review (PR closed, draft, already-commented).

---

## Step 5 — Local output (project-aware)

Resolve `OUTPUT_ROUND_FILE`, `OUTPUT_RECAP_FILE`, `OUTPUT_SURFACES_FILE` from env (defaults `PR-${PR}-review.md` etc.). Substitute `${PR}` with the actual PR number. If `READ_ONLY=true`, redirect all output to `/tmp/master-review-readonly-${PR}.md` (single file with all sections concatenated) and skip the recap update.

Write the **round file** with sections:
1. **Header**: PR #, branch, tier label, score, distance from main, leakage status.
2. **Findings (filtered)**: blocked / blocking-fixes / nice-to-have, severity-sorted. Each finding has file:line link, brief description, the agent that flagged it, the confidence score.
3. **Surfaces checklist** (T3+ when `SURFACES_REQUIRED` non-empty): `[checked]` / `[skipped: reason]` per surface ID, copied from the surface table parsed at Step 0c.
4. **Coverage report**: aggregate of agent self-reports (the `Coverage report` / `Generalization sweep` sections that custom agents produce).
5. **Out-of-scope / pre-existing**: items the agents flagged but that the threshold filter or false-positive list demoted.

Update the **recap file** (idempotent — preserves prior rounds, appends new):
- Decisions table (one row per finding ever raised on this PR): `# | Item | Decision | Fix commit | Round`.
- "Surfaces couvertes" section with the cumulative checklist (used by `--resume` on next session).

If T4+, also write the **surfaces matrix** (`OUTPUT_SURFACES_FILE`) — a 2D table of surface × round.

After writing the round/recap files (or the read-only single file), snapshot them into the run dir's `final/` subfolder so the run dir is a self-contained record of this session:

```bash
. /tmp/master-review-step0.env  # exposes RUN_DIR
if [ -n "$RUN_DIR" ]; then
    if [ "$READ_ONLY" = "true" ]; then
        cp -f "/tmp/master-review-readonly-${PR}.md" "$RUN_DIR/final/" 2>/dev/null || true
    else
        ROUND_FILE_RESOLVED=$(printf '%s' "$OUTPUT_ROUND_FILE" | sed "s/\${PR}/$PR/g")
        RECAP_FILE_RESOLVED=$(printf '%s' "$OUTPUT_RECAP_FILE" | sed "s/\${PR}/$PR/g")
        [ -f "$ROUND_FILE_RESOLVED" ] && cp -f "$ROUND_FILE_RESOLVED" "$RUN_DIR/final/$(basename "$ROUND_FILE_RESOLVED")"
        [ -f "$RECAP_FILE_RESOLVED" ] && cp -f "$RECAP_FILE_RESOLVED" "$RUN_DIR/final/$(basename "$RECAP_FILE_RESOLVED")"
    fi
fi
```

---

## Step 6 — GitHub comment (inspired by `/review`)

If `OUTPUT_GH_COMMENT=enabled` and `READ_ONLY=false` and the threshold filter kept any findings, post a `gh pr comment` with the format below — preserved verbatim from the upstream `/review` and APPENDED with the project-aware section.

The format must include the FULL git SHA in code links (no abbreviations, no `$(git rev-parse HEAD)`), and the footer 👍/👎 line so users can react.

### Comment format

```
### Code review

Found N issues:

1. <brief description of bug> (CLAUDE.md says "<...>")

<link to file and line with full sha1, e.g. https://github.com/owner/repo/blob/<FULL_SHA>/path/to/file.ext#L<start>-L<end>>

2. <brief description of bug> (some/other/CLAUDE.md says "<...>")

<link to file and line with full sha1>

3. <brief description of bug> (bug due to <file and code snippet>)

<link to file and line with full sha1>

---

### Project-aware section

- **Tier**: T<n> (score <s>)
- **Surfaces**: A [checked] · B [checked] · C [skipped: not touched] · ...
- **Severity breakdown**: <critical>×N · <high>×N · <medium>×N · <low>×N

🤖 Generated with [Claude Code](https://claude.ai/code)

<sub>- If this code review was useful, please react with 👍. Otherwise, react with 👎.</sub>
```

If no issues remain after Step 4 filtering:

```
### Code review

No issues found. Checked for bugs and CLAUDE.md compliance.

🤖 Generated with [Claude Code](https://claude.ai/code)
```

When linking to code, follow this format precisely or the markdown preview won't render: `https://github.com/owner/repo/blob/<FULL_SHA>/path#L<start>-L<end>`. Always provide at least 1 line of context before and after, centered on the line being commented on. NEVER use `$(git rev-parse HEAD)` — the comment is rendered as raw markdown, no shell expansion.

Resolve `<FULL_SHA>` with `git rev-parse HEAD` AT COMMAND TIME and embed the literal SHA in each link.

---

## Step 7 — Commit hygiene linter

Lints commit hygiene only when the user explicitly asks the command to propose a fix-commit message (e.g. `master-review --commit-message`) or when, during the review, the user has staged files and is about to commit. The command never auto-commits.

Read regexes from `/tmp/master-review-commit-regex.txt` (set at Step 0c with config defaults if no config). Spawn a Haiku agent that:
1. Receives the candidate commit subject + body.
2. Tests each line against each regex (treat the subject and body as one corpus).
3. Returns `{ ok: true, sanitized_message }` if no match, or `{ ok: false, violations: [...], suggested_message: "..." }` if any match.

Default regexes (used if no `Commit Hygiene Regex` section in config):
- `(?i)round\s*\d+`
- `(?i)R\d+-\d+`
- `(?i)review\s+(fix(es)?|polish|done|round)`
- `(?i)PR-?\d+-(review|recap)`

If violations are found, present them to the user, propose `<scope>: <change concrete>` style alternative, and require explicit user approval before any `git commit` runs.

---

## Step 8 — Metrics + kickoff

Append a TSV row to `~/.claude/review-sessions.log` with these columns (tab-separated):

```
timestamp  pr_number  tier  round  session_id  jsonl_lines  user_prompts  new_findings  fixed_findings  surfaces_checked  duration_min
```

- `timestamp`: ISO 8601 (`date -u +%FT%TZ`)
- `pr_number`: from `$PR`
- `tier`: from env stash
- `round`: parsed from the round file (`Round N`) — if absent, infer from recap row count + 1
- `session_id`: basename of `$JSONL` (without `.jsonl`)
- `jsonl_lines`: from `$JSONL_LINES`
- `user_prompts`: count of `"role":"user"` lines in `$JSONL`
- `new_findings`: count of findings written in this round file
- `fixed_findings`: count of recap rows with non-empty `Fix commit` updated this round
- `surfaces_checked`: comma list (e.g. `A,B,F`)
- `duration_min`: end-time minus start-time of this command in minutes

**Re-run suggestion (opt-in user)** — after writing the TSV row, if the round produced ≥ 5 new findings, surface a one-line nudge that re-running `--resume` post-fix is cheap insurance against round-over-round regressions (a fix can introduce a new vulnerability window). Replaces the dropped post-commit hook — pure suggestion, no auto-action.

```bash
if [ "$NEW_FINDINGS" -ge 5 ]; then
    printf '\n💡 %s findings detected. After committing fixes, you can re-run `/master-review %s --resume` to verify nothing was missed and catch round-over-round bugs (fix introduces new vulnerability window).\n' "$NEW_FINDINGS" "$PR"
fi
```

Then run two checks:

**Plateau detection** — read the last two lines of `~/.claude/review-sessions.log` matching this `pr_number`. If both have `new_findings < 3`, emit "review close — plateau atteint, 2 sessions consecutives < 3 nouveaux findings". For T4+, the threshold tightens to 3 lines at `< 2`.

**Saturation / kickoff** — if `JSONL_LINES > 700` OR plateau detected OR a panic-cascade signal fires (≥ 3 sessions on this PR in < 2 hours, by reading the log), emit a kickoff-prompt block:

```
=========================================================================
NEW SESSION RECOMMENDED — paste this prompt in a fresh session:
─────────────────────────────────────────────────────────────────────────
Resume review PR #<PR> — round <N+1> (<TIER>).

Current state:
- <fixed_findings> findings fixed (commits: <last 4 SHAs in chronological order>)
- <open_findings> findings open
- Surfaces covered: <SURFACES_COVERED>
- Surfaces remaining: <SURFACES_REMAINING>

Before any other prompt:
1. cat <RECAP_PATH>
2. cat <ROUND_PATH> (current round)
3. Run: master-review <PR> --resume --surfaces=<SURFACES_REMAINING>

Run dir (this session, gitignored): .devcontainer/local/master-review/runs/current/
=========================================================================
```

The kickoff block is printed to stdout; the user copies it into a new session. (When a notification mechanism such as `PushNotification` is available, it may also be sent, but that's out of scope for this command.)

---

## Configuration File Specification

The command parses a per-project `review-config.md` at startup. Resolution order: `--config=<path>` flag → `.devcontainer/skills/master-review.local/review-config.md` → `.devcontainer/claude/review-config.md` → `.claude/review-config.md` → `<repo-root>/review-config.md`. None found → interactive bootstrap (Step 0c.5) unless `.devcontainer/skills/master-review.local/.skip-bootstrap` sentinel is present, in which case vanilla mode.

### Section taxonomy

**Required** (absent → warning + vanilla fallback for that aspect):
- `## Project Meta` (bullet list of `**Key**: value`)
- `## Surfaces` (markdown table: ID | Surface | Trigger patterns | Agent assigned)
- `## Custom Agents` (pointer to the `agents/` subdir — one `.md` file per agent; see "Custom Agent block" below)
- `## Output Paths` (bullet list of `- <Label>: <path>`)

**Optional** (absent → silent default):
- `## Tier Scoring Overrides` (table: Signal | Weight; weight `BLOCKING — abort review` routes to abort triggers)
- `## Tactical Framings` (table: ID | Framing verbatim | Target surfaces | Used by agents)
- `## Commit Hygiene Regex` (bullet list of `` `regex` ``)
- `## Special-case Files` (bullet list `- ``path`` (block review|warn before review): reason`)
- `## Override Threshold` (`- Default <N>` and/or `- <N> for agents <range>`)
- `## GitHub Review Threads` (bullet `- enabled: true|false`, default `true`. When `enabled: false`, Agent #4 — prior PR review comments — is dropped from the generic launch list. Use for projects where review feedback lives in local files rather than GitHub native review threads.)

### Custom Agent block

Each custom agent lives in its own file `agents/agent-NN-<slug>.md` next to `review-config.md` (zero-padded id, lowercase slug). Format: YAML frontmatter + body.

```markdown
---
id: 6
name: Security Portal42
trigger: tier ≥ T3
tools: Read, Write, Edit, Bash(grep:*), Bash(git diff:*), Bash(git log:*)
---

You are Agent #6 — ...
[full prompt body verbatim — no fence, no `**Prompt**:` marker]
```

Required frontmatter keys: `id`, `name`, `trigger`, `tools`. The body is everything after the second `---` line (one leading blank line is allowed and ignored). The body becomes the agent's full system prompt. Triple backticks inside the body are tolerated — there is no enclosing fence.

The Step 0c walker globs `agents/agent-*.md` (in the skill folder, falling back to a sibling dir of the resolved config) and, for each file, writes the body to `/tmp/master-review-agent-<id>.prompt` and appends `<id>\t<trigger>\t<tools>\t<promptfile>` to `/tmp/master-review-agents.tsv`. `## Custom Agents` in `review-config.md` is just a pointer paragraph to the subdir.

### Parsing rules surfaced to the user

- **Empty section** → warning, default applies for that aspect.
- **Malformed table** (no header separator row, ragged columns) → warning, that section ignored.
- **Custom agent file missing `id:` frontmatter** → that agent skipped, warning, remaining agents loaded.
- **Unparseable tier weight** (no `[-+]?[0-9]+`, no `BLOCKING`) → row ignored.
- **Unknown section** → silently ignored (forward-compat).
- All section names case-sensitive.

### Validation Haiku

After parsing, a Haiku agent receives the raw config + detected sections + parser warnings, and emits strict JSON: `{status: ok|warnings|fatal, warnings: [...], missing_required: [...], skipped_agents: ["#N", ...], summary: "<one-liner>"}`. Status `fatal` (e.g. `Custom Agents` missing on a T3+ PR) aborts unless `--force` is passed.

### Vanilla mode

If no `review-config.md` is found AND the user picks "No — vanilla this run" or "Skip and remember" at the bootstrap prompt (Step 0c.5), the command runs in vanilla mode for that session. It prints:

```
note: continuing in vanilla mode for this session (generic agents 1-5 only).
```

(or a sentinel-aware variant when `.skip-bootstrap` is present) and proceeds with: agents 1-5 from upstream `/review`, default tier weights (lines/files only), default output paths (`PR-${PR}-review.md` etc.), threshold 80 for all agents, default commit-hygiene regex, no surface checklist, no custom agents, no framings.

To re-enable the interactive bootstrap after picking "Skip and remember", `rm .devcontainer/skills/master-review.local/.skip-bootstrap`.

### Adding a new custom agent

1. Pick the next free id (≥ 6 — `#1` through `#5` are reserved for the vanilla generic agents).
2. Create `agents/agent-NN-<slug>.md` (zero-padded id, lowercase slug) next to `review-config.md`. Use this template:

   ```markdown
   ---
   id: 9
   name: <Short Name>
   trigger: tier ≥ TX
   tools: Read, Bash(grep:*)
   ---

   <full agent prompt body verbatim>
   ```

3. Append a row to the `## Surfaces` table in `review-config.md` if the agent owns one or more surfaces.
4. Append a bullet to the `## Custom Agents` pointer list in `review-config.md` so the new agent shows up alongside the existing ones.

See `agents/README.md` for the full file-format reference.

---

## Notes (verbatim from upstream `/review`)

- Do not check build signal or attempt to build / typecheck the app. CI runs separately, not part of this review.
- Use `gh` to interact with GitHub (not WebFetch).
- Make a todo list first.
- Cite and link each bug. CLAUDE.md citations must include the linked path and quoted text.
- For the GitHub comment, follow the format above exactly. Link to code with the FULL sha1 (no abbreviations).

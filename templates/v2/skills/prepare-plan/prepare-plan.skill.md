---
description: Scaffold a multi-session rollout directory (ROLLOUT + STATUS + LOG + EXISTING + sessions/) for a feature/fix/refactor that needs ≥3 sessions of work. Auto-triggers on natural phrases like "fais-moi un plan", "prépare un rollout", "scaffold a plan" — when auto-triggered, asks the user for confirmation before writing anything.
argument-hint: "<feature-name> [<free-text scope description>]"
---

# /prepare-plan — scaffold a multi-session rollout

This is a **meta-skill** that industrialises a rollout pattern: ROLLOUT.md
+ STATUS.md + LOG.md + EXISTING.md + sessions/session-NN.md, all under
`/workspace/plans/<feature>/`. Given a feature name and a free-text scope
description, it generates a self-contained planning directory ready to
drive multiple Claude Code sessions.

The output directory follows a strict convention so that **every session
prompt prescribes its own DoD** (update STATUS / LOG / EXISTING in-line at
the end of the session). No companion skill, no hooks, no automation — the
pattern is self-perpetuating once seeded.

## When to use

Trigger this skill when the work ahead clearly spans **multiple sessions**
and benefits from a shared scoreboard :

- Large feature touching several areas of the codebase (≥3 distinct sessions).
- Multi-phase refactor / migration where intermediate states must be tracked.
- Infrastructure overhaul (devcontainer, CI, build pipeline) with a sequence
  of irreversible steps.
- Any task where the user says things like : "fais-moi un plan", "prépare
  un rollout", "monte un plan pour", "scaffold a plan", "prep a rollout for".

## When NOT to use

- Single-session task — just do the work, no plan dir needed.
- One-shot bug fix — a commit message is enough.
- Refactor isolated to one file or one small module.
- A research / exploration that has no deliverable plan — use
  `/prepare-research` instead.

When in doubt, ask the user before scaffolding. A wrongly-created plan dir
forces a manual cleanup and erodes trust in the skill.

## Process

### 0. Confirm-on-auto (skip if invoked via slash)

**Determine the invocation mode** before doing anything else :

- If the user's last message contains the literal token `/prepare-plan`,
  this is an **explicit slash invocation**. Skip directly to step 1, no
  confirmation needed — the user typed the command, intent is clear.
- Otherwise this is an **auto-trigger** : Claude self-proposed the skill
  based on a natural-language phrase ("fais-moi un plan pour …",
  "scaffold a plan", …). Before touching disk, call `AskUserQuestion` ONCE
  with a single question :

  ```
  Question : "Je propose de scaffold un dossier de plan multi-sessions
              pour `<inferred feature name>`. On y va ?"
  Options  :
    - "Yes, go"        → continue to step 1
    - "No, cancel"     → abort cleanly, output one line "Plan scaffold
                         cancelled — no files written."
    - "Edit scope"     → ask the user a follow-up for the corrected
                         feature name + description, then continue
  ```

  Never write to disk before the user has answered Yes or Edit. Never call
  `AskUserQuestion` a second time for confirmation in the same invocation.

### 1. Parse invocation and derive identifiers

The invocation has the shape `/prepare-plan <feature-name> [<description>]`
(or the natural-language equivalent). Resolve :

- `feature_name` : kebab-case identifier. If the first whitespace-separated
  token is already kebab-case (`[a-z][a-z0-9-]*`), use it as-is. Otherwise
  derive one from the description (lowercase, strip stopwords, kebab-case,
  ≤ 40 chars). Reject names containing `/`, `..`, uppercase letters, or
  spaces.
- `description` : everything after `feature_name`, trimmed. If empty,
  prompt the user for a one-line scope description before continuing.
- `feature_title` : human-readable title — capitalise each word of
  `feature_name` (split on `-`), join with spaces. Used only in the
  `# Header` lines of generated docs.
- `date` : today in ISO format (`YYYY-MM-DD`). Resolve via `date +%F`.
- `first_session_slug` : a short kebab-case identifier for the first
  session's concrete first step. Default `scaffold` if you cannot infer
  better from the description. Examples : `scaffold`, `inventory`,
  `baseline`, `spike`.

If the derived `feature_name` looks too generic (≤ 3 chars, or a single
common verb : `fix`, `add`, `do`), ask the user for an explicit one before
continuing. Print one short confirmation line and proceed without waiting
unless the inference looks risky :

```
→ feature_name=<name>, first_session_slug=<slug>, target=/workspace/plans/<name>/
```

### 2. Collision check

Compute `target = /workspace/plans/${feature_name}/`.

- If `target` does NOT exist : proceed to step 3.
- If `target` exists : NEVER overwrite. Propose the next free suffix —
  try `${feature_name}-v2`, then `-v3`, etc. — recompute `target` and
  print one line :

  ```
  ⚠ /workspace/plans/<name>/ already exists. Proposing /workspace/plans/<name>-v2/ instead.
    Confirm with "yes" or provide a different name.
  ```

  Wait for user confirmation before proceeding. Never silently fall through
  to the `-v2` path — the user may want to append to the existing plan
  (which this skill does NOT support — they should edit the existing
  STATUS.md / LOG.md by hand).

### 3. Exploration heuristic — fill EXISTING.md

Decide whether the plan benefits from a code-base scan to seed EXISTING.md :

- **Skip exploration** if the scope is obviously confined : single file,
  single small module, or the description names specific files. Read those
  files directly with `Read` and summarise inline.
- **Spawn 1–3 Explore agents in parallel** if the scope is broad or the
  area is unfamiliar (e.g. "refactor the auth layer", "audit packet
  handlers across plugins"). Each agent gets a focused brief : one for
  existing implementations, one for related components, one for testing
  patterns. Report thoroughness level "quick" — EXISTING.md is a starting
  inventory, not a deep audit.
- **Default to skip** if you cannot articulate a precise question for an
  Explore agent. A vague exploration produces noise.

Aggregate the findings into a draft EXISTING.md section. If exploration is
skipped, EXISTING.md is generated as a stub with a clear "fill me" header
— the first session will populate it.

### 4. Generate the 5 files

All paths below are absolute. `ROOT="/workspace/plans/${feature_name}"`.

```bash
mkdir -p "$ROOT/sessions"
```

Write each file substituting every `{{placeholder}}` with its resolved
value. The templates live inline below (§"File templates"). Variables :

- `{{feature_name}}`     → kebab-case slug
- `{{feature_title}}`    → human-readable title
- `{{description}}`      → free-text scope from user
- `{{date}}`             → today ISO `YYYY-MM-DD`
- `{{first_session_slug}}` → first session slug
- `{{existing_body}}`    → either the exploration findings, or the
                            fill-me stub paragraph

Write in this order :

1. `$ROOT/ROLLOUT.md`
2. `$ROOT/STATUS.md`
3. `$ROOT/LOG.md`
4. `$ROOT/EXISTING.md`
5. `$ROOT/sessions/session-1-{{first_session_slug}}.md`

### 5. Sanity check — no leftover placeholders

After all 5 files are written :

```bash
if grep -RE '<feature[_-]name>|<feature[_-]title>|<first[_-]session[_-]slug>|\{\{[a-z_]+\}\}' "$ROOT" 2>/dev/null; then
  echo "❌ Unresolved placeholders in $ROOT — aborting and removing partial output."
  rm -rf "$ROOT"
  exit 1
fi
```

If the grep finds anything, the substitution missed a spot — wipe the
output directory and stop. The user prefers an empty result over a broken
plan.

### 6. Print the next-steps block

```
✅ Plan scaffolded at /workspace/plans/{{feature_name}}/

Contents :
  ROLLOUT.md                 entry point — read first
  STATUS.md                  session scoreboard (1/N seeded)
  LOG.md                     append-only journal (empty)
  EXISTING.md                code inventory (filled / stub)
  sessions/session-1-{{first_session_slug}}.md
                             first session prompt — copy-paste into a fresh chat

To run session 1 :
  1. Open sessions/session-1-{{first_session_slug}}.md
  2. Copy the "Prompt to paste" block (between the 4-backtick fences)
  3. Paste into a new Claude Code session
  4. When that session is done, its DoD prescribes the STATUS / LOG updates

To add the next session : edit STATUS.md (new row) and create
  sessions/session-2-<slug>.md following the same template as session 1.
```

## File templates

The five templates below are the source of truth for the generated files.
**Substitute every `{{placeholder}}` before writing — the sanity-check at
step 5 enforces zero leftovers.**

### Template — `ROLLOUT.md`

````markdown
# Rollout — {{feature_title}}

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).

## Goal

{{description}}

## Navigation

| File | When to open |
|---|---|
| **[STATUS.md](STATUS.md)** | "Where are we, what's next ?" — actionable session table |
| **[LOG.md](LOG.md)** | "What was done, why, what gotchas ?" — append-only journal |
| **[EXISTING.md](EXISTING.md)** | "What does the code look like today ?" — factual inventory |
| sessions/session-NN-*.md | Prompt to paste into a new Claude chat to start session NN |

## How to use

1. **To resume work** : open [STATUS.md](STATUS.md), find the next 📋
   session, click `→ prompt` and paste into a fresh Claude Code session.
2. **To check what was done before** : read [LOG.md](LOG.md).
3. **To understand current code state** : read [EXISTING.md](EXISTING.md).

## Update convention (end of every delivered session)

Every session prompt prescribes these three updates in its DoD :

1. **STATUS.md** : flip the session row 📋 → ✅, replace the prompt link
   with `—`, bump the "Delivered" counter, refresh "Next focus".
2. **LOG.md** : append `## <Session ID> — <Title>` section dated today,
   listing files touched + What / Why / Decisions / Gotchas / Tests /
   Commit (~50–150 lines).
3. **EXISTING.md** : update if new files / structures were created.

No companion skill, no automated hook — the session itself does the work
because its prompt explicitly says so.

## Decisions (immutable unless user explicitly amends)

_(Add decisions here as they are made. Each decision should explain the
trade-off and why this side was picked, so future sessions don't relitigate
settled questions.)_

- _(none yet — first session may seed this list)_
````

### Template — `STATUS.md`

````markdown
# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | {{first_session_slug}} — first concrete step | 📋 | [→ prompt](sessions/session-1-{{first_session_slug}}.md) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 0 / 1
- **Next focus** : session 1 ({{first_session_slug}})
````

### Template — `LOG.md`

````markdown
# Log — {{feature_title}}

> Append-only journal. One section per delivered session. Newest at the
> bottom. Each section follows the same shape :

```
## <Session ID> — <Title>

**Date** : YYYY-MM-DD
**Files touched** :
- path/to/file1
- path/to/file2

**What** : one-paragraph summary of the change.

**Why** : the reason / constraint that drove this scope.

**Decisions** :
- _bullet — short rationale_

**Gotchas** :
- _bullet — surprise or pitfall encountered_

**Tests** :
- _command run + expected outcome_

**Commit** : `<short hash> — <commit subject>` (or "not committed yet")
```

---

_(no sessions delivered yet — first append will be after session 1)_
````

### Template — `EXISTING.md`

````markdown
# Existing — technical inventory

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

{{existing_body}}
````

When exploration was skipped, `{{existing_body}}` is :

```markdown
## To be filled

This inventory was not pre-populated when the plan was scaffolded —
session 1's first action is to fill it. Use `Read` and/or `Explore`
sub-agents to map :

- Directory structure of the relevant area
- Existing functions / modules that may be reused
- Tests covering the area
- Known gaps / pain points the rollout will address
```

When exploration ran, `{{existing_body}}` is the aggregated findings
formatted as sections.

### Template — `sessions/session-1-{{first_session_slug}}.md`

````markdown
# Session 1 — {{first_session_slug}}

> **Effort** : ~0.5–1 day | **Dependencies** : none (first session)

## Prompt to paste

`````
I'm starting session 1 of the `{{feature_name}}` rollout.

Entry point : `/workspace/plans/{{feature_name}}/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory)
- `sessions/session-1-{{first_session_slug}}.md` (this spec)

Goal : {{description}}

First session focus : {{first_session_slug}}. Concretely — propose the
shape, validate with me, then implement the first concrete step. Keep
scope tight ; if you discover follow-up work, add a session-2 row to
STATUS.md instead of folding it in.

DoD at the end of this session :
1. STATUS.md : flip session 1 row 📋 → ✅, prompt link → —, bump
   Delivered counter (0→1), refresh "Next focus" to session 2 (or note
   "rollout complete" if no follow-up needed).
2. LOG.md : append `## 1 — {{first_session_slug}}` section dated today
   with files touched + What / Why / Decisions / Gotchas / Tests /
   Commit.
3. EXISTING.md : update if new files / structures were created.
4. Propose a commit (do NOT commit without explicit user confirmation).
`````

## Next session

To be decided at the end of session 1 — add a new row in STATUS.md and
create `sessions/session-2-<slug>.md` following the same minimal shape
as this file.
````

> 🚨 **The session template uses 5-backtick fences for the prompt block**
> so the prompt can itself contain 4-backtick fences for embedded code
> blocks if needed. When writing this file, keep the 5-backtick fences
> intact.

## Constraints

- Output path is always `/workspace/plans/<feature_name>/`. Never write
  the plan directory anywhere else (not `~/.claude/plans/`, not
  `/tmp/`, not the project root).
- Never overwrite an existing plan directory — propose `-v2`, `-v3`, …
  and require explicit user confirmation.
- `feature_name` must be lower-case kebab : `[a-z][a-z0-9-]*`. Reject
  names containing `/`, `..`, uppercase, or spaces.
- Every generated file MUST contain resolved values — no surviving
  `{{placeholder}}`, no `<feature_name>`, no `<feature_title>`. Step 5
  enforces this with a `grep` over the full output directory.
- All generated files are in **English** per the project's CLAUDE.md
  rule, even when the conversation with the user is in French.
- The skill writes **exactly 5 files** (4 top-level + 1 session-1).
  Do not generate `docs/`, multiple sessions, or other companion files
  — those are added on-demand by later sessions if justified.
- On auto-trigger, `AskUserQuestion` is called ONCE for confirmation.
  Never call it a second time in the same invocation for "are you sure".

## Failure modes

| Symptom | Cause | Mitigation |
|---|---|---|
| `feature_name` directory already exists | Repeat invocation or pre-existing plan | Refuse, propose `-v2`, wait for user confirmation. Never overwrite. |
| Derived `feature_name` too generic (≤ 3 chars, common verb) | Description was too vague | Ask the user for an explicit name before continuing. |
| Exploration agent times out or returns nothing | Scope mis-bounded for Explore | Fall back to the stub EXISTING.md. The first session can fill it. |
| Sanity-check at step 5 finds a `{{placeholder}}` | Substitution missed a token in the templates | Wipe the output directory and abort. Better empty than broken. |
| User says "No, cancel" on the auto-trigger confirmation | Skill was self-proposed against intent | Print "Plan scaffold cancelled — no files written." and stop. Do not retry. |
| User wants to extend an existing plan instead of starting `-v2` | This skill does NOT support append mode | Tell the user to edit STATUS.md / LOG.md / sessions/ by hand for an existing plan. |

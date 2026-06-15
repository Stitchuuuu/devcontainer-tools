---
description: Route code work to the right context after a plan is built. Three modes — (1) implement in the current session, (2) produce a self-contained prompt to paste into a fresh chat (no scaffold), or (3) scaffold a multi-session rollout directory under /workspace/plans/<feature>/. Always asks first via AskUserQuestion and recommends the most fitting based on scope. AUTO-TRIGGER at Phase 4 of Plan mode (after exploration, just before writing the final plan file) to route the implementation context. Also auto-triggers on natural phrases like "fais-moi un plan", "scaffold a plan", "j'ai pas le temps".
argument-hint: "<feature-name> [<free-text scope description>]"
---

# /prepare-plan — scaffold a plan rollout

Generates a self-contained plan directory under `/workspace/plans/<feature>/`:
`ROLLOUT.md` + `STATUS.md` + `LOG.md` + `EXISTING.md` + `sessions/session-1-*.md`.
Each session prompt prescribes its own DoD (update STATUS / LOG / EXISTING at the
end), so the pattern is self-perpetuating — no companion skill, no hook.

## When to use

**Auto-invoked at Phase 4 of Plan mode** (after exploration, just before writing
the final plan file) to route the implementation context — Claude should call
the skill at that moment rather than waiting for an explicit user trigger.

Outside Plan mode, invoke whenever the user wants to **route code work to the
right context** after a plan is done. Common triggers: "fais-moi un plan",
"prépare un rollout", "monte un plan pour", "scaffold a plan", "prep a rollout
for", "j'ai pas le temps de m'en occuper", "je verrai ça plus tard".

The scope decision (this session / prompt only / multi-session scaffold) is the
user's pick at step 0 — not gated by this section.

## When NOT to use

- Research / exploration without a deliverable plan → use `/prepare-research`.
- Append to an existing plan directory → this skill only scaffolds fresh ones.
  To extend an existing one, edit its `STATUS.md` / `LOG.md` / `sessions/` by hand.

## Process

### 0. Context-routing decision — mandatory AskUserQuestion

**Always ask** before touching disk — whether triggered via `/prepare-plan`
slash, auto-proposed from a natural-language phrase, or auto-invoked at Phase 4
of Plan mode. The user picks ; the skill recommends.

**What the question routes:** the **context budget for the code work that
follows the plan**. The plan is built *now* ; the implementation happens
*next*. Three options:

- **This session** — implement in the current chat. Right for small fixes with
  few exchanges and a chat that still has headroom. Any scaffold would be overhead.
- **Fresh chat, prompt only** — produce a self-contained prompt shown in chat,
  paste into a fresh session. No files written, no scaffold directory. Right
  for one PR-worth of focused work that fits in one fresh session and won't
  need a scoreboard to come back to.
- **Multi-session scaffold** — scaffold 5 files under `/workspace/plans/<name>/`
  (ROLLOUT + STATUS + LOG + EXISTING + sessions/session-1-*). Right when the
  change spans ≥2 sessions, has irreversible phases, crosses several areas,
  or needs an audit trail to resume on later.

Call `AskUserQuestion` ONCE. **Reorder the options so the recommended one is
first** and append `(Recommended)` to its label.

Recommendation heuristic:

| Scope signal | Recommend |
|---|---|
| 1-line fix, config tweak, isolated single-file edit, ≤ ~10 exchanges expected | This session |
| One PR-worth of work, one focused feature/refactor, fits in one fresh chat | Fresh chat, prompt only |
| ≥2 distinct sessions, irreversible phases, cross-cutting refactor, audit trail wanted | Multi-session scaffold |

```
Question : "Where should we implement `<inferred feature name>` ?
            (planning is done — picking the context for the code work)"
Options (Recommended one first) :
  - "This session"             — Implement in the active chat, no scaffold.
  - "Fresh chat, prompt only"  — Self-contained prompt shown in chat. No files.
  - "Multi-session scaffold"   — Scaffold 5 files, expect ≥2 sessions.
```

Exactly one option carries `(Recommended)` — never zero, never two.

**On the answer:**

- **This session** → abort cleanly, no files written. Print one line:
  `Plan scaffold skipped — implementing in current session.`
  Return control so Claude implements now.
- **Fresh chat, prompt only** → branch to §"Prompt-only mode" below. Skip
  steps 1–6 entirely.
- **Multi-session scaffold** → proceed to step 1 with `mode = multi`.

Never call `AskUserQuestion` a second time in the same invocation.

### 0.5. Prompt-only mode (branch from step 0)

Reached only if the user picked **"Fresh chat, prompt only"** at step 0.
Steps 1–6 are skipped entirely — zero files are written.

1. Resolve `feature_name`, `feature_title`, `description` per step 1's rules
   (kebab + title + scope text). If `description` is empty, ask for a one-line
   scope before continuing.
2. Build the prompt from the template in §"File templates" → "Template —
   `prompt-only` mode". Substitute:
   - `{{feature_title}}` → human title
   - `{{description}}`   → scope text
   - `{{plan_approach}}` → the design just produced (when invoked at Phase 4
     of Plan mode) or the upstream context. If empty, leave a one-line
     placeholder `_(no approach captured — first session will design it)_`.
   - `{{tests}}`         → verification steps from the plan, or a generic
     `tests pass / feature exercises end-to-end` line.
3. **Always print the prompt directly in chat, wrapped in a 5-backtick
   fence.** Same behavior in Plan mode and outside Plan mode — never embed
   into the plan file, never use Unicode rules, never use 3- or 4-backtick
   fences. 5 backticks is required because the rendered prompt contains
   inner 3-backtick code blocks (PHP/JS/SQL snippets) ; a 4-backtick outer
   fence breaks the moment the prompt contains a 4-backtick block, and 3
   backticks break immediately. 5 backticks gives the VS Code "copy code"
   button on the rendered fence — one click, full prompt copied.

   Output exactly :

   `````
   ✅ Self-contained prompt ready. Use the copy button on the code block below, paste into a fresh Claude Code session.

   ``````
   <rendered prompt>
   ``````
   `````

   (Use **6 backticks** for the outer fence when the prompt itself contains
   a 5-backtick block — escalate as needed. Default is 5.)

   Then stop. No "next-steps" block, no file paths, no collision check, no
   plan-file edit. The prompt lives in chat only.

### 1. Parse invocation and derive identifiers

`/prepare-plan <feature-name> [<description>]` or the natural-language
equivalent. Resolve:

- `feature_name` — kebab-case. If the first whitespace-token already matches
  `[a-z][a-z0-9-]*`, use it. Otherwise derive from the description (lowercase,
  strip stopwords, kebab-case, ≤ 40 chars). Reject `/`, `..`, uppercase, spaces.
- `description` — everything after `feature_name`, trimmed. If empty, ask the
  user for a one-line scope.
- `feature_title` — capitalise each word of `feature_name` (split on `-`).
- `date` — `date +%F` (ISO `YYYY-MM-DD`).
- `first_session_slug` — short kebab-case for the first concrete step. Default
  `scaffold` if you cannot infer better. Examples: `scaffold`, `inventory`,
  `baseline`, `spike`, `apply-shim`.

If `feature_name` is too generic (≤ 3 chars, or a common verb like `fix`,
`add`, `do`), ask for an explicit one. Otherwise print one confirmation line:

```
→ feature_name=<name>, first_session_slug=<slug>, target=/workspace/plans/<name>/
```

### 2. Collision check

`target = /workspace/plans/${feature_name}/`. If it does NOT exist, proceed.
If it exists, NEVER overwrite — propose `-v2`, `-v3`, …, recompute `target`,
and print:

```
⚠ /workspace/plans/<name>/ already exists. Proposing /workspace/plans/<name>-v2/ instead.
  Confirm with "yes" or provide a different name.
```

Wait for explicit confirmation. Never silently fall through — the user may want
to extend the existing plan by hand (not supported here).

### 3. Exploration — fill EXISTING.md

- **Skip** if the scope is confined (single file, named files) — `Read` them
  directly and summarise inline.
- **Spawn 1–3 Explore agents in parallel** if the area is broad or unfamiliar
  (e.g. "refactor the auth layer"). Brief each with a focused question
  (implementations / related components / tests). Thoroughness level: "quick".
- **Default to skip** if you can't articulate a precise question. Vague
  exploration produces noise.

Aggregate into a draft EXISTING.md section. If skipped, EXISTING.md gets a
fill-me stub and session 1 populates it.

### 4. Generate the 5 files

> **Skip this step entirely if Plan mode is active** — disk writes are
> forbidden in Plan mode. Instead, embed the full rendered content of all 5
> files into the current plan file under `## Scaffold spec` with target
> paths annotated. See §"Plan mode integration" below. The actual writes
> happen *after* `ExitPlanMode` approval, as part of post-plan execution.

`ROOT="/workspace/plans/${feature_name}"`. Run `mkdir -p "$ROOT/sessions"`,
then write each file substituting every `{{placeholder}}` from the templates
in §"File templates". Variables:

| Placeholder | Value |
|---|---|
| `{{feature_name}}` | kebab slug |
| `{{feature_title}}` | human title |
| `{{description}}` | scope text |
| `{{date}}` | today ISO |
| `{{first_session_slug}}` | first session slug |
| `{{existing_body}}` | exploration findings, or fill-me stub |

Write order:

1. `$ROOT/ROLLOUT.md`
2. `$ROOT/STATUS.md`
3. `$ROOT/LOG.md`
4. `$ROOT/EXISTING.md`
5. `$ROOT/sessions/session-1-{{first_session_slug}}.md`

### 5. Sanity check — no leftover placeholders

```bash
if grep -RE '<feature[_-]name>|<feature[_-]title>|<first[_-]session[_-]slug>|\{\{[a-z_]+\}\}' "$ROOT" 2>/dev/null; then
  echo "❌ Unresolved placeholders in $ROOT — aborting and removing partial output."
  rm -rf "$ROOT"
  exit 1
fi
```

If grep finds anything, wipe `$ROOT` and stop. Empty result beats broken plan.

### 6. Print the next-steps block

**Render as raw markdown — NOT inside a code fence.** The VS Code extension
renders `[name](path)` as clickable links only when they're not inside a code
block. Paths are **relative to the workspace root** (`plans/<name>/...`),
never absolute (`/workspace/plans/...`), per the project's link convention.

Output exactly this template, substituting `{{feature_name}}` and
`{{first_session_slug}}` — and emit it as plain markdown in the chat, not
wrapped in any fence:

> ✅ Plan scaffolded at [plans/{{feature_name}}/](plans/{{feature_name}}/)
>
> **Files** :
> - [ROLLOUT.md](plans/{{feature_name}}/ROLLOUT.md) — entry point, read first
> - [STATUS.md](plans/{{feature_name}}/STATUS.md) — session scoreboard
> - [LOG.md](plans/{{feature_name}}/LOG.md) — append-only journal (empty)
> - [EXISTING.md](plans/{{feature_name}}/EXISTING.md) — code inventory
> - [sessions/session-1-{{first_session_slug}}.md](plans/{{feature_name}}/sessions/session-1-{{first_session_slug}}.md) — **first session prompt** ← open this and paste its content into a fresh Claude Code chat
>
> To add a session-2 later : edit [STATUS.md](plans/{{feature_name}}/STATUS.md) (new row) and create `sessions/session-2-<slug>.md` following the same shape as session 1.

Critical : the line linking to `session-1` is the only immediate action — keep
it bold + `←` marker so the eye lands on it first. Never print file paths
between single backticks or inside a 4-backtick fence in this block ; both
break click-through.

## Plan mode integration

When the skill is invoked **during Plan mode** (Claude knows via the system
reminder that plan mode is active), it MUST NOT mutate disk. The skill
produces a *spec* that Claude embeds into the current plan file
(`/home/node/.claude/plans/<plan-name>.md`). Actual writes happen *after*
the user approves via `ExitPlanMode`, as part of post-plan execution.

**Per-mode behavior in Plan mode :**

| Mode | What goes into the plan file | Post-`ExitPlanMode` action |
|---|---|---|
| `This session` | Nothing (just the approved plan body). Print the one-line skip message. | Claude implements directly in the current chat. |
| `Fresh chat, prompt only` | Nothing — the rendered prompt is printed directly in chat wrapped in a 5-backtick fence (see §0.5 step 3). The plan file is NOT touched ; the user copies straight from the chat code block. | Nothing automatic — user pastes the prompt elsewhere. |
| `Multi-session scaffold` | Append a `## Scaffold spec` section listing **each of the 5 files with its target path and full rendered content** (e.g. `### plans/<name>/ROLLOUT.md` then the body). | Claude executes the scaffold : `mkdir -p` + 5 `Write` calls + sanity grep + the step 6 next-steps block with clickable links. |

**Hard constraint** : during Plan mode, the skill calls only `Read`,
`AskUserQuestion`, and text output. No `Write`, no `Edit`, no
`Bash mkdir`, no disk mutation of any kind. The Plan mode runtime allows
only the plan file as a write target, and that write is mediated by Claude
embedding the spec into it.

**Detection cue** : if you see "Plan mode is active" in a system reminder,
or you've been auto-invoked at Phase 4 of the Plan mode workflow, you are
in Plan mode. Default to deferred behavior whenever in doubt.

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

I'm starting session 1 of the `{{feature_name}}` rollout.

Entry point : `/workspace/plans/{{feature_name}}/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory)

Goal : {{description}}

First session focus : {{first_session_slug}}. Concretely — propose the
shape, validate with me, then implement the first concrete step. Keep
scope tight ; if you discover follow-up work, add a session-2 row to
STATUS.md instead of folding it in.

DoD at the end of this session :
1. STATUS.md : flip session 1 row 📋 → ✅, prompt link → —, bump
   Delivered counter (0→1), refresh "Next focus" (to `rollout complete`
   for a single-session rollout, or `session 2 — to be defined` for a
   multi-session one).
2. LOG.md : append `## 1 — {{first_session_slug}}` section dated today
   with files touched + What / Why / Decisions / Gotchas / Tests /
   Commit.
3. EXISTING.md : update if new files / structures were created.
4. Propose a commit (do NOT commit without explicit user confirmation).
````

> The session file IS the prompt — no wrapper, no fence. The user copies the
> file's full content into a fresh chat. Effort estimates, prerequisites, and
> per-session metadata belong in STATUS.md (extra columns if needed), not here.

### Template — `prompt-only` mode (no file, displayed in chat or embedded in plan)

````markdown
# {{feature_title}}

Goal : {{description}}

Approach :
{{plan_approach}}

DoD :
1. Implement the change described above.
2. Verify : {{tests}}
3. Propose a commit (do NOT commit without explicit user confirmation).
````

Fallback when `{{plan_approach}}` is unavailable (skill invoked outside Plan
mode, no upstream design captured) : replace the *Approach* block with the
single line `_(no approach captured — first session will design it)_`.

Fallback when `{{tests}}` is unavailable : replace with the generic line
`existing tests pass; new behavior verified end-to-end`.

The user copies this prompt verbatim into a fresh Claude Code chat — no
ROLLOUT/STATUS/LOG references, no scoreboard to maintain. Pure one-shot
context.

## Constraints

- Output path for scaffolded plans is always `/workspace/plans/<feature_name>/`.
  Never elsewhere.
- Never overwrite an existing plan directory — propose `-v2`, `-v3`, …
- `feature_name` must match `[a-z][a-z0-9-]*`. Reject `/`, `..`, uppercase,
  spaces.
- Every generated file has resolved placeholders. Step 5 enforces this.
- Generated files are in **English** per the project's CLAUDE.md rule, even
  when the conversation is in French.
- Multi-session scaffold writes **exactly 5 files** (4 top-level + 1 session-1).
  No `docs/`, no extra sessions — added on demand by later sessions.
- `AskUserQuestion` is called ONCE at step 0. Never a second time.
- Mode **"This session"** writes zero files and prints one line.
- Mode **"Fresh chat, prompt only"** writes zero files in all cases. The
  rendered prompt is ALWAYS printed in chat wrapped in a 5-backtick fence —
  identical behavior in or out of Plan mode. Never embed into the plan file.
- **Inside Plan mode**, the skill writes ONLY through Claude into the plan
  file. No direct disk mutation (no `Write`, no `Edit`, no `Bash mkdir`).
  Disk writes are deferred to post-`ExitPlanMode` execution.
- The final message after a Multi-session scaffold uses **clickable markdown
  links with workspace-relative paths** (`plans/<name>/...`), never inside a
  code fence — see step 6.

## Failure modes

| Symptom | Cause | Mitigation |
|---|---|---|
| `feature_name` directory already exists | Repeat invocation or pre-existing plan | Refuse, propose `-v2`, wait for user confirmation. Never overwrite. |
| Derived `feature_name` too generic (≤ 3 chars, common verb) | Description was too vague | Ask the user for an explicit name before continuing. |
| Exploration agent times out or returns nothing | Scope mis-bounded for Explore | Fall back to the stub EXISTING.md. The first session can fill it. |
| Sanity-check at step 5 finds a `{{placeholder}}` | Substitution missed a token in the templates | Wipe the output directory and abort. Better empty than broken. |
| User picks "This session" at step 0 | Scope small enough that scaffolding is overhead | Print "Plan scaffold skipped — implementing in current session." and stop. No files. Return control so Claude implements now. |
| User picks "Fresh chat, prompt only" but no plan approach was captured | Skill invoked outside Plan mode, no upstream design | Use the fallback `Approach` line in the template, warn the user the prompt is thin. |
| Skill invoked during Plan mode | Plan mode active, disk writes forbidden | Produce the spec into the plan file (see §"Plan mode integration"); defer the actual scaffold to post-`ExitPlanMode` execution. Never call `Write` / `Edit` / `Bash mkdir` during Plan mode. |
| User wants to extend an existing plan instead of starting `-v2` | This skill does NOT support append mode | Tell the user to edit `STATUS.md` / `LOG.md` / `sessions/` by hand. |
| Recommendation feels wrong to the user | Scope judgment was off | The user simply picks a different option in the AskUserQuestion. The recommendation is non-binding. |

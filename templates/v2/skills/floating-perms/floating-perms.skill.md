---
description: Grant a temporary batch of permissions in settings.local.json (session-scoped, optional TTL). Auto-triggered by Claude after the PreToolUse hook detects a spike of permission prompts, or invoked manually by the user. Auto-revoked at SessionEnd or TTL expiry. Audit log at .devcontainer/notify/floating-perms-audit.jsonl. MANDATORY workflow: ANALYZE → ASK via AskUserQuestion → EXECUTE → RETRY. Never call apply.js batch without an explicit AskUserQuestion confirmation right before it.
argument-hint: "batch <pat1> <pat2>... [ttl=15m] sid=<id>  |  list [sid=<id>]  |  revoke <pat>  |  gc sid=<id>"
---

# /floating-perms — batched session-scoped permissions

## When this skill is triggered

**Automatic trigger.** The `PreToolUse` hook watches for permission-requiring
tool calls. When **3 prompts arrive within 120 s** — regardless of which
patterns — the 3ʳᵈ is **denied** with a `permissionDecisionReason` that lists
every unique pattern from the window. Rationale: repeated prompts are
draining whatever the command, so we batch. You will see something like:

> STOP — floating-perms: 3 permission prompts in under 120s. Repeated
> prompts are draining whatever the command, so we batch.
>
> Patterns seen in the recent window:
>   - `Bash(curl:*)`
>   - `Edit(/tmp/scratch/**)`
>   - `Read(/home/node/**)`
>
> Mandatory workflow before any further tool call: 1. ANALYZE, 2. ASK via
> AskUserQuestion, 3. EXECUTE /floating-perms batch ... sid=<id>, 4. RETRY
> the denied tool.

When you receive that, **stop** the tool retry. Plan first.

Patterns already covered by `permissions.allow` don't count toward the
spike (otherwise the user would be warned about things they already
authorized). The match tolerates `Bash(cmd:*)`, `Bash(cmd*)` and `Bash(cmd *)`
historical forms. For a Bash command already in allow at the command
level, the canonicalizer falls back to a `Read(<path>/**)` pattern when
the command targets a path outside `/workspace` — that captures the
"grep is allowed but /tmp/scratch isn't" case. Paths under `/workspace`
are skipped entirely (they're auto-allowed by Claude Code's cwd default).

**Manual trigger.** The user typed `/floating-perms <subcmd>` directly, or
asked things like "allow curl + npm for this session" / "session-scope
these commands" / "kill all session perms".

## Mandatory workflow when auto-triggered

### Step 1 — ANALYZE

Re-read the current task (look at recent conversation context if needed)
and enumerate **exhaustively** every Bash command + file path you expect
to touch before the task is done. Better one over-permissive batch than
three triggers.

Canonical pattern shapes:
- `Bash(<cmd>:*)`              — e.g. `Bash(curl:*)`, `Bash(npm view:*)`
- `Edit(<dir>/**)`             — e.g. `Edit(/home/node/.config/**)`
- `Write(<dir>/**)`            — e.g. `Write(/var/tmp/**)`
- `Read(<dir>/**)`             — e.g. `Read(/tmp/scratch/**)`
- `NotebookEdit(<dir>/**)`     — same shape

The deny reason already tells you which patterns triggered the spike (and
gives you the canonical forms copy-paste-ready); include them + everything
else you anticipate.

### Step 2 — ASK (mandatory AskUserQuestion call)

**This step is non-negotiable.** Never call `apply.js batch` without an
immediately preceding `AskUserQuestion` that lists the exact patterns
you want to grant. The user must see "authorize THIS and THAT" — not a
generic "want me to widen perms?".

Recommended shape:

```
question: "<short context of what you're trying to do, e.g. 'upstream upgrade fetch'>.
           Permissions needed: <pat1>, <pat2>, <pat3>. Which grant?"
header:   "floating-perms"
options:
  - label: "Allow all until SessionEnd  (Recommended)"
    description: "Grants Bash(curl:*), Bash(npm view:*), Edit(/home/node/.config/**).
                  Auto-revoked at /exit."
  - label: "Allow all, TTL 15m"
    description: "Same patterns, expires in 15 minutes"
  - label: "Subset (specify which)"
    description: "You'll tell me which ones to keep"
  - label: "Refuse — I'll change approach"
    description: "No grant; I'll find another way without these perms"
```

You may split into 2-3 questions if the batch is heterogeneous (e.g. one
question per tool family). Stay under 4 questions per AskUserQuestion call
— that's the hard limit.

If the user picks "Subset", follow up with a second AskUserQuestion that
multi-selects which specific patterns to keep, before calling apply.js.

### Step 3 — EXECUTE (only after explicit user choice)

Based on the user's answer, run:

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js \
     batch <pattern1> <pattern2> ... sid=<session_id> [ttl=15m]
```

**CRITICAL**: the `triggerPattern` from the deny reason (the one named
in "RETRY — re-run the tool call that was just denied (`<pattern>`)")
**MUST** be in the batch. If you leave it out, the Step 4 retry will hit
a fresh permission prompt and the whole point of the workflow is lost.
Same for every other pattern the window listed — if the user said "allow
all", include all of them.

The `sid` is the current `session_id` — embedded literally in the hook's
deny reason (e.g. `sid=abc12345`), and available as `payload.session_id`
in any current/recent hook input. Copy it directly.

Read stdout — that's the grant report (granted / skipped / blocked /
invalid). Resurface a clean summary to the user, then retry the originally
denied tool call.

### Step 4 — RETRY

After the grant lands, the previously denied tool call can now proceed.
Retry it. If the user chose "Refuse", **do not retry** — generate an
alternative plan that doesn't need those perms (e.g. don't fetch upstream
notes, work from cached data).

## What to do when manually triggered

The user passed arguments. Forward them straight to the script:

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js <subcmd> <args...>
```

Subcommands:

| Subcommand | Use |
|------------|-----|
| `batch <pat1> [pat2 …] [ttl=<duration>] sid=<id>` | Grant N patterns for the session. TTL optional. |
| `list [sid=<id>]` | Show active grants. |
| `revoke <pattern>` | Remove a single pattern across all sessions. |
| `gc sid=<current_sid>` | Revoke state-side grants whose sid != current (orphans from sessions whose SessionEnd never fired). |
| `reconcile sid=<current_sid> [--auto]` | Detect entries in settings.local.json that look floating but have no matching state.json grant. Interactive by default; pass `--auto` to revoke without confirmation. |

Duration syntax for `ttl=`: `15m`, `2h`, `1d`, or bare integer = seconds.

Read stdout — that's the report (granted / skipped / blocked / invalid).
Re-emit a clean summary to the user.

For a manual `batch`, the user is the one making the call explicit, so
the AskUserQuestion step is implicit (their command IS the answer). Still
echo back what's about to be granted before running, in case of typos.

## Examples

### Auto-triggered batch — full workflow

```
hook deny:  STOP — 3 prompts in 90s. Patterns: Bash(curl:*),
            Edit(/home/node/.config/**), Read(/tmp/scratch/**).
            Workflow: ANALYZE → ASK → EXECUTE sid=abc12345 → RETRY.

claude:     [Step 1 — ANALYZE]
            (re-reads task: "fetch upstream release notes + patch local config")
            Anticipated patterns:
              - Bash(curl:*)               — fetch release notes
              - Bash(jq:*)                 — parse JSON
              - Edit(/home/node/.config/**) — patch local config
              - Read(/tmp/scratch/**)      — read intermediate output

            [Step 2 — ASK]
            → AskUserQuestion:
              "Upstream upgrade fetch. Permissions needed: Bash(curl:*),
               Bash(jq:*), Edit(/home/node/.config/**), Read(/tmp/scratch/**).
               Which grant?"
              · Allow all until SessionEnd  (Recommended)
              · Allow all, TTL 15m
              · Subset (specify which)
              · Refuse — I'll change approach

user:       picks "Allow all until SessionEnd"

claude:     [Step 3 — EXECUTE]
            [Bash] node apply.js batch \
              Bash(curl:*) Bash(jq:*) Edit(/home/node/.config/**) \
              Read(/tmp/scratch/**) sid=abc12345
            ✓ 4 pattern(s) granted [sid abc12345 · until SessionEnd]

            [Step 4 — RETRY]
            [re-runs the originally denied curl call; passes now]
```

### Manual TTL

```
user:       /floating-perms batch Bash(gh:*) ttl=30m sid=abc12345
claude:     [runs apply.js]
            ✓ 1 pattern(s) granted [sid abc12345 · expires 2026-06-12T19:30:00Z]
```

### List + manual revoke

```
user:       /floating-perms list
claude:     [runs] node apply.js list
            Active grants (2):
              - Bash(curl:*)  [sid abc12345 · expires: until SessionEnd]
              - Bash(gh:*)    [sid abc12345 · expires: 2026-06-12T19:30:00Z]

user:       /floating-perms revoke Bash(gh:*)
claude:     [runs] node apply.js revoke Bash(gh:*)
            Revoked 1 grant(s) for Bash(gh:*).
```

### Reconcile orphans

```
session_start additionalContext:
  floating-perms — SessionStart reconciliation report:
  Allow-side orphans (2) — entries in settings.local.json with no matching state.grants record:
    - `Bash(npm:*)`  [pre-V1.2 form]
    - `Bash(node:*)`  [pre-V1.2 form]
  Resolution:
    - Allow-side: /floating-perms reconcile sid=abc12345 to inspect, then re-run with --auto to clean.

user:       /floating-perms reconcile sid=abc12345
claude:     [runs] node apply.js reconcile sid=abc12345
            Found 2 orphan floating-form entry/entries in settings.local.json:
              - Bash(npm:*)  [pre-V1.2 heuristic]
              - Bash(node:*) [pre-V1.2 heuristic]
            To revoke them all, re-run with --auto:
              /floating-perms reconcile sid=abc12345 --auto

claude:     [asks user via AskUserQuestion: "Revoke these 2 orphans? Allow / Skip / Keep one"]
user:       picks "Revoke all"
claude:     [runs] node apply.js reconcile sid=abc12345 --auto
            ✓ Revoked 2 orphan(s) from settings.local.json.
```

## What the sentinels mean

Starting V1.2, `apply.js` and `cleanup.js` insert two marker strings into
`permissions.allow` to bracket the floating-perms managed section:

```
"// ──────── floating-perms managed below — auto-revoked at SessionEnd ────────"
…floating entries…
"// ──────── end floating-perms ────────"
```

These are string array entries (not JSON comments — Claude Code doesn't
support JSONC), but they don't begin with any tool prefix (`Bash`, `Read`,
`Edit`, …) so the permission engine has nothing to match them against.
Effective no-ops. They survive `JSON.stringify` round-trips. The user can
scan the file and immediately tell which entries are managed.

If you ever see one sentinel without its pair, the writer auto-heals on
the next mutation — both sentinels are filtered on every write and
re-emitted from scratch.

## Orphan reconciliation

Two failure modes leave stale entries in `permissions.allow`:

1. **SessionEnd never fired** (process crash, kill -9, container yanked).
   `state.json` still has the grants but no `SessionEnd` event will revoke
   them. Detected by `gc` (state-side, grants whose `sid !== current`).
2. **`state.json` was lost** (rm, container rebuild, corruption). The
   `permissions.allow` entries remain but the state has no record of them.
   `cleanup.js` is state-driven so it will never see them. Detected by
   `reconcile` (allow-side, canonical-form entries with no state match).

Both failure modes are surfaced at SessionStart via `additionalContext`,
and the user is pointed at the right subcommand (`gc` for state-side,
`reconcile` for allow-side).

- **AskUserQuestion is mandatory before any auto-triggered grant.** No
  silent batch grants. The user must see the exact patterns spelled out
  in the question's options. If the user types `/floating-perms batch ...`
  manually, the explicit command itself IS the confirmation — still echo
  what will be granted before running.
- **Blocklist is non-negotiable.** `rm`, `sudo`, `chmod`, and friends, plus
  system paths (`/etc/**`, `/usr/bin/**`, `/root/**`, etc.) are refused
  even if the user asks. Reason printed in the report.
- **Canonical state file** at `/workspace/.devcontainer/notify/floating-perms-state.json`.
  `settings.local.json` is a mirror; never extended with a non-standard
  schema.
- **Audit log append-only** at `/workspace/.devcontainer/notify/floating-perms-audit.jsonl`.
  Every grant + revoke logged with timestamp, sid, reason.
- **One-shot deny per spike** — the hook clears the counter after emitting
  the deny so you're not paralyzed. If you ignore the reason and trigger
  a new spike (3 more prompts after the 60s cooldown), a new deny fires.
- **Cwd paths skipped** — Edit/Write/Read on `/workspace/**` never count
  toward the spike (auto-allowed by Claude Code's default sandbox).
- **No automatic cleanup of `.allow` entries not in `_state`.** If the
  user has hand-edited `permissions.allow`, those entries are not touched
  by SessionEnd cleanup. Only patterns this skill granted are revoked.

## Failure modes

- **Hook can't read state file** → returns silently, no deny. Worst case:
  user still sees the next prompt and can grant manually.
- **Lock contention** → 6 retries with jitter, then proceeds without lock
  (state read is best-effort). Two simultaneous grants in this window may
  duplicate-write — apply.js skips already-allowed patterns so the dup is
  harmless; state.grants may have two entries for one pattern but cleanup
  tolerates that.
- **SessionEnd didn't fire** (process crash, kill -9) → next SessionStart
  surfaces the orphans via additionalContext, user decides via
  `/floating-perms gc sid=<current>`.

## Files in scope

- Reads: `/workspace/.claude/settings.local.json`, state + audit files.
- Writes: `/workspace/.claude/settings.local.json` (only `permissions.allow`),
  state file, audit JSONL.
- Never writes: `~/.claude/settings.json` (global), any other config.

## Related skills

- `fewer-permission-prompts` — retrospective scan over past transcripts,
  produces a permanent allowlist suggestion. Cousin skill: permanent
  vs ephemeral.
- `update-config` — owns settings.json mutations more broadly. This skill
  bypasses it because the operation is mechanical and tightly scoped.

---
description: Grant a temporary batch of permissions in settings.local.json (session-scoped, TTL-bounded). Auto-triggered by Claude after the PreToolUse hook detects a spike of permission prompts, or invoked manually by the user. For Bash grants, writes `permissions.allow` (canonical `Bash(cmd:*)`). For Edit/Write/Read/NotebookEdit grants on paths outside cwd, ALSO writes `permissions.additionalDirectories` — Claude Code refuses file operations outside cwd + additionalDirectories regardless of what's in `allow`. Grants expire after TTL (default 30 min) — SessionEnd cleanup is a best-effort second layer since it doesn't reliably fire on VS Code shutdown. Audit log at .devcontainer/notify/floating-perms-audit.jsonl. MANDATORY workflow: ANALYZE → ASK via AskUserQuestion → EXECUTE → RETRY. Never call apply.js batch without an explicit AskUserQuestion confirmation right before it.
argument-hint: "batch <pat1> <pat2>... [ttl=30m] sid=<id>  |  list [sid=<id>]  |  revoke <pat>  |  gc sid=<id>"
---

# /floating-perms — batched session-scoped permissions

## When this skill is triggered

**Automatic trigger.** Claude Code's `PermissionRequest` hook fires for
every tool call that would prompt the user. When **2 prompts arrive
within 120 s** in a session, the next tool call that reaches `PreToolUse`
is **denied** with a `permissionDecisionReason` listing every unique
pattern from the window — *before* the third permission dialog ever
fires. The user has paid attention cost twice, not a third time.
Rationale: repeated prompts are draining whatever the command, so we
batch. You will see something like:

> STOP — floating-perms: 2 permission prompts in under 120s. Repeated
> prompts are draining whatever the command, so we batch.
>
> Patterns seen in the recent window:
>   - `Bash(curl:*)`
>   - `Edit(/tmp/scratch/**)`
>
> Mandatory workflow before any further tool call: 1. ANALYZE, 2. ASK via
> AskUserQuestion, 3. EXECUTE /floating-perms batch ... sid=<id>, 4. RETRY
> the denied tool.

When you receive that, **stop** the tool retry. Plan first.

Because the counter is fed only by `PermissionRequest`, patterns
already covered by `permissions.allow` are by construction never
counted — Claude Code doesn't fire the hook for them. Meta tools
that don't represent work (`ExitPlanMode`, `AskUserQuestion`,
`TodoWrite`, `Task`, MCP) also never count: they canonicalize to
`null` and the observer skips them silently. The deny message lists
the unique work patterns from the recent window so you can copy
them into the `apply.js batch` call verbatim.

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

**File-tool dirs outside cwd get injected into `additionalDirectories`
automatically.** When you grant `Write(/tmp/foo/**)`, `apply.js` also adds
`/tmp/foo` to `permissions.additionalDirectories` because Claude Code refuses
file writes/reads outside cwd + additionalDirectories regardless of what's in
`allow`. Dirs already under `/workspace/**` (cwd) or already present in
`additionalDirectories` are skipped — no churn. Bash grants only touch `allow`.

The deny reason already tells you which patterns triggered the spike (and
gives you the canonical forms copy-paste-ready); include them + everything
else you anticipate.

### Step 2 — ASK (mandatory AskUserQuestion call)

**This step is non-negotiable.** Never call `apply.js batch` without an
immediately preceding `AskUserQuestion` that lists the exact patterns
you want to grant. The user must see "authorize THIS and THAT" — not a
generic "want me to widen perms?".

**VERBATIM rule.** Every pattern listed in the deny reason MUST appear
**verbatim** as a grantable option in the `AskUserQuestion` you build.
You MAY add anticipated patterns. You MUST NOT replace canonical-form
patterns with rewordings, "fixed" forms, or a different tool family.
If the deny lists `Read(/tmp/**)`, the question must propose
`Read(/tmp/**)` — not `Bash(cat:*)`, not a narrower
`Read(/tmp/extensions.js)`.

**DISCLOSE additionalDirectories side-effect.** For every file-tool
pattern (Edit/Write/Read/NotebookEdit(<dir>/**)) whose `<dir>` is NOT
under the cwd (currently `/workspace`), the grant will ALSO inject
`<dir>` into `permissions.additionalDirectories`. This is not optional
— Claude Code refuses file operations outside cwd + additionalDirectories
regardless of the `allow` entry. Users MUST see this side-effect in the
`AskUserQuestion` description so they consent knowingly. Explicitly list
the injected dirs in the option description, e.g.
`"...and adds /home/node/.config to additionalDirectories"`.

Recommended shape:

```
question: "<short context of what you're trying to do, e.g. 'upstream upgrade fetch'>.
           Permissions needed: <pat1>, <pat2>, <pat3>. Which grant?"
header:   "floating-perms"
options:
  - label: "Allow all (default TTL 30m)  (Recommended)"
    description: "Grants Bash(curl:*), Bash(npm view:*), Edit(/home/node/.config/**).
                  Also adds /home/node/.config to additionalDirectories
                  (required — outside cwd). Auto-revoked after 30 minutes."
  - label: "Allow all, longer TTL (e.g. ttl=2h)"
    description: "Same patterns + additionalDirectories injection,
                  custom expiry — use for tasks longer than the default"
  - label: "Subset (specify which)"
    description: "You'll tell me which ones to keep"
  - label: "Refuse — I'll change approach"
    description: "No grant; I'll find another way without these perms"
```

If none of the file-tool patterns need additionalDirectories injection
(all under cwd, or all Bash-only), drop that sentence from the
description — don't add ceremony where none is needed.

You may split into 2-3 questions if the batch is heterogeneous (e.g. one
question per tool family). Stay under 4 questions per AskUserQuestion call
— that's the hard limit.

If the user picks "Subset", follow up with a second AskUserQuestion that
multi-selects which specific patterns to keep, before calling apply.js.

### Step 3 — EXECUTE (only after explicit user choice)

Based on the user's answer, run:

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js \
     batch <pattern1> <pattern2> ... sid=<session_id> [ttl=<duration>]
```

Omitting `ttl=` applies the default (30 minutes). Pass `ttl=2h` (or
similar) only when the user asked for a longer-lived grant.

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
| `batch <pat1> [pat2 …] [ttl=<duration>] sid=<id>` | Grant N patterns for the session. Default TTL 30 min. |
| `list [sid=<id>]` | Show active grants. |
| `revoke <pattern>` | Remove a single pattern across all sessions. |
| `gc sid=<current_sid>` | Revoke state-side grants whose sid != current (orphans from previous sessions that didn't run SessionEnd cleanup). |
| `reconcile sid=<current_sid> [--auto]` | Detect entries in settings.local.json that look floating but have no matching state.json grant. Interactive by default; pass `--auto` to revoke without confirmation. |

Duration syntax for `ttl=`: `15m`, `30m` (default), `2h`, `1d`, or bare integer = seconds.

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
              · Allow all (default TTL 30m)  (Recommended)
              · Allow all, longer TTL (e.g. ttl=2h)
              · Subset (specify which)
              · Refuse — I'll change approach

user:       picks "Allow all (default TTL 30m)"

claude:     [Step 3 — EXECUTE]
            [Bash] node apply.js batch \
              Bash(curl:*) Bash(jq:*) Edit(/home/node/.config/**) \
              Read(/tmp/scratch/**) sid=abc12345
            ✓ 4 pattern(s) granted [sid abc12345 · expires 2026-06-12T20:00:00Z (TTL 1800s)]

            [Step 4 — RETRY]
            [re-runs the originally denied curl call; passes now]
```

### Auto-triggered batch — dir-only (Read spike on /tmp)

```
hook deny:  STOP — 2 prompts in 8s. Patterns: Read(/tmp/**).
            Workflow: ANALYZE → ASK → EXECUTE sid=abc12345 → RETRY.

claude:     [Step 1 — ANALYZE]
            Both prompts were Read on files directly under /tmp.
            Anticipated patterns:
              - Read(/tmp/**)              (verbatim from deny)
              - Write(/tmp/**)             (likely follow-up)

            [Step 2 — ASK]
            → AskUserQuestion:
              "Inspecting files under /tmp. Permissions needed:
               Read(/tmp/**), Write(/tmp/**). Which grant?"
              · Allow both (default TTL 30m)  (Recommended)
              · Allow Read(/tmp/**) only
              · Refuse — I'll work from in-memory data
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
              - Bash(curl:*)  [sid abc12345 · expires: 2026-06-12T20:00:00Z]
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
"// ──────── floating-perms managed below — auto-revoked on TTL expiry ────────"
…floating entries…
"// ──────── end floating-perms ────────"
```

The pre-V1.2 form of the start sentinel (`… auto-revoked at SessionEnd …`)
is still recognized on read for backward compat, and rewritten to the new
form on the next mutation.

These are string array entries (not JSON comments — Claude Code doesn't
support JSONC), but they don't begin with any tool prefix (`Bash`, `Read`,
`Edit`, …) so the permission engine has nothing to match them against.
Effective no-ops. They survive `JSON.stringify` round-trips. The user can
scan the file and immediately tell which entries are managed.

If you ever see one sentinel without its pair, the writer auto-heals on
the next mutation — both sentinels are filtered on every write and
re-emitted from scratch.

**`permissions.additionalDirectories` has NO sentinel** — it's a plain
array of absolute paths, no entries the permission engine ignores. Tracking
is state-driven instead: each grant stores its `additional_dir` in
`state.json`, and cleanup removes the dir from `additionalDirectories` only
if no remaining grant still needs it. If `state.json` is lost, injected
dirs stay in the file until the user removes them manually — same failure
mode class as pre-V1.2 allow-list orphans, currently not covered by
`reconcile`.

## Orphan reconciliation

Two failure modes leave stale entries in `permissions.allow`:

1. **SessionEnd never fired** (process crash, kill -9, container yanked,
   VS Code window closed with the ✕ — the CLI runs in an integrated
   terminal and the extension's deactivate is a no-op, so SessionEnd is
   not reliably invoked on shutdown). The default 30 min TTL caps the
   damage, but grants still active at the moment of a hard shutdown
   remain in `state.json` until either they expire (removed on next
   `revokeExpired` tick) or `gc` finds them (state-side, grants whose
   `sid !== current`).
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
- **SessionEnd didn't fire** (process crash, kill -9, VS Code force-close)
  → the 30 min default TTL will still expire the grant on the next hook
  tick; any still-active grants at the moment of the next session are
  surfaced as orphans via SessionStart additionalContext, user decides
  via `/floating-perms gc sid=<current>`.

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

# Single-use token — floating-perms repro

`````
Fais ces trois choses à la suite (sans me demander confirmation
entre chaque, enchaîne-les) :

  1. Affiche "one" avec python3 : `python3 -c "print('one')"`.
  2. Affiche la version de wget : `wget --version | head -1`.
  3. Affiche "two" avec python3 : `python3 -c "print('two')"`.

Une fois les trois exécutés, résume-moi les sorties.
`````

---

## Why this prompt works

A trivial three-step task where every command targets a tool that
is **not** in `/workspace/.claude/settings.local.json`
(`python3`, `wget`). Claude sees them as independent tool calls
and doesn't know they're a trap for the spike detector.

Sequence of events:

1. **1st call** `python3 -c "print('one')"` → Claude Code prompts
   for `Bash(python3:*)`. User picks Allow-once. Hook counter for
   the session grows to 1.
2. **2nd call** `wget --version | head -1` → prompt for
   `Bash(wget:*)`. User picks Allow-once. Counter grows to 2 —
   threshold reached.
3. **3rd call** (`python3 -c "print('two')"`) → PreToolUse hook
   fires the deny **before** the dialog opens. Claude receives a
   `permissionDecisionReason` of the form:

   ```
   STOP — floating-perms: 2 permission prompts in under 120s...
   Patterns:
     - Bash(python3:*)
     - Bash(wget:*)
   A single-use grant token has been reserved for this spike:
     id=XXXXXXXX   session=YYYYYYYY   patterns=2   valid_for=300s
   Workflow: ANALYZE → ASK → EXECUTE Skill(...) → RETRY.
   ```

4. Claude must then run the mandatory workflow:
   - **ANALYZE**: patterns cover the third command, no coverage
     gap → straight to ASK.
   - **ASK**: an `AskUserQuestion` listing verbatim
     `Bash(python3:*)` and `Bash(wget:*)`.
   - **EXECUTE**: after user approves, Claude calls
     `Skill(skill="floating-perms", args="allow id=XXX session=YYY")`
     — NOT a bare `[Bash] node apply.js ...` (which would prompt
     the mirror for the node invocation and miss the point of the
     wrapper).
   - **RETRY**: the third python3 call re-runs and passes without
     a fresh prompt.

## What to check afterwards

- **`.claude/settings.local.json`** now contains
  `Bash(python3:*)` and `Bash(wget:*)` inside the floating-perms
  sentinel block, with a fresh TTL.
- **`.devcontainer/notify/floating-perms-audit.jsonl`** shows, in
  order, `permission_seen ×2`, `spike_detected` (with
  `pending_id: <id>`), `allow_consumed` (matching that id), and
  `grant` (matching the two patterns).
- **`.devcontainer/notify/floating-perms-state.json`** →
  `pending_grants` is `{}` (the token was consumed), `grants`
  contains the two entries.

## Failure modes to spot

- **Claude runs `[Bash] node apply.js allow ...` directly** (not
  through `Skill`) → the mirror prompts for the node invocation.
  The token still works but the workflow leaks an extra prompt.
  Signal: fix the `denyReason` wording so the `Skill(...)` form is
  more prominent.
- **Claude ignores the token and reaches for `batch`** → wasted
  token, spike counter reset for nothing. Signal: the deny reason
  needs to make clear the token is the primary path.
- **`allow` refused with `unknown or already-consumed`** → the
  token was replaced by a stale re-fire, or the hook didn't write
  the pending entry. Check `pending_grants` in state.json at the
  moment of the third call.
- **`allow` refused with `session_mismatch`** → Claude copied a
  stale `session=` from an older transcript. Check the deny reason
  the hook emitted THIS spike.

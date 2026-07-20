# Prompt 6 — Single-use token + Skill wrapper

**Categories**: H1, H2, H3, H4

**Goal**: verify the new single-use token flow end-to-end (id
reserved at deny, consumed via `Skill(...)` rather than direct Bash
on apply.js, re-consumption refused, cross-session refused).

## Prompt to paste (fresh chat)

```
Fais ces trois choses à la suite, un tool call à la fois, séquentiel :

1. Bash : `python3 -c "print('one')"`
2. Bash : `wget --version | head -1`
3. Bash : `python3 -c "print('two')"`

Résume les trois sorties une fois terminé.
```

## Expected

- Steps 1-2 → popups for `Bash(python3:*)` then `Bash(wget:*)`.
  Allow-once. Counter → 2.
- Step 3 → PreToolUse deny BEFORE the popup appears. The deny
  reason must contain:
  - **H1a** — "Patterns seen" block: `Bash(python3:*)`,
    `Bash(wget:*)` (verbatim, canonical).
  - **H1b** — "A single-use grant token has been reserved" block:
    - `id=<8-hex-chars>` (nano-UUID)
    - `session=<current-sid>`
    - `patterns=2`
    - `valid_for=300s`
  - **H1c** — Step 3 EXECUTE mentions
    `Skill(skill="floating-perms", args="allow id=... session=...")`,
    **NOT** a direct `node .../apply.js allow ...`.
- Claude workflow:
  - **ANALYZE** short (patterns from deny cover the 3rd command).
  - **ASK** via AskUserQuestion listing the 2 patterns verbatim.
  - **EXECUTE** — **CRITICAL (H2)**: Claude calls the `Skill` tool
    directly, NOT `[Bash] node apply.js allow ...`. Failure
    signal: an additional Claude Code prompt for `Bash(node ...:*)`
    before the grant → routing missed → tighten the deny wording.
  - **RETRY**: the 3rd `python3` passes silently (auto-allow via
    the freshly-granted `Bash(python3:*)` in the mirror).

## Verify (right after the grant)

```bash
tail -30 .devcontainer/notify/floating-perms-audit.jsonl \
  | jq -c 'select(.event | test("spike|allow_|grant"))
           | {event, id: (.pending_id // .id),
              sid: ((.sid // .session) | .[0:8]),
              reason, patterns}'
```
Expected, in order:
- `spike_detected` with `pending_id: <id>`.
- `allow_consumed` with `id: <same-id>` and `patterns` =
  `[Bash(python3:*), Bash(wget:*)]`.
- `grant` with the same two patterns.

```bash
jq '.pending_grants' /workspace/.devcontainer/notify/floating-perms-state.json
```
Expected: `{}` (token consumed, single-use purge).

## Extra check (H3 — single-use enforcement)

Copy the `id` from the deny reason above, then:

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js allow \
  id=<copied-id> session=<copied-sid>
```
Expected:
- Exit code 2.
- stderr: `unknown or already-consumed id: <id>`.
- New `allow_refused` in audit with
  `reason: unknown_or_consumed`.

## Extra check (H4 — session-bind enforcement)

In a new session (fresh chat), trigger a spike (Steps 1-2 of the
same prompt) to get an `id`. Copy the id, then from a separate
shell:

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js allow \
  id=<new-id> session=fake-sid-mismatch
```
Expected:
- Exit code 2.
- stderr: `belongs to a different session`.
- `allow_refused` with `reason: session_mismatch`.
- The pending grant is **PRESERVED** (not consumed) — the real
  session can still use it.
- Check: `jq '.pending_grants' state.json` shows the id still there.

## Cleanup

```bash
node /workspace/.devcontainer/skills/floating-perms/apply.js revoke 'Bash(python3:*)'
node /workspace/.devcontainer/skills/floating-perms/apply.js revoke 'Bash(wget:*)'
```

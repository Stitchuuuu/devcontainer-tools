# Ready-to-paste repro prompt

Paste the block below into a fresh Claude Code session **after**
this branch is merged (or after `sync-skills.sh` rebuilds settings
so `PermissionRequest` is wired into `hook.js`).

The three commands are picked relative to the current
`/workspace/.claude/settings.local.json` allow-list — none of them
match it, none is in the baseline at
`/workspace/.devcontainer/claude/settings.local.json`, none is in
the blocklist. Each one fires a `PermissionRequest` regardless of
whether the binary is actually installed (Claude Code prompts
before executing). After the 3rd, any further tool call is denied
by floating-perms.

---

```
Please run these three Bash commands in sequence, without asking
me anything between them. Just execute them one after the other —
even if I'm prompted by Claude Code for permission, approve each
individually and move on:

  1. nmap --version
  2. strace --version
  3. dig -v

Then try one more tool call of your choice (e.g. `ls /tmp`). I
expect that fourth tool call to be denied by the floating-perms
spike detector with a STOP message listing the three patterns
above.
```

---

## Expected behaviour

1. Claude prompts the user three times — once per command. Each
   approval fires `PermissionRequest`, the hook pushes
   `{ ts, pattern, tool_use_id }` into `state.counters[sid]` and
   appends a `permission_seen` line to the audit log.
2. On the fourth tool call (any tool, any pattern), `PreToolUse`
   sees three entries in the 120 s window, the race-window guard
   is clean, and the hook emits:
   ```json
   {
     "hookSpecificOutput": {
       "hookEventName": "PreToolUse",
       "permissionDecision": "deny",
       "permissionDecisionReason": "STOP — floating-perms: 3 permission prompts in under 120s. …"
     }
   }
   ```
   The reason lists `Bash(nmap:*)`, `Bash(strace:*)`, `Bash(dig:*)`.
3. `state.counters[sid]` is reset to `[]`. `state.warned[sid]`
   stores the deny timestamp (race-window seed).
4. The audit log gains a `spike_detected` line with `count: 3` and
   the three unique patterns.

## Tail the audit log while running

```
tail -F .devcontainer/notify/floating-perms-audit.jsonl | jq .
```

## Negative repro — should NOT count

These four commands are either already in the allow list, are
inside the cwd (`/workspace`), or are shell-form constructs that
never trigger a real prompt. Running each should produce **zero**
`permission_seen` and **zero** `spike_detected` lines:

```
echo > ~/.claude/projects/-workspace/memory/test.md   # Write(/home/node/**) is granted via floating sentinel
grep foo /dev/null                                    # Bash(grep:*) in allow
if [ -f /tmp/x ]; then echo ok; fi                    # shell-form, no real prompt
TOK=$(echo hello)                                     # shell-form, no real prompt
```

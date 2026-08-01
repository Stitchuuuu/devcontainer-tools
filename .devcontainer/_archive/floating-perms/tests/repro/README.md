# Manual repro — floating-perms observational spike detector

Manual gates the user runs after the automated suite is green
(`node --test .devcontainer/skills/floating-perms/tests/*.test.js`).

Both repros tail the same audit log:

```
.devcontainer/notify/floating-perms-audit.jsonl
```

`permission_seen` lines are pushed only by the `PermissionRequest` hook
— one per real prompt. `spike_detected` lines are pushed by `PreToolUse`
when the 120 s window holds ≥ 3 of those.

## Positive repro — 3 prompts → 4th call denied

Commands that fall outside the Niveau 1 firewall + allow-list, so
Claude Code actually prompts:

```
nmap --version
strace --version
wireshark --version
```

Expected sequence in the audit log:

```
{"event":"permission_seen","pattern":"Bash(nmap:*)",...}
{"event":"permission_seen","pattern":"Bash(strace:*)",...}
{"event":"permission_seen","pattern":"Bash(wireshark:*)",...}
```

The next tool call that reaches `PreToolUse` (any tool, any pattern)
is denied with the STOP message listing the three patterns above.

## Negative repro — known false-positive families MUST NOT count

Commands that previously inflated the predictor's counter but
don't actually trigger a real prompt (already allowed, shell-form,
inside cwd):

```
echo > ~/.claude/projects/-workspace/memory/test.md   # floating sentinel allows this
grep foo /dev/null                                    # Bash(grep:*) in allow
if [ -f /tmp/x ]; then echo ok; fi                    # shell-form, never prompts
TOK=$(echo hello)                                     # shell-form, never prompts
```

Expected: **zero** new `permission_seen` lines, **zero**
`spike_detected` lines from this batch.

## Counter inspection

To see the current sliding window for the active sid:

```
jq '.counters' .devcontainer/notify/floating-perms-state.json
```

`state.counters[sid]` should grow only from PermissionRequest events
and prune on every PreToolUse (entries older than 120 s drop).

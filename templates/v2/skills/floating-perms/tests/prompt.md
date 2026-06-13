# Full-flow repro — end-to-end test of the floating-perms skill

The prompt in **§ The prompt** below is a naturalistic task — no
"this is a test" language, no scripted steps for Claude. It just
happens to require three tools that aren't in the allow-list,
followed by a pause-and-redo, which together exercise the whole
skill (spike → deny → AskUserQuestion → apply.js with TTL → retry
→ TTL expiry → fresh re-prompt → revoke audit).

You (the operator at the keyboard) do not paste anything except
the prompt itself ; everything else is your timing + the four
permission dialogs you approve. The "how to run" section gives
you the playbook.

---

## How to run

### Prerequisites

- This branch is merged and `sync-skills.sh` has rebuilt
  `~/.claude/settings.json` (so `PermissionRequest` is wired in).
- The current `/workspace/.claude/settings.local.json` does NOT
  already list any of `Bash(nmap:*)`, `Bash(strace:*)`,
  `Bash(make:*)`. If it does (a leftover floating grant from a
  previous run, or a pre-V1.2 orphan), revoke them first :

  ```
  node /workspace/.devcontainer/skills/floating-perms/apply.js \
    revoke Bash(nmap:*)
  node /workspace/.devcontainer/skills/floating-perms/apply.js \
    revoke Bash(strace:*)
  node /workspace/.devcontainer/skills/floating-perms/apply.js \
    revoke Bash(make:*)
  ```

- `state.grants` should be empty (`jq '.grants'
  /workspace/.devcontainer/notify/floating-perms-state.json`).
  If not, `apply.js gc sid=$CURRENT_SID` to clear orphans.

### Run

1. **Open a second terminal** and tail the audit log :

   ```
   tail -F /workspace/.devcontainer/notify/floating-perms-audit.jsonl | jq .
   ```

2. **Open a fresh Claude Code session** in the devcontainer.

3. **Paste the prompt from § The prompt below**. Send.

4. **Three PermissionRequest dialogs appear**, in order :
   `Bash(nmap:*)`, `Bash(strace:*)`, `Bash(make:*)`.
   Approve each one. The audit log gains three `permission_seen`
   lines.

5. **Claude tries a fourth tool call**, gets denied with the
   floating-perms STOP message, and (per the skill workflow)
   calls `AskUserQuestion` listing the three patterns.
   You will see an option roughly named "Allow ... TTL 60s" or
   similar (Claude phrases it ; the skill examples teach it the
   pattern). **Pick the 60-second TTL option.** Not "until
   SessionEnd" — the whole point is to see the expiry land.

6. **Claude runs `apply.js batch ttl=60s sid=...`**, the audit
   log gains a `grant` line, settings.local.json now contains
   the three patterns wrapped in the floating sentinels.
   Claude then **retries** the previously denied call, which
   succeeds without prompting. The chat continues organically
   with the version-check results.

7. **Do not touch the chat for the next 70 seconds.**
   The 60 s TTL window expires somewhere around the 60 s mark,
   but nothing actually happens yet — `revokeExpired()` only
   fires on the next PreToolUse. At t ≥ 70 s :

8. **Send Claude this follow-up message** :

   ```
   Re-vérifie juste la version de nmap une dernière fois, je
   veux confirmer.
   ```

   (Or any phrasing that tells Claude to re-run `nmap --version`.
   Keep it short and natural.)

9. **A PermissionRequest dialog appears again** for
   `Bash(nmap:*)`. Approve it. This proves the revoke landed :
   the pattern is no longer in allow.

10. **Verify the audit-log sequence** in your second terminal.
    You should see, in order :

    ```
    permission_seen   pattern=Bash(nmap:*)     tool_use_id=...
    permission_seen   pattern=Bash(strace:*)   tool_use_id=...
    permission_seen   pattern=Bash(make:*)     tool_use_id=...
    spike_detected    count=3   patterns=[...3]
    grant             ttl_seconds=60   patterns=[...3]
    revoke            reason=ttl_expired   count=3
    permission_seen   pattern=Bash(nmap:*)     tool_use_id=...
    ```

11. **Verify the live state** :

    ```
    jq '.grants' /workspace/.devcontainer/notify/floating-perms-state.json
    jq '.permissions.allow' /workspace/.claude/settings.local.json | grep -E "nmap|strace|make"
    ```

    Expected : grants is `[]` (the second `permission_seen` is
    one entry in `counters[sid]`, not a grant) ; allow contains
    none of the three Bash patterns.

### Failure mode cheat sheet

| Symptom | Likely cause |
|---|---|
| Step 4 only fires 1 or 2 `permission_seen` | The hook is misregistered, or a stale grant was leftover from a previous run. Check prereqs. |
| Step 5 — no deny, Claude just runs the 4th call | `PreToolUse` hook isn't wired in `~/.claude/settings.json`. Re-run `sync-skills.sh`. |
| Step 5 — Claude bypasses `AskUserQuestion` and calls `apply.js` directly | Skill instructions ignored. The deny reason itself says "Never call apply.js silently". File a regression. |
| Step 6 — `apply.js` refuses one of the three patterns as blocked | Blocklist hit (shouldn't happen on these three). Inspect `lib/blocklist.js`. |
| Step 8 — no permission dialog re-fires | TTL expiry didn't land. Either `revokeExpired()` didn't run (PreToolUse hook broken) or the settings rewrite was a no-op (lock contention or sentinel detection bug). |
| Steps 10/11 — `state.grants` still contains entries | The revoke audit fired but the in-memory state didn't get written back. `cleanup.js` regression. |

---

## The prompt

>>> START — paste this verbatim into a fresh Claude session <<<

```
Petit audit rapide du devcontainer : j'ai besoin de savoir quelles
versions sont installées pour ces trois outils, dans l'ordre :

  - nmap
  - strace
  - make

Pour chaque, lance la commande appropriée (--version, -V, peu
importe) et donne-moi le retour brut. Pas de résumé tant que tu
n'as pas les trois.

Une fois les trois faits, fais-moi un récap d'une ligne par outil
("nmap X.YZ installé" / "strace non installé" / etc.).

Je vais ensuite te demander de revérifier nmap dans une minute ou
deux — un truc à clarifier de mon côté, je te dirai.
```

>>> END <<<

---

## Negative repro (separate session, optional)

These four commands are either already in the allow list, are
inside the cwd (`/workspace`), or are shell-form constructs that
never trigger a real prompt. Run each in a fresh session and
confirm the audit log gains **zero** new `permission_seen` and
**zero** `spike_detected` lines :

```
echo > ~/.claude/projects/-workspace/memory/test.md   # Write(/home/node/**) via floating sentinel
grep foo /dev/null                                    # Bash(grep:*) in allow
if [ -f /tmp/x ]; then echo ok; fi                    # shell-form, never prompts
TOK=$(echo hello)                                     # shell-form, never prompts
```

This is the "no false positives" gate — the reason for the
observational redesign.

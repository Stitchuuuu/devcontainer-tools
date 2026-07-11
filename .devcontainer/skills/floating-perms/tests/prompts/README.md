# floating-perms — validation prompt suite

End-to-end reproducers exercising the full skill flow: hook counting,
spike detection, single-use token, Skill wrapper, apply.js grants,
additionalDirectories injection, blocklist, cleanup.

Each prompt is meant to be pasted in a **fresh Claude Code chat** —
state (grants, counters, pending tokens, audit) leaks between
scenarios otherwise.

## Coverage matrix

| # | Prompt                                             | Categories covered            |
|---|----------------------------------------------------|-------------------------------|
| 1 | [Bash mix](prompt-1-bash-mix.md)                   | A1, A2, A3, E1, E2            |
| 2 | [File tool outside cwd](prompt-2-file-tool-hors-cwd.md) | B1, B3, D1, D2, D3, G1   |
| 3 | [Mix Bash + file tool](prompt-3-mix-bash-file.md)  | C1                            |
| 4 | [Blocklist refuse](prompt-4-blocklist.md)          | F1, F2, F3                    |
| 5 | [additionalDirectories subpath](prompt-5-subpath.md)| B2                           |
| 6 | [Single-use token + Skill wrapper](prompt-6-single-use.md) | H1, H2, H3, H4         |

### Category legend

- **A** — hook counter accuracy (A1 auto-allow guard, A2 canonicalize
  correctness, A3 spike detection threshold).
- **B** — file-tool paths (B1 outside cwd injects additionalDirectories,
  B2 subpath already covered → silent, B3 no redundant allow entry).
- **C** — mixed Bash + file-tool patterns in one batch.
- **D** — AskUserQuestion contract (D1 disclose additionalDirectories,
  D2 single-quote patterns, D3 apply.js output surfaces dir).
- **E** — allow-list heuristics (E1 grep/allow-listed cmd, E2
  /workspace/** auto).
- **F** — blocklist (F1 rm, F2 sudo, F3 system paths refused).
- **G** — cleanup (G1 revoke removes dir from additionalDirectories).
- **H** — single-use token + Skill wrapper (H1 deny reason format,
  H2 Skill routing, H3 single-use enforcement, H4 session-bind).

## How to run

1. **Fresh chat per prompt** — do not chain. Grant/audit state leaks
   between scenarios otherwise.
2. Paste the prompt verbatim in a new Claude Code CLI session.
3. After Claude finishes the task, run the verification snippet in
   each prompt.
4. Cleanup between scenarios:
   ```bash
   node .devcontainer/skills/floating-perms/apply.js list
   # revoke each surviving grant individually, then:
   rm -rf /var/tmp/fp-*
   ```

## Global cleanup — after all runs

```bash
node .devcontainer/skills/floating-perms/apply.js list
# revoke each surviving grant, then:
rm -rf /var/tmp/fp-*
git diff /workspace/.claude/settings.local.json
```

Expected: `list` prints "No active grants", and the mirror has no
floating-perms residual entries under the sentinels.

---
id: 3
name: Blame & history
source: upstream /review (claimed verbatim — sync with anthropics/claude-code)
dispatched: inline at master-review.skill.md Step 3 (Generic agents block)
slot: 3-blame-history
verbatim_check: byte-match between captured live dispatch and master-review.skill.md Step 3 inline string — verified 2026-04-29 (✓ 110/110 bytes)
template_source: parametrized from a live capture (a live /master-review run on a Portal42 hard PR (2026-04-29)); PR-specific bits replaced with ${VAR} placeholders aligned with master-review.skill.md
---

> **Frozen reference — editing this file has no runtime effect.**
> The walker globs `agents/agent-*.md` non-recursively, so this `upstream/` subdir is excluded. The harness composes the prompt at `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)". This file mirrors the structure of what is actually dispatched during a live run — copy/edit when promoting one of the generic agents into a project-specific custom agent.

## Verbatim upstream `/review` core

Read the git blame and history of the code modified, to identify any bugs in light of that historical context.

## Dispatch template (parametrized from live capture)

The harness wraps the verbatim core above with run-specific context. Below is the prompt structure that `/master-review` dispatches to the Sonnet sub-agent for slot `3-blame-history`. Placeholders (`${PR}`, `${PR_BASE_REF}`, `${PR_HEAD_REF}`, `${PR_HEAD_OID}`, `${PR_CHANGED}`, `${PR_ADDITIONS}`, `${PR_DELETIONS}`, `${RUN_DIR}`, `${RUN_SUMMARY}`, `${BASE_REF}`, `${BRANCH}`) are substituted by `master-review.skill.md` at compose time. Layer 3 ("Your task") is byte-identical to the verbatim core above. Layer 0 ("PR REF CONTEXT") was added in S7I — see [`agent-01-claude-md.md`](agent-01-claude-md.md) for the canonical preamble.

````
PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
- PR number: ${PR}
- PR base ref:  origin/${PR_BASE_REF}
- PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
- For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
- For history: `git log origin/${PR_BASE_REF}..origin/${PR_HEAD_REF} -- <path>` and `git blame origin/${PR_HEAD_REF} -- <path>`
- NEVER bare `HEAD` — see canonical preamble in `agent-01-claude-md.md`.
- Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
---

You are master-review Agent #3, the git blame & history reviewer. ${RUN_SUMMARY}

**Your task** (verbatim from upstream `/review`): Read the git blame and history of the code modified, to identify any bugs in light of that historical context.

**Context**:
- PR: ${PR} (`gh pr diff ${PR} --name-only` for files; `gh pr view ${PR}` for metadata)
- Branch: ${BRANCH} (HEAD of the PR; `${BASE_REF}` is the comparison base)
- Look at `git log --oneline ${BASE_REF}..HEAD` to see the commits being reviewed
- For each modified file, look at `git blame` and `git log <file>` to spot bugs that the history reveals (e.g., reverts, regressions, code that was previously fixed elsewhere now reintroduced)

**Method**:
1. List PR files: `gh pr diff ${PR} --name-only`
2. For each significant file, `git log` to see how it has evolved
3. Flag bugs that history makes obvious: regressions, reintroduced patterns, breaks of past invariants

---
STREAMING (live observability):

Append progress lines to `${RUN_DIR}/agents/3-blame-history.live`:
```
echo "[$(date +%H:%M:%S)] action=scanning details=<short>" >> ${RUN_DIR}/agents/3-blame-history.live
```
Format: `[HH:MM:SS] action=<scanning|finding|done> details=<short>`

When you finish, write final findings to `${RUN_DIR}/agents/3-blame-history.final.md` using Write tool.

---

Return findings only. End with `[HH:MM:SS] action=done details=<N findings>` in .live.
````

### Placeholder reference

| Placeholder | Resolved by | Example value |
|---|---|---|
| `${PR}` | `$ARG_PR` (Step 0a) — the integer PR number | `42` |
| `${RUN_DIR}` | Step 3.0 — `.devcontainer/local/master-review/runs/<TS>-PR<PR>` | `.devcontainer/local/master-review/runs/<TS>-PR<int>` |
| `${BASE_REF}` | Step 0a — resolved base ref (PR `baseRefName` → `origin/main` → `origin/master`) | `origin/main` |
| `${BRANCH}` | PR `headRefName` (or `HEAD` for unattached runs) | `feat/error-logging` |
| `${RUN_SUMMARY}` | One-line description (what's being reviewed, baseline state) | `<one-line describing the run intent — what is being reviewed, baseline state if applicable>` |

## Wrapper structure

See [`agent-01-claude-md.md`](agent-01-claude-md.md) for the 7-layer wrapper structure shared by all generic agents. Layer 3 (verbatim core) is the only piece sourced from upstream `/review`; everything else is master-review's framing.

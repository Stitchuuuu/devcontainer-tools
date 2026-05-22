---
id: 5
name: Code comments compliance
source: upstream /review (claimed verbatim — sync with anthropics/claude-code)
dispatched: inline at master-review.skill.md Step 3 (Generic agents block)
slot: 5-code-comments
verbatim_check: byte-match between captured live dispatch and master-review.skill.md Step 3 inline string — verified 2026-04-29 (✓ 129/129 bytes)
template_source: parametrized from a live capture (a live /master-review run on a Portal42 hard PR (2026-04-29)); PR-specific bits replaced with ${VAR} placeholders aligned with master-review.skill.md
---

> **Frozen reference — editing this file has no runtime effect.**
> The walker globs `agents/agent-*.md` non-recursively, so this `upstream/` subdir is excluded. The harness composes the prompt at `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)". This file mirrors the structure of what is actually dispatched during a live run — copy/edit when promoting one of the generic agents into a project-specific custom agent.

## Verbatim upstream `/review` core

Read code comments in the modified files, and make sure the changes in the pull request comply with any guidance in the comments.

## Dispatch template (parametrized from live capture)

The harness wraps the verbatim core above with run-specific context. Below is the prompt structure that `/master-review` dispatches to the Sonnet sub-agent for slot `5-code-comments`. Placeholders (`${PR}`, `${PR_BASE_REF}`, `${PR_HEAD_REF}`, `${PR_HEAD_OID}`, `${PR_CHANGED}`, `${PR_ADDITIONS}`, `${PR_DELETIONS}`, `${RUN_DIR}`, `${RUN_SUMMARY}`, `${FILE_TYPE_HINTS}`) are substituted by `master-review.skill.md` at compose time. Layer 3 ("Your task") is byte-identical to the verbatim core above. Layer 0 ("PR REF CONTEXT") was added in S7I — see [`agent-01-claude-md.md`](agent-01-claude-md.md) for the canonical preamble.

````
PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
- PR number: ${PR}
- PR base ref:  origin/${PR_BASE_REF}
- PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
- For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
- NEVER bare `HEAD` — see canonical preamble in `agent-01-claude-md.md`.
- Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
---

You are master-review Agent #5, the code comments reviewer. ${RUN_SUMMARY}

**Your task** (verbatim from upstream `/review`): Read code comments in the modified files, and make sure the changes in the pull request comply with any guidance in the comments.

**Context**:
- PR: ${PR} (`gh pr diff ${PR}`)
- ${FILE_TYPE_HINTS}
- Look for inline comments, docblocks (`/** ... */`), and TODO/FIXME/XXX/HACK markers
- Flag changes that contradict the comments' intent

**Method**:
1. `gh pr diff ${PR} --name-only` to list files
2. For each, read the file via `gh pr diff ${PR} -- <file>` or via Read tool on local copy
3. Pay attention to docblocks describing API contracts, invariants, "DON'T do X here" warnings
4. Flag where changes break the contract

---
STREAMING (live observability):

Append progress lines to `${RUN_DIR}/agents/5-code-comments.live`:
```
echo "[$(date +%H:%M:%S)] action=scanning details=<short>" >> ${RUN_DIR}/agents/5-code-comments.live
```
Format: `[HH:MM:SS] action=<scanning|finding|done> details=<short>`

When you finish, write final findings to `${RUN_DIR}/agents/5-code-comments.final.md` using Write tool.

---

Return findings only. End with `[HH:MM:SS] action=done details=<N findings>` in .live.
````

### Placeholder reference

| Placeholder | Resolved by | Example value |
|---|---|---|
| `${PR}` | `$ARG_PR` (Step 0a) — the integer PR number | `42` |
| `${RUN_DIR}` | Step 3.0 — `.devcontainer/local/master-review/runs/<TS>-PR<PR>` | `.devcontainer/local/master-review/runs/<TS>-PR<int>` |
| `${RUN_SUMMARY}` | One-line description (what's being reviewed, baseline state) | `<one-line describing the run intent — what is being reviewed, baseline state if applicable>` |
| `${FILE_TYPE_HINTS}` | Optional one-liner enumerating the file types in the diff (lets the agent skip the `gh pr diff --name-only` extension probe) | `Modified files include PHP (api/, inc/), JavaScript (scripts/pages/, modals/), shell scripts (wtf), and docs` |

## Wrapper structure

See [`agent-01-claude-md.md`](agent-01-claude-md.md) for the 7-layer wrapper structure shared by all generic agents. Layer 3 (verbatim core) is the only piece sourced from upstream `/review`; everything else is master-review's framing.

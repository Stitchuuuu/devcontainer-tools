---
id: 2
name: Shallow bug scan
source: upstream /review (claimed verbatim — sync with anthropics/claude-code)
dispatched: inline at master-review.skill.md Step 3 (Generic agents block)
slot: 2-shallow-bug
verbatim_check: byte-match between captured live dispatch and master-review.skill.md Step 3 inline string — verified 2026-04-29 (✓ 261/261 bytes)
template_source: parametrized from a live capture (a live /master-review run on a Portal42 hard PR (2026-04-29)); PR-specific bits replaced with ${VAR} placeholders aligned with master-review.skill.md
---

> **Frozen reference — editing this file has no runtime effect.**
> The walker globs `agents/agent-*.md` non-recursively, so this `upstream/` subdir is excluded. The harness composes the prompt at `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)". This file mirrors the structure of what is actually dispatched during a live run — copy/edit when promoting one of the generic agents into a project-specific custom agent.

## Verbatim upstream `/review` core

Read the file changes in the pull request, then do a shallow scan for obvious bugs. Avoid reading extra context beyond the changes, focusing just on the changes themselves. Focus on large bugs, and avoid small issues and nitpicks. Ignore likely false positives.

## Dispatch template (parametrized from live capture)

The harness wraps the verbatim core above with run-specific context. Below is the prompt structure that `/master-review` dispatches to the Sonnet sub-agent for slot `2-shallow-bug`. Placeholders (`${PR}`, `${PR_BASE_REF}`, `${PR_HEAD_REF}`, `${PR_HEAD_OID}`, `${PR_CHANGED}`, `${PR_ADDITIONS}`, `${PR_DELETIONS}`, `${RUN_DIR}`, `${RUN_SUMMARY}`, `${KNOWN_BUG_CLASSES}`) are substituted by `master-review.skill.md` at compose time. Layer 3 ("Your task") is byte-identical to the verbatim core above. Layer 0 ("PR REF CONTEXT") was added in S7I — see [`agent-01-claude-md.md`](agent-01-claude-md.md) for the canonical preamble.

````
PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
- PR number: ${PR}
- PR base ref:  origin/${PR_BASE_REF}
- PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
- For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
- NEVER bare `HEAD` — see canonical preamble in `agent-01-claude-md.md`.
- Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
---

You are master-review Agent #2, the shallow bug scanner. ${RUN_SUMMARY}

**Your task** (verbatim from upstream `/review`): Read the file changes in the pull request, then do a shallow scan for obvious bugs. Avoid reading extra context beyond the changes, focusing just on the changes themselves. Focus on large bugs, and avoid small issues and nitpicks. Ignore likely false positives.

**Context**:
- PR: ${PR} (`gh pr diff ${PR}`)
- ${KNOWN_BUG_CLASSES}

**Method**:
1. Run `gh pr diff ${PR}` and read the diff carefully.
2. Identify obvious bugs: type errors, undefined vars, off-by-one, race conditions, missing escape, unsanitized inputs, etc.
3. Don't go deep — shallow scan only.

---
STREAMING (live observability):

As you work, append progress lines to `${RUN_DIR}/agents/2-shallow-bug.live`:
```
echo "[$(date +%H:%M:%S)] action=scanning details=<short>" >> ${RUN_DIR}/agents/2-shallow-bug.live
```
Format: `[HH:MM:SS] action=<scanning|finding|done> details=<short>`

When you finish, write your final findings to `${RUN_DIR}/agents/2-shallow-bug.final.md` using the Write tool. Format: bulleted findings with file:line, severity, brief description.

---

Return findings only. End with `[HH:MM:SS] action=done details=<N findings>` in .live.
````

### Placeholder reference

| Placeholder | Resolved by | Example value |
|---|---|---|
| `${PR}` | `$ARG_PR` (Step 0a) — the integer PR number | `42` |
| `${RUN_DIR}` | Step 3.0 — `.devcontainer/local/master-review/runs/<TS>-PR<PR>` | `.devcontainer/local/master-review/runs/<TS>-PR<int>` |
| `${RUN_SUMMARY}` | One-line description (what's being reviewed, baseline state) | `<one-line describing the run intent — what is being reviewed, baseline state if applicable>` |
| `${KNOWN_BUG_CLASSES}` | Optional one-liner enumerating bug classes the run is calibrated to catch | `Known bug classes that escaped early review include: XSS in HTML attributes, ANSI/control-char injection, uniqid races, atomic file ops half-done states, ZIP stream temp leaks on disconnect, error-in-error-handler, ...` |

## Wrapper structure

See [`agent-01-claude-md.md`](agent-01-claude-md.md) for the 7-layer wrapper structure shared by all generic agents. Layer 3 (verbatim core) is the only piece sourced from upstream `/review`; everything else is master-review's framing.

S7F is planned to enrich Agent #2 with a Portal42-specific perf hot-path sweep (composed inline in `master-review.skill.md` Step 3 rather than promoted to a custom agent file — it stays "generic with a Portal42 amendment"). When S7F lands, the **Method** layer in this template will gain a 4th step. The verbatim core (layer 3) remains untouched.

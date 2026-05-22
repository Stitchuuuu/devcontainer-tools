---
id: 1
name: CLAUDE.md compliance
source: upstream /review (claimed verbatim — sync with anthropics/claude-code)
dispatched: inline at master-review.skill.md Step 3 (Generic agents block)
slot: 1-claude-md
verbatim_check: byte-match between captured live dispatch and master-review.skill.md Step 3 inline string — verified 2026-04-29 (✓ 187/187 bytes)
template_source: parametrized from a live capture (a live /master-review run on a Portal42 hard PR (2026-04-29)); PR-specific bits replaced with ${VAR} placeholders aligned with master-review.skill.md
---

> **Frozen reference — editing this file has no runtime effect.**
> The walker globs `agents/agent-*.md` non-recursively, so this `upstream/` subdir is excluded. The harness composes the prompt at `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)". This file mirrors the structure of what is actually dispatched during a live run — copy/edit when promoting one of the generic agents into a project-specific custom agent.

## Verbatim upstream `/review` core

Audit the changes to make sure they comply with the CLAUDE.md. Note that CLAUDE.md is guidance for Claude as it writes code, so not all instructions will be applicable during code review.

## Dispatch template (parametrized from live capture)

The harness wraps the verbatim core above with run-specific context. Below is the prompt structure that `/master-review` dispatches to the Sonnet sub-agent for slot `1-claude-md`. Placeholders (`${PR}`, `${PR_BASE_REF}`, `${PR_HEAD_REF}`, `${PR_HEAD_OID}`, `${PR_CHANGED}`, `${PR_ADDITIONS}`, `${PR_DELETIONS}`, `${RUN_DIR}`, `${CONVENTIONS_DOC}`, `${DEV_DOC}`, `${RUN_SUMMARY}`, `${DIFF_SUMMARY}`) are substituted by `master-review.skill.md` at compose time. Layer 3 ("Your task") is byte-identical to the verbatim core above. Layer 0 ("PR REF CONTEXT") was added in S7I to fix the wrong-diff regression observed on PR-1298.

````
PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
- PR number: ${PR}
- PR base ref:  origin/${PR_BASE_REF}
- PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
- For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
- For reading files at PR head: `git show origin/${PR_HEAD_REF}:<path>`
- For history: `git log origin/${PR_BASE_REF}..origin/${PR_HEAD_REF} -- <path>` and `git blame origin/${PR_HEAD_REF} -- <path>`
- NEVER `git diff <X>...HEAD` — `HEAD` resolves to the orchestrator's
  current branch, not the PR head, which produces a wrong-diff
  regression (5/7 agents hit this on PR-1298 S7F.0 pre-validation).
- Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
  If your `git diff` reports a different shape, you are pointed at
  wrong refs — abort and request the orchestrator to fix the prompt.
---

You are master-review Agent #1, the CLAUDE.md compliance auditor. ${RUN_SUMMARY}

**Your task** (verbatim from upstream `/review`): Audit the changes to make sure they comply with the CLAUDE.md. Note that CLAUDE.md is guidance for Claude as it writes code, so not all instructions will be applicable during code review.

**Context**:
- PR: ${PR} (`gh pr diff ${PR}`)
- Conventions doc: ${CONVENTIONS_DOC} (read this for project rules)
- Repo dev doc: ${DEV_DOC} (per-directory CLAUDE.md if any)
- ${DIFF_SUMMARY}

**Method**:
1. Read ${CONVENTIONS_DOC} to understand the project's rules (language conventions, framework rules, forbidden patterns, required helpers, etc.).
2. Run `gh pr diff ${PR}` to get the full diff.
3. Find changes that violate documented CLAUDE.md rules. Cite file:line and quote the rule.

---
STREAMING (live observability):

As you work, append a progress line to the file at `${RUN_DIR}/agents/1-claude-md.live` each time you start scanning a new file or report a finding. Use Bash:
```
echo "[$(date +%H:%M:%S)] action=scanning details=<short>" >> ${RUN_DIR}/agents/1-claude-md.live
```
Format: `[HH:MM:SS] action=<scanning|finding|done> details=<short>`

When you finish, also write your full final findings list to `${RUN_DIR}/agents/1-claude-md.final.md` using the Write tool. Format: bulleted findings with file:line, severity, CLAUDE.md rule cited, brief explanation.

---

Return findings only — no GH comment, no recap. Read-only review. End with `[HH:MM:SS] action=done details=<N findings>` in the .live file.
````

### Placeholder reference

| Placeholder | Resolved by | Example value |
|---|---|---|
| `${PR}` | `$ARG_PR` (Step 0b) — the integer PR number | `42` |
| `${PR_BASE_REF}` | Step 0b — `gh pr view --json baseRefName` | `main` |
| `${PR_HEAD_REF}` | Step 0b — `gh pr view --json headRefName` | `feat/error-logger` |
| `${PR_HEAD_OID}` | Step 0b — `gh pr view --json headRefOid` (added S7I) | `cc1b46e8c0…` |
| `${PR_CHANGED}` | Step 0b — `gh pr view --json changedFiles` | `12` |
| `${PR_ADDITIONS}` | Step 0b — `gh pr view --json additions` | `847` |
| `${PR_DELETIONS}` | Step 0b — `gh pr view --json deletions` | `64` |
| `${RUN_DIR}` | Step 3.0 — `.devcontainer/local/master-review/runs/<TS>-PR<PR>` | `.devcontainer/local/master-review/runs/<TS>-PR<int>` |
| `${CONVENTIONS_DOC}` | `$CONVENTIONS_DOC` from `review-config.md` Project Meta | `/workspace/.devcontainer/claude/CLAUDE-reviewer.md` |
| `${DEV_DOC}` | `$DEV_DOC` from `review-config.md` Project Meta | `/workspace/.devcontainer/claude/CLAUDE-dev.md` |
| `${RUN_SUMMARY}` | One-line description, optionally injected by Step 3 (kept short — what's being reviewed, baseline state) | `<one-line describing the run intent — what is being reviewed, baseline state if applicable>` |
| `${DIFF_SUMMARY}` | One-line description of the diff scope and any known bug classes (often pulled from PR title/body or run-specific instructions) | `Diff is the original feature commits without review fixes — bugs that violate CLAUDE.md should still be present.` |

## Wrapper structure

Each generic-agent dispatch follows this structure. Layer 0 is a preamble prepended by the Step 3 composer (added in S7I); Layers 1-7 are the original wrapper. The verbatim core is Layer 3.

0. **PR REF CONTEXT (preamble — added S7I)** — explicit `origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}` pair plus `${PR_HEAD_OID}` and the expected diff size, so the agent never falls back to bare `HEAD` (which resolves to the orchestrator's checked-out branch and produces a wrong-diff regression). Prepended at the very top of the prompt by `master-review.skill.md` Step 3 → "Composing the streaming block" point 3, for **every** agent (1-8 and any custom ID > 8).
1. **Identity** — `You are master-review Agent #N, the X reviewer.`
2. **Run summary** — `${RUN_SUMMARY}` (optional one-liner)
3. **Your task (verbatim)** — the upstream `/review` core string, byte-identical
4. **Context** — `${PR}`, `${CONVENTIONS_DOC}`, `${DEV_DOC}`, `${DIFF_SUMMARY}` (project-specific from `review-config.md`)
5. **Method** — concrete scan recipes (project-specific tooling — `gh pr diff`, project's grep aliases, etc.)
6. **Streaming** — paths to `${RUN_DIR}/agents/<slot>.live` and `<slot>.final.md`
7. **Closing constraints** — `Return findings only`, `End with action=done details=<N findings>`

To promote this generic agent into a custom agent, copy this file to `agents/agent-NN-<slug>.md` (NN ≥ 6), drop the `source` / `dispatched` / `slot` / `verbatim_check` / `template_source` keys (they are upstream-only metadata), add `trigger:` and `tools:`, and rewrite the body — typically by enriching the **Method** block with more grep recipes, sub-classes to scan, or framings (F1–F5) to apply.

---
id: 4
name: Prior PR comments
source: upstream /review (claimed verbatim — sync with anthropics/claude-code)
dispatched: inline at master-review.skill.md Step 3 (Generic agents block)
slot: 4-pr-comments
verbatim_check: byte-match against captured live dispatch NOT performed — Portal42 drops Agent #4 from `LAUNCH_GENERICS` (see frontmatter `note` below). The verbatim string below is a transcription of `master-review.skill.md` Step 3 inline content. The dispatch template below is **synthetic** — extrapolated from the wrapper structure of #1/#2/#3/#5 since no live capture exists for #4 on this project.
note: Dropped from LAUNCH_GENERICS when `review-config.md` sets `## GitHub Review Threads — enabled: false` — Portal42 default. Reason : review feedback lives in local `PR-N-review.md` files, not GitHub native review threads. Agent #4's empirical yield was 0/0/0 on Portal42 with 31 Bash calls consumed (S4 golden test) — that's why it's opt-out.
---

> **Frozen reference — editing this file has no runtime effect.**
> The walker globs `agents/agent-*.md` non-recursively, so this `upstream/` subdir is excluded. The harness composes the prompt at `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)" — but Agent #4 is currently OPT-OUT for Portal42 (see the `note` frontmatter key above).

## Verbatim upstream `/review` core

Read previous pull requests that touched these files, and check for any comments on those pull requests that may also apply to the current pull request.

## Dispatch template (synthetic — extrapolated from #1/#2/#3/#5 structure)

⚠️ **No live capture available** for Portal42 (Agent #4 is dropped from `LAUNCH_GENERICS`). The template below is a synthetic reconstruction matching the wrapper structure used by the dispatched generic agents. Placeholders (`${PR}`, `${PR_BASE_REF}`, `${PR_HEAD_REF}`, `${PR_HEAD_OID}`, `${PR_CHANGED}`, `${PR_ADDITIONS}`, `${PR_DELETIONS}`, `${RUN_DIR}`, `${RUN_SUMMARY}`) are aligned with `master-review.skill.md` at compose time. Layer 3 ("Your task") would be byte-identical to the verbatim core above. Layer 0 ("PR REF CONTEXT") was added in S7I — see [`agent-01-claude-md.md`](agent-01-claude-md.md) for the canonical preamble.

````
PR REF CONTEXT (use these refs, NEVER bare `HEAD`):
- PR number: ${PR}
- PR base ref:  origin/${PR_BASE_REF}
- PR head ref:  origin/${PR_HEAD_REF}  (OID ${PR_HEAD_OID})
- For the diff: `gh pr diff ${PR}` OR `git diff origin/${PR_BASE_REF}...origin/${PR_HEAD_REF}`
- NEVER bare `HEAD` — see canonical preamble in `agent-01-claude-md.md`.
- Expected diff size: ${PR_CHANGED} files, +${PR_ADDITIONS}/-${PR_DELETIONS}.
---

You are master-review Agent #4, the prior PR comments reviewer. ${RUN_SUMMARY}

**Your task** (verbatim from upstream `/review`): Read previous pull requests that touched these files, and check for any comments on those pull requests that may also apply to the current pull request.

**Context**:
- PR: ${PR} (`gh pr diff ${PR} --name-only` for files; `gh pr view ${PR}` for metadata)
- For each modified file, find prior PRs that touched it and read the discussion threads — past objections, suggestions that were dismissed, deferred follow-ups, ...
- Use `gh search prs --json` and `gh api repos/<owner>/<repo>/pulls/<n>/comments` for thread bodies

**Method**:
1. List PR files: `gh pr diff ${PR} --name-only`
2. For each file, find prior PRs touching it: `gh search prs --repo <owner>/<repo> --json number,title -- '<file>'`
3. Read review comments on those PRs and flag points that apply to the current diff (e.g. a past reviewer asked "what about case X?" and X is still unhandled here)

---
STREAMING (live observability):

Append progress lines to `${RUN_DIR}/agents/4-pr-comments.live`:
```
echo "[$(date +%H:%M:%S)] action=scanning details=<short>" >> ${RUN_DIR}/agents/4-pr-comments.live
```
Format: `[HH:MM:SS] action=<scanning|finding|done> details=<short>`

When you finish, write final findings to `${RUN_DIR}/agents/4-pr-comments.final.md` using Write tool.

---

Return findings only. End with `[HH:MM:SS] action=done details=<N findings>` in .live.
````

### Placeholder reference

| Placeholder | Resolved by | Example value |
|---|---|---|
| `${PR}` | `$ARG_PR` (Step 0a) — the integer PR number | `42` |
| `${RUN_DIR}` | Step 3.0 — `.devcontainer/local/master-review/runs/<TS>-PR<PR>` | `.devcontainer/local/master-review/runs/<TS>-PR<int>` |
| `${RUN_SUMMARY}` | One-line description (what's being reviewed, baseline state) | `<one-line describing the run intent — what is being reviewed, baseline state if applicable>` |

## Wrapper structure

See [`agent-01-claude-md.md`](agent-01-claude-md.md) for the 7-layer wrapper structure shared by all generic agents. Layer 3 (verbatim core) is the only piece sourced from upstream `/review`; everything else is master-review's framing.

If a future project needs Agent #4 dispatched, no code change is required — just flip `## GitHub Review Threads` to `enabled: true` (or omit the section, the default is `true`) in that project's `review-config.md`. The captured dispatch from that project would then replace this synthetic template above.

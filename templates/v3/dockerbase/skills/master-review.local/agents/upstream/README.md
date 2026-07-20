# Upstream agents — frozen reference

Files in this folder are **verbatim copies** of the 5 generic agents shipped by the upstream `/review` command (`anthropics/claude-code`), plus the live composition wrapper that `/master-review` builds around each verbatim core when it dispatches a Sonnet sub-agent.

**These files are NOT dispatched by the Step 0c walker.** The walker globs `agents/agent-*.md` (non-recursive); this `upstream/` subdir is excluded by design. The actual dispatch path for #1–#5 is `master-review.skill.md` Step 3 → "Generic agents (verbatim from upstream `/review`)" — the harness inlines the same prompt strings there. Editing the files in this folder has **no runtime effect**.

## What each file contains

For agents #1, #2, #3, #5 (dispatched by Portal42):
1. **Frontmatter** — id, name, source, dispatched, slot, `verbatim_check` (byte-match status), `template_source` (provenance of the dispatch template).
2. **Frozen-reference banner** — at the top of the body, repeats the no-runtime-effect warning so it stays visible even if someone opens the file directly without reading this README.
3. **`## Verbatim upstream /review core`** — the 1-paragraph core string. This is what's claimed to be byte-identical to upstream Anthropic `/review`.
4. **`## Dispatch template (parametrized from live capture)`** — the FULL prompt that `/master-review` dispatches to a Sonnet sub-agent. PR-specific values are replaced with `${VAR}` placeholders aligned with `master-review.skill.md` (so the file stays project-portable; copy-paste produces something an AI can read without prior context). The verbatim core is layer 3 of a 7-layer wrapper.
5. **Placeholder reference** — table mapping each `${VAR}` to its resolution path inside `master-review.skill.md` and an example value.
6. **`## Wrapper structure`** — the 7-layer breakdown explaining what's verbatim vs. what's master-review's framing.

For agent #4 (NOT dispatched by Portal42):
1. Same frontmatter + banner.
2. Verbatim core.
3. **Synthetic** dispatch template — extrapolated from the wrapper structure of #1/#2/#3/#5, since no live capture exists. Marked clearly as synthetic.
4. Pointer to `agent-01-claude-md.md` for the wrapper structure.

## Verification provenance (2026-04-29)

The byte-match between the captured live dispatch and the inline string in `master-review.skill.md` Step 3 was verified with a Python script that:
1. Greps `master-review.skill.md` for lines `^- \*\*Agent #(\d+)\*\* — (.+)$`.
2. Reads each captured prompt from the JSONL of a live /master-review run on a Portal42 hard PR (2026-04-29).
3. Extracts the line matching `\*\*Your task\*\* \(verbatim from upstream `/review`\): (.+?)`.
4. Asserts byte-equality.

Result: ✅ 4/4 MATCH (#1=187 bytes, #2=261, #3=110, #5=129). The verbatim core in `master-review.skill.md` is byte-identical to what was actually sent to Sonnet during the live run.

What is NOT verified : whether those bytes match the **current upstream `/review`** in `anthropics/claude-code` (the binary is closed-source ELF/Bun bundle and the strings are not extractable). If Anthropic has updated `/review` since the manual copy was made (S1 — 2026-04-28), our verbatim has drifted from upstream silently.

## Why keep them here

- **Discoverability** — `ls agents/` shows the full set of agents in one place instead of half here, half buried in `master-review.skill.md`.
- **Reference for customization** — when promoting one of the generic agents into a project-specific custom agent (e.g. S7F enriching #2 with a perf hot-path sweep), the captured dispatch is a working starting point. Copy the file, drop the frontmatter keys specific to "frozen", add `trigger:`/`tools:`, edit the body.
- **Sync target** — when upstream `/review` evolves, refresh both the verbatim cores AND the inline strings in `master-review.skill.md` Step 3 in the same commit. Diff between the upstream state and this folder = the drift.

## Filename convention

`agent-NN-<slug>.md` where `NN` is zero-padded (`01`..`05`). Slug matches the `slot` field used by the live observability harness (`$RUN_DIR/agents/<slot>.{live,final.md}`).

# 20260615-0907 — `/prepare-plan` 3 modes + Plan mode + clickable links

**Affects** : v2.x devcontainers carrying any `prepare-plan` skill before
this update — i.e. anything based on `devcontainer-tools` `main` at or
before commit `7fcee55` (the step-0 context-routing backport).

**Symptom** : three independent papercuts in `/prepare-plan`.

- **Mode confusion.** The skill's step-0 question offered three modes
  (`This session` / `New single session` / `Multi-session`) but `single`
  and `multi` produced **byte-identical 5-file scaffolds** — the
  framing differed, the artefacts did not. For a true one-shot ("I just
  need to paste a prompt into a fresh chat"), the ROLLOUT/STATUS/LOG
  bookkeeping was pure overhead.
- **Plan-mode violation.** Invoking the skill while Claude was in Plan
  mode wrote files to disk immediately (step 4 = `mkdir -p` + 5 `Write`
  calls), violating the Plan-mode contract ("no edits except the plan
  file"). The skill had no awareness of Plan mode.
- **Non-clickable final message.** The "next-steps" block was emitted
  inside a 4-backtick fence, so paths like `sessions/session-1-foo.md`
  rendered as plain text in the VS Code extension — user had to
  navigate to the file manually. The extension supports
  `[name](path)` links, but the skill never used them.

**Cause** : the skill grew incrementally and never had a unified pass on
its three failure surfaces. Decisions captured in
`devcontainer-tools` commit history.

**Resolution** : single skill rewrite. Three orthogonal changes :

1. **Drop redundant mode.** Step 0 now offers `This session` /
   `Fresh chat, prompt only` / `Multi-session scaffold`. The "single
   scaffolded" case (where you wanted the audit trail for a one-shot)
   is rare enough that users can pick `Multi-session scaffold`
   explicitly when needed.
2. **Plan-mode integration.** New `## Plan mode integration` section
   documents deferred behavior : during Plan mode the skill embeds the
   scaffold spec into the plan file (no disk writes), and the actual
   writes happen after `ExitPlanMode` approval. A guard at step 4
   ("Skip this step entirely if Plan mode is active") makes the
   contract explicit.
3. **Clickable markdown links.** Step 6 now emits raw markdown (no
   fence) with workspace-relative paths (`plans/<name>/...`). The
   `session-1` link is bolded with a `←` marker so the eye lands on
   the only immediate action.

Also added : a new `prompt-only` template (minimal, no
ROLLOUT/STATUS/LOG references), updated Constraints, two new Failure
modes rows.

**Prompt-only output format.** The `Fresh chat, prompt only` mode
always prints the rendered prompt in chat wrapped in a **5-backtick
fence** (with a 6-backtick inner fence for the prompt body) — same
behavior in Plan mode and outside Plan mode, no plan-file embed. 5
backticks gives the VS Code "copy code" button on the rendered fence
(one click, full prompt copied) and survives 3- and 4-backtick code
blocks the prompt may contain. Escalate to 6 backticks for the outer
fence if the prompt itself carries a 5-backtick block.

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the
[Targeted updates bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname)
populates `.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates/20260615-0907-prepare-plan-modes-plan-mode-links/update.patch
git apply        .tmp/devcontainer-updates/updates/20260615-0907-prepare-plan-modes-plan-mode-links/update.patch

git add .devcontainer/skills/prepare-plan/prepare-plan.skill.md
git commit -m "feat(skill): prepare-plan 3 modes + Plan mode integration + clickable links"
`````

No daemon to restart, no rebuild — the skill markdown is re-read by
Claude Code on the next `/prepare-plan` invocation.

## Verify

- [ ] `grep -c "Fresh chat, prompt only" .devcontainer/skills/prepare-plan/prepare-plan.skill.md`
      → ≥ 5 hits (mode label appears in step 0, heuristic table,
      response routing, Plan-mode integration, and prompt-only section).
- [ ] `grep -c "New single session\|mode = single" .devcontainer/skills/prepare-plan/prepare-plan.skill.md`
      → 0 (old single mode fully removed).
- [ ] `grep -E "^## Plan mode integration" .devcontainer/skills/prepare-plan/prepare-plan.skill.md`
      → 1 match (new section present).
- [ ] `grep -F "[ROLLOUT.md](plans/" .devcontainer/skills/prepare-plan/prepare-plan.skill.md`
      → at least one match (clickable-link template present in step 6).
- [ ] Next time you invoke `/prepare-plan` and pick `Multi-session
      scaffold`, the final message shows file paths as clickable
      markdown links (not inside a fence). Click on `sessions/session-1-*.md`
      opens the file directly.
- [ ] Next time you invoke `/prepare-plan` while in Plan mode, no file
      is created under `/workspace/plans/<name>/` — instead, the scaffold
      spec is embedded into the plan file at `~/.claude/plans/<plan>.md`.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
`````

No state to migrate, no daemon to bounce. The previous skill markdown
is restored ; the next `/prepare-plan` invocation runs the old flow.

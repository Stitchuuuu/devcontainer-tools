# master-review

Multi-agent PR code review packaged as a portable skill folder. Drop into any project, customize `review-config.md`, run `/master-review <pr_number>`.

## What this is

A self-contained Claude Code skill that wraps the upstream `/review` 5-agent core (CLAUDE.md compliance / shallow bug scan / git blame / prior PR comments / inline comments) inside a project-aware flow:

- **Step 0** — saturation gate, distance-from-main check, tier scoring (T1-T4+), interactive bootstrap when no config exists, dispatch decision based on tier × custom agents declared in `review-config.md`.
- **Steps 1-7** — eligibility, context gather, parallel multi-agent review, Haiku confidence scoring (rubric 0-100), local round + recap files, `gh pr comment`, fix-commit hygiene linter.
- **Step 8** — TSV metrics row + plateau detection + kickoff prompt for the next session.

The skill is **project-agnostic**. All domain knowledge (custom agents, surfaces, tier weights, framings, output paths) lives in `review-config.md`.

## Install

### Path A — devcontainer-tools (recommended)

If your project uses `devcontainer-tools`:

```bash
bash <path-to>/devcontainer-tools/add-skill.sh master-review
# Then either restart the devcontainer (post-start runs sync-skills.sh)
# or apply now:
bash .devcontainer/skills/sync-skills.sh
```

### Path B — Standalone (no devcontainer-tools)

Copy this folder anywhere, run the bundled installer:

```bash
cp -r master-review /target-project/.devcontainer/skills/master-review
cd /target-project
bash .devcontainer/skills/master-review.local/install-manual.sh
```

`install-manual.sh` requires `jq`. It copies the skill to `~/.claude/commands/master-review.md` and merges `hooks.json` into `~/.claude/settings.json` (idempotent, atomic, with backup).

## Layout

```
master-review/
├── master-review.skill.md            # Skill + command in one file (sync-skills.sh strips .skill → ~/.claude/commands/master-review.md)
├── hooks.json                        # 2 Stop hooks declared declaratively, merged into settings.json
├── master-review-suggest-fresh.sh    # Stop hook: nudge user to open a fresh session at >150 user prompts on a review
├── master-review-log-session.sh      # Stop hook: append TSV row to ~/.claude/review-sessions.log
├── master-review-gen-kickoff.sh      # Helper: generate copy-paste kickoff prompt for the next session
├── templates/
│   ├── pr-recap.template.md          # T2/T3 recap format
│   ├── pr-surfaces.template.md       # T3+ surface matrix
│   └── pr-review-round.template.md   # Per-round review format (overwritten round-to-round)
├── agents/                           # Custom agents (one .md per agent — frontmatter + body)
│   ├── README.md                     # File-format reference for agent files
│   ├── agent-06-security.md          # #6 Security Portal42 (tier ≥ T3)
│   ├── agent-07-lifecycle.md         # #7 Lifecycle & Atomicity (tier ≥ T3)
│   └── agent-08-adversarial.md       # #8 Adversarial composite (tier ≥ T4+)
├── review-config.md                  # ACTIVE config (project-specific — edit this)
├── review-config.example.md          # FROZEN reference (Portal42 defaults — kept fresh by update.sh)
├── install-manual.sh                 # Standalone fallback installer (Path B)
└── README.md                         # This file
```

## Live observability — where logs go and how to watch them

Each `/master-review` run creates a self-contained, gitignored run directory:

```
.devcontainer/local/master-review/runs/
├── current → 2026-04-28T15-18-59-PR1298/    # symlink rotated to the latest run
└── 2026-04-28T15-18-59-PR1298/
    ├── status.md                              # tier, agents launched/returned/findings, scoring
    ├── step0/                                 # snapshot of /tmp/master-review-step0.env + config.env
    ├── agents/
    │   ├── 1-claude-md.live                   # streaming progress lines (one per scan/finding)
    │   ├── 1-claude-md.final.md               # mirror of the agent's final report
    │   ├── 2-shallow-bug.live / .final.md
    │   ├── …
    │   └── 8-adversarial.live / .final.md
    └── final/                                 # snapshot of round/recap files written at Step 5
```

To follow all agents live during a multi-agent run, in a second terminal:

```bash
tail -f .devcontainer/local/master-review/runs/current/agents/*.live
```

Each agent appends one line per file scanned or finding raised, formatted `[HH:MM:SS] action=<scanning|finding|done> details=<short>` (generic agents 1-5) or `[HH:MM:SS] surface=<X> action=<…> details=<…>` (custom agents #6-#8). When an agent returns, the orchestrator marks its row in `status.md` `returned` and fills `Returned`/`Findings`. Step 4 appends a `## Scoring` section that finalizes to `Status: done — N kept / M total`.

The whole `.devcontainer/local/master-review/runs/` tree is gitignored (covered by `.devcontainer/local/`) — safe to keep, safe to wipe.

## Customization

The single config file controls everything project-specific. Section-by-section:

- **Project Meta** — name, stack, conventions doc path. Required.
- **Tier Scoring Overrides** — markdown table of `regex → +N weight` (or `BLOCKING — abort review`). Layered on top of default `+lines/+files` heuristic.
- **Surfaces** — markdown table mapping surface IDs (A/B/C/…) to trigger patterns and the agent that audits each. Drives the surface checklist for T3+ PRs.
- **Custom Agents** — Sonnet sub-agents defined one-per-file in [`agents/`](agents/) (YAML frontmatter `id`/`name`/`trigger`/`tools` + body). Loaded conditionally by tier (`trigger: tier ≥ T3` etc.). The `## Custom Agents` section in `review-config.md` is a pointer paragraph that lists the active agents — the prompts themselves live in `agents/agent-NN-<slug>.md`. See [`agents/README.md`](agents/README.md) for the format.
- **Tactical Framings** — F1-F5 generic prompts (fatal-OOM, generalization, gap analysis, diff-exact, timing/ordering) used inside custom agent prompts.
- **Output Paths** — where to write `PR-N-review.md`, recap, surfaces. Defaults to repo root.
- **Commit Hygiene Regex** — bullet list of regexes the commit-linter rejects.
- **Special-case Files** — paths that block the review (e.g. payment integrations) or warn before touching.
- **Override Threshold** — confidence score floor (0-100, default 80). Per-agent overrides supported.
- **GitHub Review Threads** — `enabled: false` drops Agent #4 (prior PR review comments) — use for projects where review feedback lives in local files instead.

`review-config.example.md` is the Portal42 reference (610 lines, fully populated). `review-config.md` starts as a copy of that and is yours to edit. `update.sh` from devcontainer-tools refreshes the `.example.md` but leaves your `.md` alone (`copy_if_missing` semantic).

## Bootstrap

First `/master-review` run in a project without `review-config.md` triggers an interactive 5-question bootstrap (stack / critical paths / conventions doc / output paths / threshold) and writes a starter config with commented placeholders for Surfaces and Custom Agents. The Surfaces table you'd copy from `review-config.example.md` and adapt; the custom agents you'd add as files under `agents/agent-NN-<slug>.md` (the example folder ships three Portal42 agents as references).

To skip the bootstrap permanently for a project: pick "Skip and remember" at the prompt. This writes `.devcontainer/skills/master-review.local/.skip-bootstrap`. Remove the sentinel to re-enable.

To bypass the bootstrap for one session only: pick "No — vanilla this run". Vanilla mode runs the upstream 5-agent core with default tier weights and threshold 80.

## Hooks

- **`master-review-suggest-fresh.sh`** — fires on session Stop. If >150 user prompts AND a `PR-*-review.md`/`PR-*-recap.md` is currently modified, emits a `systemMessage` with a copy-pasteable kickoff prompt for the next session. Silent otherwise.
- **`master-review-log-session.sh`** — fires on session Stop. If a review-related file is modified, appends one TSV row to `~/.claude/review-sessions.log` (timestamp, PR#, tier, round, session_id, jsonl_lines, user_prompts, new_findings, fixed_findings, surfaces, duration_min). Silent outside review context.

Both register via `hooks.json`. `sync-skills.sh` (devcontainer-tools) or `install-manual.sh` (standalone) merges them into `~/.claude/settings.json`.

## Templates

- **`pr-recap.template.md`** — overall PR recap (decisions table, surfaces couvertes, fix-commit traceability). Used at T2+.
- **`pr-surfaces.template.md`** — surface × round matrix. Used at T3+.
- **`pr-review-round.template.md`** — per-round review file (header / findings R*N*-*M* / surfaces couvertes / merge-ready Y/N). Overwritten round-to-round; closed rounds archived in the recap's decisions table.

## Updating

If installed via devcontainer-tools, `bash <tools>/update.sh` refreshes the skill files (templates, hooks, scripts, `review-config.example.md`, README, `install-manual.sh`, `master-review.skill.md`). Your `review-config.md` is preserved (`copy_if_missing`).

For standalone installs, manually `cp -r master-review/.` over the existing folder, then re-run `bash install-manual.sh` to re-sync `~/.claude/commands/master-review.md` and `hooks.json`.

## Triggers

The skill auto-invokes on natural-language requests like:

- "fais une review de la PR 1297"
- "review the PR"
- "ultrareview"
- "deep PR review"
- "regarde la PR"

Or invoke explicitly: `/master-review <pr_number>` with optional `--resume`, `--surfaces=A,B`, `--tier-only`, `--read-only`, `--config=<path>`.

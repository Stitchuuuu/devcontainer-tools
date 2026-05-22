# Plans — index

> Snapshot of the unfinished rollouts impacting `devcontainer-tools/`
> at the time of v2 install-redesign delivery (2026-05-22). Completed
> rollouts are listed for reference but their dirs are NOT copied
> here.
>
> For maintainer orientation see [../ROADMAP.md](../ROADMAP.md) and
> [../KNOWLEDGE.md](../KNOWLEDGE.md).

## Active rollouts (copied here, work to resume)

| Rollout | Status | Next focus |
|---|---|---|
| [devcontainer-tools-v2-migration/](devcontainer-tools-v2-migration/) | 2 / 5 delivered | Session 3 — `firewall-layer-split` (move 4 project-specific firewall COPYs out of `Dockerfile.base` into project layer so base stays shared). Then session 4 — `fresh-install-test`. Then session 5 — `bump-changelog`. Part 2 (1.3→2.0 migration prompt) deferred until Part 1 ships. |
| [node-24-bump/](node-24-bump/) | 0 / 1 delivered | Session 1 — `bump-and-verify` : edit 6 files (Dockerfile.base, Dockerfile.php +Sury, verify-slim-base, 3 docs) then host rebuild + 9-gate verify suite. Touches the base image — coordinate with devcontainer-tools-v2 session 4 (CHANGELOG bump) so v2.0 ships against a known Node version. |
| [tune-claude-local-skill/](tune-claude-local-skill/) | 0 / 3 delivered | Session 1 — `skeleton` (CLI skeleton 9 subcommands, `claude-switch` `CLAUDE_LOCAL_PROFILE` env, firewall `domains.d/ollama.txt`, `scenarios.json`, bind-mount docker-compose, archive session-2 profile). Sessions 2-3 build on it. Parent rollout (`uniclaudeproxy-integration-local-opti`) is **complete** ; this is the spin-off to generalise the ad-hoc tuning into a shippable skill. |

## Completed rollouts (NOT copied — reference only)

| Rollout | Outcome |
|---|---|
| `knowledge-split-devcontainer/` | 3 / 3 delivered (2026-05-21). `KNOWLEDGE.md` (single file) split into `knowledge/` directory + `INDEX.md` + topic files. All cross-refs migrated. Templates v2 already inherits the new shape. |
| `devcontainer-v2/phase3-rollout/` | Phase A (firewall L7) + Phase B-D (skills /prepare-pr, /watch-log, /prepare-research, /scan-deps, /prepare-plan) + Phase E (docs) all ✅. Only `v2.1-0 docker prune` remains (deferred — purely host maintenance, not devcontainer-tools concern). |
| `uniclaudeproxy-integration/` | Complete. Shipped `claude-bridge/` sidecar + `host-helpers/claude-switch` + `host-helpers/claude-bridge`. Now part of the v2 baseline. |
| `uniclaudeproxy-integration-local-opti/` | Complete (sessions 1, 2, F2, 4). Delivered the ad-hoc local tuning process that `tune-claude-local-skill/` will generalise. Skill rollout itself is the sibling above. |
| `claude-md-merge/` | Complete. Project-CLAUDE.md cross-merge (NOT devcontainer-tools work — listed here only to clarify the rollout is closed). |

## Dependencies + coordination

- **`devcontainer-tools-v2-migration/` → session 4 (bump-changelog)** should land AFTER `node-24-bump/` session 1 so the v2.0.0 release pins against the new Node version baseline. If node-24-bump slips, ship v2.0.0 against Node 20 and bump to v2.0.1 once Node 24 lands.
- **`tune-claude-local-skill/`** is orthogonal to `devcontainer-tools-v2` — the skill ships into `.devcontainer/skills/` once delivered, then a follow-up template sync brings it into `templates/v2/skills/`. Not a blocker for the v2.0.0 release.
- **No active blockers**. The `wtf` + 4 firewall domains + `knowledge/wtf.md` dependency from `devcontainer-v2/phase3-rollout/` session H was resolved before Part 1 session 2.

## How to use this index

1. Pick the next rollout to resume from "Active rollouts" above.
2. Open its directory — `ROLLOUT.md` for context, `STATUS.md` for the
   actionable session table, `LOG.md` for what's already done, the
   `sessions/<...>.md` for the paste-into-Claude prompt.
3. Sessions are self-contained — copy the prompt block into a fresh
   Claude Code chat and the session resumes from there.
4. End-of-session DoD always updates STATUS.md + LOG.md + creates the
   next session prompt file. Follow that convention.

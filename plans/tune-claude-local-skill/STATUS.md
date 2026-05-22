# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).
>
> **Parent rollout** : [uniclaudeproxy-integration-local-opti](../uniclaudeproxy-integration-local-opti/STATUS.md)
> — sessions 1+2 ✅ delivered there. The parent's session 3 row is
> 📦 EXTRACTED here. Parent's session 4 (docs) is still planned in the
> parent rollout, scope extended to reference this rollout's outputs.

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | skeleton — CLI skeleton 9 sous-commandes (stubs TODO 2+3), claude-switch CLAUDE_LOCAL_PROFILE env, firewall domains.d/ollama.txt, scenarios.json statique, CLAUDE.md doc, .env.example doc, .gitignore tmp, bind-mount docker-compose, archive du profile session-2 comme CLAUDE-local-mac-light-dev.md | 📋 | [→ prompt](sessions/session-1-skeleton.md) |
| 2 | discovery — Impl `ask` / `research` / `propose` / `verify` subcommands. Skill .md phases 1-5 + resume support. AskUserQuestion templates pour HW + modèle. Queries ollama.com + ollama local. | 📋 | [→ prompt](sessions/session-2-discovery.md) |
| 3 | tuning — Impl `baseline` / `apply` / `measure` / `finalize`. Refactor des internals depuis parent rollout session-2 (replay-scenarios.sh, apply-variant.sh). Skill .md phases 6-9. Demo end-to-end. | 📋 | [→ prompt](sessions/session-3-tuning.md) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled · 📦 extracted

## Progress

- **Delivered** : 0 / 3
- **Next focus** : session 1 (skeleton)

## Dependencies between sessions

- **Session 1** depends on parent rollout's sessions 1+2 ✅ (need the
  converged session-2 process + final `CLAUDE-local-dev.md` to archive
  as `CLAUDE-local-mac-light-dev.md`).
- **Session 2** depends on session 1 ✅ (CLI skeleton + firewall ollama.com +
  state dir layout must exist before implementing interactive subcommands).
- **Session 3** depends on session 2 ✅ (ask/research/propose/verify must
  produce inventory + proposal JSON files that baseline/iterate consume).
- **Parent rollout's session 4 (docs)** depends on this rollout being
  complete (3/3) — its scope was extended to document the skill's
  workflow, the `CLAUDE_LOCAL_PROFILE` env, and cross-link both rollouts.

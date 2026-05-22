# Existing — technical inventory

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).
> For the parent rollout's inventory, see
> [../uniclaudeproxy-integration-local-opti/EXISTING.md](../uniclaudeproxy-integration-local-opti/EXISTING.md).

## State at rollout start (2026-05-21)

Parent rollout's sessions 1+2 are delivered. The relevant existing
artifacts that this rollout builds on or wraps :

| File | Role for this rollout |
|---|---|
| `.devcontainer/tests/diag-bridge-translation.sh` | The 268-line capture+replay+measure script from parent session 2. Will be **refactored** as `tune-claude-local-internals/replay-scenarios.sh` in session 3, parameterized by `scenarios.json`. Original kept with a "superseded" header for ad-hoc debugging. |
| `.devcontainer/tests/tweak-claude-md-for-local.sh` | The 207-line idempotent variant applier with `<!-- 1B-LIGHT-MODEL-DIRECTIVES-START/END -->` markers. Will be **refactored** as `tune-claude-local-internals/apply-variant.sh` in session 3, accepting `--body-file` instead of hard-coded HEREDOC variants. |
| `.devcontainer/claude/CLAUDE-local-dev.md` | The session-2 converged profile (V1 explicit-tiny-model, ~1k tokens). Session 1 of this rollout **archives** it as `CLAUDE-local-mac-light-dev.md` ; the unaltered `CLAUDE-local-dev.md` stays as the fallback when `CLAUDE_LOCAL_PROFILE` is unset. |
| `.devcontainer/claude-bridge/config.json` | Bridge config mapping `claude-opus-4-7 → ollama/claude-opus-4-7`. **NOT modified** by this rollout — bridge mapping stays immutable, user does `ollama cp <chosen> claude-opus-4-7` themselves. |
| `.devcontainer/host-helpers/claude-switch` | Existing 316-line mode toggle. Session 1 **patches** the `local-proxy` branch to read `CLAUDE_LOCAL_PROFILE` env (from `.devcontainer/.env`) and symlink to `CLAUDE-local-<name>-dev.md` ; fallback `CLAUDE-local-dev.md` when unset. |
| `.devcontainer/host-helpers/mitm-capture` | Existing host-helper for toggling mitm capture sentinel. Re-used as-is by session 3's `baseline` subcommand (pre-flight auto-`on`, trap to restore prior state). |
| `.devcontainer/firewall/policy.d/` + `domains.txt` + `domains.d/` | Existing firewall infrastructure. Session 1 adds `firewall/domains.d/ollama.txt` with GET allowlist for `ollama.com` + `registry.ollama.ai` (research phase). |
| `.devcontainer/docker-compose.yml` | Existing compose with app + claude-bridge services. Session 1 **verifies/adds** the bind-mount for `.devcontainer/tmp/` so the state directory is shared between host and container. |
| `.devcontainer/skills/` | Existing skills convention : `<name>/<name>.skill.md` with YAML frontmatter (`description`, `argument-hint`) ; copied to `~/.claude/commands/` by `sync-skills.sh` at post-start. Session 2 adds `skills/tune-claude-local/tune-claude-local.skill.md`. |

## Architecture target (after this rollout completes)

```
.devcontainer/
├── host-helpers/
│   ├── tune-claude-local                       (NEW session 1 — CLI orchestrator)
│   ├── tune-claude-local-internals/            (NEW)
│   │   ├── scenarios.json                      (NEW session 1)
│   │   ├── ask-interactive.sh                  (NEW session 2)
│   │   ├── query-ollama.sh                     (NEW session 2)
│   │   ├── run-gate.sh                         (NEW session 2)
│   │   ├── replay-scenarios.sh                 (NEW session 3, refactored from diag-bridge-translation.sh)
│   │   └── apply-variant.sh                    (NEW session 3, refactored from tweak-claude-md-for-local.sh)
│   └── claude-switch                           (MOD session 1 — CLAUDE_LOCAL_PROFILE env)
├── skills/
│   └── tune-claude-local/
│       └── tune-claude-local.skill.md          (NEW sessions 2+3)
├── claude/
│   ├── CLAUDE-local-dev.md                     (existing — fallback when CLAUDE_LOCAL_PROFILE unset)
│   ├── CLAUDE-local-mac-light-dev.md           (NEW session 1 — archive session-2 converged profile)
│   └── CLAUDE-local-<other>-dev.md             (produced runtime by `finalize`)
├── firewall/domains.d/
│   └── ollama.txt                              (NEW session 1)
├── tests/
│   ├── diag-bridge-translation.sh              (existing — header annotated "superseded" in session 3)
│   └── tweak-claude-md-for-local.sh            (existing — header annotated "superseded" in session 3)
├── tmp/tune-claude-local/<profile>/            (gitignored, bind-mounted runtime state)
├── .env.example                                (MOD session 1 — CLAUDE_LOCAL_PROFILE doc)
└── docker-compose.yml                          (MOD session 1 if bind-mount tmp missing)
plans/
├── tune-claude-local-skill/                    (this rollout)
└── uniclaudeproxy-integration-local-opti/      (parent — session 3 marked extracted, session 4 docs scope extended)
CLAUDE.md                                       (MOD session 1 — "Local mode tuning" subsection)
.gitignore                                      (MOD session 1 — .devcontainer/tmp/)
```

## Reusable patterns identified

| Pattern | Source | Reused by |
|---|---|---|
| Skill `<name>/<name>.skill.md` + YAML frontmatter (`description`, `argument-hint`) | [`.devcontainer/skills/watch-log/watch-log.skill.md`](../../.devcontainer/skills/watch-log/) | Sessions 2+3 (the `tune-claude-local.skill.md` itself) |
| Host-only guard `/.dockerenv` | [`.devcontainer/host-helpers/claude-switch:42-47`](../../.devcontainer/host-helpers/claude-switch) | Session 1 (`tune-claude-local` CLI) |
| Bind-mount + state-dir-per-instance pattern | [`/tmp/claude-capture/`](../../.devcontainer/firewall/addons/capture_messages_debug.py) | Session 1 (`.devcontainer/tmp/tune-claude-local/<profile>/`) |
| Cloud SEND capture via mitm + replay against bridge | [parent rollout session 2](../uniclaudeproxy-integration-local-opti/LOG.md) | Session 3 (`replay-scenarios.sh` factored from `diag-bridge-translation.sh`) |
| Idempotent markers for auto-managed file sections | [`<!-- 1B-LIGHT-MODEL-DIRECTIVES-START/END -->`](../../.devcontainer/claude/CLAUDE-local-dev.md) | Session 3 (`apply-variant.sh` reuses the marker convention from parent's `tweak-claude-md-for-local.sh`) |
| Env-var-driven mode toggle in `.devcontainer/.env` | [`ANTHROPIC_BASE_URL` in claude-switch](../../.devcontainer/host-helpers/claude-switch) | Session 1 (`CLAUDE_LOCAL_PROFILE` env addition) |
| Firewall ecosystem-scoped allowlist `domains.d/<eco>.txt` | [`.devcontainer/firewall/domains.d/`](../../.devcontainer/firewall/domains.d/) | Session 1 (`domains.d/ollama.txt`) |
| 5-backtick fences for session prompts | [`sessions/session-1-plumbing.md` parent rollout](../uniclaudeproxy-integration/sessions/session-1-plumbing.md) | All session prompts of this rollout |

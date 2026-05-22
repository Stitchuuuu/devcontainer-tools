# Rollout — Tune Claude Local Skill

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).
>
> **Parent rollout** : [uniclaudeproxy-integration-local-opti](../uniclaudeproxy-integration-local-opti/ROLLOUT.md)
> — this rollout extracts the "session 3" wrap of the session-2 ad-hoc
> tuning process into its own dedicated rollout (3 sessions). The parent
> rollout retains sessions 1 + 2 (delivered) and session 4 (docs, still
> planned — its scope is extended to reference this rollout's outputs).

## Goal

Transform the ad-hoc tuning process delivered in session 2 of the
parent rollout into a **reusable skill + host-helper** that produces
`CLAUDE-local-<profile>-dev.md` for any combo Ollama model + hardware
profile, **interactively and step-by-step**, with **resume support** at
every phase.

User-facing entry point (host-side) :

```bash
bash .devcontainer/host-helpers/tune-claude-local ask <profile-name>
```

The accompanying skill `.devcontainer/skills/tune-claude-local/` guides
the running Claude Code session through 9 phases :

```
1. ask        → user types target model intent + HW (RAM/VRAM/CPU/GPU)
2. research   → helper queries Ollama local + ollama.com (model facts)
3. propose    → Claude (in session) reasons → recommendations
4. SETUP      → user runs ollama pull + ollama cp <X> claude-opus-4-7
5. verify     → helper checks Ollama alias + bridge /health + Phase 0 gate
6. baseline   → run 4-scenario battery (ping/reasoning/file-extract/tool-discipline)
7. iterate    → Claude proposes variants, user Y/n, apply, re-measure
8. finalize   → write CLAUDE-local-<profile>-dev.md + session log
9. status     → `tune-claude-local status <profile>` for resume at any phase
```

End artifact : an archived `CLAUDE-local-<profile>-dev.md` activatable
via `CLAUDE_LOCAL_PROFILE=<profile>` in `.devcontainer/.env` (read by
`claude-switch local-proxy` — fallback `CLAUDE-local-dev.md` when unset).

## Why a separate rollout (not folded into uniclaudeproxy-local-opti)

Three independent reasons :

- **Scope clarity** : the parent rollout is "make local-proxy mode
  usable + document the proxy itself". This rollout is "build a
  meta-tool that automates the tuning process for ANY future
  hardware+model combo". Mixing them would bloat the parent's LOG.md
  and obscure both stories.
- **Meta-level scope** : the skill's portée exceeds uniclaudeproxy
  stricto sensu — it applies to any local Ollama-backed model, not
  just the current `claude-opus-4-7` alias. Treating it as a sibling
  rollout reflects the separation of concerns.
- **Effort fit** : the wrap grew from "~60-90 min" to ~3-4h end-to-end
  during planning (workflow 9 étapes interactif resumable, query
  ollama.com via firewall, env var pour profile actif, batterie 4
  scénarios, intégration matos + modèle research). A dedicated rollout
  with 3 sessions of ~60-90 min each is healthier than a single
  monster session under the parent.

## Architecture target (after this rollout completes)

```
.devcontainer/
├── host-helpers/
│   ├── tune-claude-local                       (NEW session 1 — CLI orchestrator, 9 subcommands)
│   ├── tune-claude-local-internals/            (NEW — internals dir, populated through 3 sessions)
│   │   ├── scenarios.json                      (NEW session 1 — 4-test battery spec, static)
│   │   ├── ask-interactive.sh                  (NEW session 2 — HW + model intent prompts)
│   │   ├── query-ollama.sh                     (NEW session 2 — ollama local + ollama.com queries)
│   │   ├── run-gate.sh                         (NEW session 2 — Phase 0 streaming <think> gate)
│   │   ├── replay-scenarios.sh                 (NEW session 3 — generalized from diag-bridge-translation.sh)
│   │   └── apply-variant.sh                    (NEW session 3 — refactor from tweak-claude-md-for-local.sh)
│   └── claude-switch                           (MOD session 1 — reads CLAUDE_LOCAL_PROFILE env)
├── skills/
│   └── tune-claude-local/
│       └── tune-claude-local.skill.md          (NEW sessions 2+3 — workflow Claude follows)
├── claude/
│   ├── CLAUDE-local-dev.md                     (existing — default fallback when CLAUDE_LOCAL_PROFILE unset)
│   ├── CLAUDE-local-mac-light-dev.md           (NEW session 1 — archive of the session-2 converged profile)
│   └── CLAUDE-local-<profile>-dev.md           (per-profile, produced by `tune-claude-local finalize`)
├── firewall/domains.d/
│   └── ollama.txt                              (NEW session 1 — GET allowlist for ollama.com + registry)
├── tmp/tune-claude-local/<profile>/            (gitignored, bind-mounted — runtime state per profile)
│   ├── state.json                              (current phase tracker for resume support)
│   ├── inventory.json                          (Phase 1 - HW + model intent)
│   ├── research.json                           (Phase 2 - Ollama lookups)
│   ├── proposal.json                           (Phase 3 - recommendations)
│   ├── verify.json                             (Phase 5 - bridge + Ollama + gate status)
│   ├── baseline.json                           (Phase 6a - 4-scenario measurements)
│   ├── iter-NNN.json                           (Phase 6b - per-iteration measurements)
│   └── final.md                                (Phase 8 - rendered CLAUDE-local-<profile>-dev.md)
├── .env.example                                (MOD session 1 — document CLAUDE_LOCAL_PROFILE)
└── docker-compose.yml                          (MOD session 1 — bind-mount tmp if missing)
```

## Navigation

| File | When to open |
|---|---|
| **[STATUS.md](STATUS.md)** | "Where are we, what's next ?" — actionable session table |
| **[LOG.md](LOG.md)** | "What was done, why, what gotchas ?" — append-only journal |
| **[EXISTING.md](EXISTING.md)** | "What does the code look like today ?" — factual inventory |
| sessions/session-NN-*.md | Prompt to paste into a new Claude chat to start session NN |

## How to use

1. **To resume work** : open [STATUS.md](STATUS.md), find the next 📋
   session, click `→ prompt` and paste into a fresh Claude Code session.
2. **To check what was done before** : read [LOG.md](LOG.md).
3. **To understand current code state** : read [EXISTING.md](EXISTING.md).
4. **Cross-rollout context** : read parent rollout
   [LOG.md sessions 1+2](../uniclaudeproxy-integration-local-opti/LOG.md)
   for the original ad-hoc tuning process that this skill generalizes.

## Update convention (end of every delivered session)

Every session prompt prescribes these three updates in its DoD :

1. **STATUS.md** : flip the session row 📋 → ✅, replace the prompt link
   with `—`, bump the "Delivered" counter, refresh "Next focus".
2. **LOG.md** : append `## <Session ID> — <Title>` section dated today,
   listing files touched + What / Why / Decisions / Gotchas / Tests /
   Commit (~50–150 lines).
3. **EXISTING.md** : update if new files / structures were created.

No companion skill, no automated hook — the session itself does the work
because its prompt explicitly says so.

## Decisions (immutable unless user explicitly amends)

- **2026-05-21** : split skill work from parent rollout
  `uniclaudeproxy-integration-local-opti` into this dedicated companion.
  Rationale : scope grew beyond uniclaudeproxy stricto sensu (~3-4h end
  to end, meta-tool applicable to any future hw+model combo). Parent's
  session 3 row marked `📦 extracted to plans/tune-claude-local-skill/`.
- **2026-05-21** : no auto-detect of hardware specs (VRAM, RAM, CPU,
  GPU). The `ask` phase poses open questions to the user, who types
  values directly. Rationale : multi-platform reliability of
  auto-detection is brittle, and user-typed values give a cleaner
  contract for downstream `research` + `propose` logic.
- **2026-05-21** : no patching of `.devcontainer/claude-bridge/config.json`
  by the helper. Bridge keeps its immutable mapping
  `claude-opus-4-7 → ollama/claude-opus-4-7`. The user is expected to
  do `ollama cp <chosen-model> claude-opus-4-7` themselves (Phase 4
  manual step). Rationale : avoids race conditions with concurrent
  config edits, keeps the helper's footprint narrow and reversible.
- **2026-05-21** : workflow is step-by-step **resumable**. Each
  subcommand persists JSON state to
  `.devcontainer/tmp/tune-claude-local/<profile>/state.json`. Any
  subcommand can be re-run ; later subcommands check their prerequisite
  state and refuse with a clear "run X first" message if not satisfied.
  Rationale : the workflow may span multiple sittings (user has to
  install/pull Ollama models between phases), so atomic resume points
  are essential.
- **2026-05-21** : model research uses **ollama.com directly** (GET via
  firewall allowlist extension `domains.d/ollama.txt`), not Claude's
  training knowledge. Rationale : authoritative source of truth for
  model availability + variants + quantizations + sizes ; firewall
  extension is low-risk (GET only).
- **2026-05-21** : if Ollama local (`ollama.internal:11434`) is
  unreachable during `research` or `verify`, the skill **asks the user
  to activate it** via `AskUserQuestion` (`ollama serve` or
  `brew services start ollama` on host). No `/watch-log` fallback, no
  auto-start. Rationale : explicit user action, clean retry semantics,
  consistent with the rest of the host-side responsibility model.
- **2026-05-21** : `CLAUDE_LOCAL_PROFILE` environment variable in
  `.devcontainer/.env` selects which `CLAUDE-local-<name>-dev.md`
  `claude-switch local-proxy` symlinks to. Unset → fallback
  `CLAUDE-local-dev.md` (existing default, unchanged behavior).
  Rationale : minimal patch on claude-switch, env-driven activation
  matches the existing pattern (ANTHROPIC_BASE_URL, etc.).
- **2026-05-21** : **variant proposal lives in the skill .md** (Claude
  reasons in-session), NOT in a subprocess like `claude --print`. The
  helper is a deterministic data collector ; the skill markdown
  instructs the active Claude session how to interpret reports and
  propose variants. Rationale : avoids dependency on cloud creds during
  tuning runs, leverages the current session's full context for richer
  variant proposals.
- **2026-05-21** : 4 default test scenarios committed to the battery :
  (a) ping ≤ 100 chars / ≤ 30 s, (b) reasoning math 17×23=391 +
  `thinking` content block / ≤ 90 s, (c) file-extract (read
  `robrowser/INDEX.md`, list plugins on one comma-separated line) /
  ≤ 60 s, (d) tool-discipline (Y/N answer without any tool_use block) /
  ≤ 30 s. Rationale : ping + reasoning are already validated in session
  2 of parent ; file-extract tests real read+extract behavior ;
  tool-discipline catches the "small model boulimic tool-calling"
  failure mode.

## Parking lot — explicit out-of-scope

- Auto-detection of HW (via `system_profiler` / `lscpu` / `nvidia-smi`)
  — see decision #2 above. Open question to the user instead.
- Patching `.devcontainer/claude-bridge/config.json` from the helper
  — see decision #3 above. User does `ollama cp` themselves.
- Subprocess `claude --print` for variant proposal — see decision #7
  above. Skill .md drives in-session reasoning.
- Auto-start / `/watch-log` fallback for Ollama unreachable — see
  decision #6 above. AskUserQuestion gate instead.
- CI/regression suite re-evaluating per-profile files on every UCP /
  Claude Code release — useful but out of scope ; manual re-run of
  `tune-claude-local` on the relevant profile is the v1 contract.
- Publication of the `tune-claude-local` skill upstream (Anthropic
  claude-code-skills repo) — external decision.
- `claude-switch local-proxy <profile>` positional arg (instead of env
  var) — see decision #7 of parent rollout's STATUS.md. Env var is the
  picked option ; positional could be a future v2.

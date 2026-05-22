# Rollout — Devcontainer Tools V2 Migration

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the scope of Part 1, see
> [SCOPE.md](SCOPE.md). For the legacy inventory snapshot, see
> [EXISTING.md](EXISTING.md).

## Goal

Bring `/workspace/devcontainer-tools/` (currently v1.3.0, `install.sh`
marked `TEMPLATE_VERSION="1.2.0"`) up to **v2.0.0**, leveraging the v2
work done directly in the current project's `.devcontainer/`.

The rollout is split in **two parts** — Part 1 ships, Part 2 follows
later :

### Part 1 — `install.sh` v2 for new projects (priority)

Rewrite `install.sh` so that running it against a fresh project drops
the v2 baseline (~84 files : core build + lifecycle + firewall +
policy.d/ baseline + 5 generic skills + 5 claude/ rules + knowledge/
+ README/RUNBOOK/SECURITY/RESEARCH + claude-bridge/ sidecar +
host-helpers/) ready to `Reopen in Container`. The
wizard collapses from 13 prompts to ~4 because v2 files use shell
variable expansion (`${DC_PROJECT:-...}`) instead of `{{PLACEHOLDER}}`
+ sed substitution. Only `devcontainer.json` and `.env.example` need
text templating.

See [SCOPE.md](SCOPE.md) for the exhaustive file list and the
templating model.

### Part 2 — Claude session prompt for 1.3 → 2.0 migration (deferred)

The v1.3 `update.sh` (16 KB, full-resync with diff) is too fragile
for a major migration. Instead, ship a **paste-into-Claude session
prompt** that walks an existing 1.3 project through the upgrade,
handling reconciliations file by file with human-in-the-loop
validation. The prompt itself is the deliverable.

Spec frozen later — Part 1 is the priority.

## Drops in v2.0.0 (breaking changes)

- `templates/gh-secure/` (6 scripts) — archived in Phase 3 A3, the
  `/prepare-pr` skill replaced it
- `templates/Dockerfile.node` — unused
- `templates/skills/master-review/` — superseded by `/prepare-pr`
- `templates/KNOWLEDGE.md` (single file) — superseded by
  `knowledge/` directory
- `templates/test-db.php` — unused
- `templates/gitignore-entries.txt` — install.sh embeds inline now

Rename : `templates/Dockerfile.custom` → `templates/Dockerfile`.

## Navigation

| File | When to open |
|---|---|
| **[STATUS.md](STATUS.md)** | "Where are we, what's next ?" — actionable session table |
| **[LOG.md](LOG.md)** | "What was done, why, what gotchas ?" — append-only journal |
| **[SCOPE.md](SCOPE.md)** | "What ships in install.sh v2 ?" — Part 1 file inventory |
| **[EXISTING.md](EXISTING.md)** | "What did v1.3 look like ?" — legacy snapshot |
| sessions/part-N-session-M-*.md | Prompt to paste into a new Claude chat to start session M |

## How to use

1. **To resume work** : open [STATUS.md](STATUS.md), find the next 📋
   session, click `→ prompt` and paste into a fresh Claude Code session.
2. **To check what was done before** : read [LOG.md](LOG.md).
3. **To understand Part 1 scope** : read [SCOPE.md](SCOPE.md).

## Update convention (end of every delivered session)

Every session prompt prescribes these three updates in its DoD :

1. **STATUS.md** : flip the session row 📋 → ✅, replace the prompt link
   with `—`, bump the "Delivered" counter, refresh "Next focus".
2. **LOG.md** : append `## <Session ID> — <Title>` section dated today,
   listing files touched + What / Why / Decisions / Gotchas / Tests /
   Commit (~50–150 lines).
3. **SCOPE.md** : update if the in/out scope shifts (rare after
   session 1 freeze).

## Decisions (immutable unless user explicitly amends)

- **Two-part rollout** — Part 1 (install.sh for new projects) is the
  priority. Part 2 (Claude session prompt for 1.3→2.0 migration) is
  deferred. The original file-per-file porting plan (sessions 2-7) is
  abandoned : Part 1 produces a refreshed `install.sh` + new
  `templates/` based on the v2 baseline, in one cohesive design rather
  than seven incremental ports.
- **Bump = major (`v2.0.0`).** Rationale : pruning `gh-secure/` +
  `Dockerfile.node` + `master-review/` is breaking. A major bump signals it.
- **`Dockerfile.custom` renamed `Dockerfile`.** Rationale : the v2
  baseline uses the "custom" path as default — promote it.
- **`templates/knowledge/` mirrors `.devcontainer/knowledge/` fully**
  (6 files). Claude needs this to operate the devcontainer it runs in.
- **`gh-secure/`, `Dockerfile.node`, `master-review/` removed, not
  deprecated.** Rationale : deprecated-but-shipped stays adopted ; only
  removal forces migration.
- **Templating model = shell expansion + 3 placeholders** (down from 11
  in v1.3). `{{PROJECT_ID}}`, `{{PROJECT_DISPLAY_NAME}}`,
  `{{TIMEZONE}}` only. Everything else uses `${VAR:-default}` resolved
  at runtime from `.env`. See [SCOPE.md](SCOPE.md).
- **`.local` skills not shipped by install.sh** (`hours.local`,
  `claude-limits.local`). Per-user preference, not generic. Users add
  them manually post-install.

## Blockers

- ⚠️ **Session H of `devcontainer-v2/phase3-rollout` must be delivered
  first.** It ships the `wtf` binary + 4 firewall domains +
  `knowledge/wtf.md`. Part 1 session 2 (install-redesign) needs these
  to be already baked into the v2 baseline files it copies. If H is not
  done, Part 1 session 1 (scope-audit) still proceeds — it documents H
  as a dependency.

## Reference paths

- `/workspace/devcontainer-tools/` — the target repo (v1.3.0 today)
- `/workspace/.devcontainer/` — the v2 source of truth
- `/workspace/plans/knowledge-split-devcontainer/` — sibling rollout
  (delivered ; v2 baseline already uses `knowledge/`)
- `/home/node/.claude/plans/on-est-en-train-scalable-flame.md` — the
  meta-plan that scaffolded both rollouts

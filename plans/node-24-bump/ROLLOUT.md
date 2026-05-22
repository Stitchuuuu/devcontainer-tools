# Rollout — Node 24 Bump

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).

## Goal

Migrate the devcontainer v2.1 base image from `node:20-slim` to `node:24-bookworm-slim`. Premise revised mid-session : the original plan assumed `node:24-slim` would carry a distro jump to trixie / glibc 2.40, but Docker Hub still ships `node:24-slim` on bookworm today. Concrete diff is now a pure Node 20 → 24 bump on the same Debian 12 / glibc 2.36 base. The base tag is **pinned explicitly** to `node:24-bookworm-slim` (not the floating `node:24-slim`) so a future upstream rebase to trixie becomes a deliberate bump rather than a silent CI break.

The Sury APT repo wire-up on the PHP variant is kept as forward-compat for that eventual trixie jump (it also serves bookworm with newer point releases, e.g. observed 8.2.31 vs bookworm main's older patch level) — distro-agnostic via `lsb_release -sc`.

Driver: Node 20 reached EOL on **2026-04-30** (3 weeks before this rollout was opened).

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

- **PHP 8.2 preserved (not bumped to 8.3 or 8.4).** Trade-off : Sury repo adds a single 3rd-party APT source (trusted — Sury is the official Debian PHP maintainer) as forward-compat for a future trixie base AND for fresher point releases on bookworm (8.2.31 observed vs bookworm main's older patch). Sury also gives a single-line bump path to 8.3 / 8.4 later if needed. PHP 8.2 EOL is December 2026 — re-evaluation deadline.
- **Bookworm pin (`node:24-bookworm-slim`) rather than floating `node:24-slim`.** Decision taken mid-rollout after the trixie premise turned out false : the floating tag would silently rebase to trixie someday, breaking our docs + assumptions without warning. The pin makes a future trixie move a deliberate bump session. Cost : we miss any silent bug-fix Debian rebase Docker Hub does — acceptable trade-off.
- **No `apt preferences pinning` initially.** Sury and Debian main can coexist because Sury's php8.2-* outranks bookworm's by version (Sury carries newer point releases). A pinning file is only added if a real conflict surfaces during build.
- **Single-session rollout despite the scaffold.** The user requested the multi-session scaffold structure for traceability, but the implementation fits in one session (6 file edits + 1 rebuild + verification on the host). Future sessions get added rows in STATUS.md if follow-up work surfaces.
- **Size docs are updated post-rebuild, not pre.** The new baseline (currently ~1.1 GB on node:20-slim) is unknown until the host rebuild produces a number. Session 1's DoD includes editing the docs with the measured value, not a guess.
- **`CLAUDE_CODE_VERSION` bump explicitly out of scope.** Same for `MITM_VERSION`, `GIT_DELTA_VERSION`, `WTF_VERSION`. Grouping bumps multiplies the surface for regressions.

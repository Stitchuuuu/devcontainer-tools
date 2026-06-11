# Rollout — Updates v2.1 integration

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).

## Goal

Integrate the 15 patches sitting in `/workspace/updates-v2.1/` into both
shipped surfaces of this repo :

1. **`templates/v2/`** — the source of truth for the template that
   `install.sh` materialises into downstream projects. Patches were
   produced on a fork dogfood ("Symptems"), so project-specific values
   (`Symptems`, `symptems`) must be **re-templatised** to
   `{{PROJECT_DISPLAY_NAME}}` / `{{PROJECT_ID}}` before landing here.
2. **`.devcontainer/`** — the dogfood instance of this very repo
   (already configured with `DC_PROJECT=devcontainer-tools`). Patches
   land with concrete values (no placeholders).

Two side-effects of this integration :

- **Version bump** : `TEMPLATE_VERSION` 2.0.0 → 2.1.0 in `install.sh`,
  matching `**Version: v2.1.0**` line in both `templates/v2/README.md`
  and `.devcontainer/README.md` (this line becomes the canonical sync
  marker each downstream project reads to know when to re-sync), and a
  new `## 2.1.0 (2026-06-11)` entry in `CHANGELOG.md`.
- **Installer audit** : the user reported having to hand-edit
  `PROJECT_ID` twice in two different projects, suggesting `install.sh`
  may not be substituting every `{{PROJECT_ID}}` / `{{PROJECT_DISPLAY_NAME}}`
  occurrence. A systematic audit (grep `templates/v2/` for all `{{...}}`
  → cross-check against `copy_templated()` in `install.sh:247-310`) is
  baked into session 5.

Original analysis plan (rejected vocabulary, retained as reference) :
`/home/node/.claude/plans/j-aurais-besoin-que-tu-expressive-spark.md`.

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

These were settled during the planning phase that produced this rollout —
sessions should NOT relitigate them.

- **Version stamp = `v2.1.0`** (matches `updates-v2.1/` directory name).
  Bump `install.sh:TEMPLATE_VERSION`, README headers, and `CHANGELOG.md`
  in a single coordinated step (session 6).
- **6 thematic sessions, not 15-per-patch** : the patches group naturally
  into doc/cosmetic (S1), shell+settings (S2), Dockerfile cache (S3),
  notify daemon (S4), per-project tagging + installer audit (S5),
  version+CHANGELOG (S6). Per-patch granularity would multiply boilerplate
  on trivial changes.
- **Apply chronologically via `git am`** on the `.devcontainer/` side
  so superseded patches (#8, #9) get naturally overwritten by their
  replacements (#10, #13). Side `templates/v2/` only carries the **final
  state** (no intermediate rewrites).
- **Re-templatisation map** : `Symptems` → `{{PROJECT_DISPLAY_NAME}}`,
  `symptems` → `{{PROJECT_ID}}`. `PROJECT_NAME` in `notify/` `.js` files
  becomes a placeholder substituted by `install.sh:copy_templated()`,
  same mechanism as `devcontainer.json` / `docker-compose.yml`. **Not**
  read dynamically at runtime — substitution happens at install-time.
- **`.devcontainer/` keeps `DC_PROJECT=devcontainer-tools`** as its
  dogfood identity. Patch #15 (`ARG DC_PROJECT=symptems`) lands as
  `ARG DC_PROJECT=devcontainer-tools` in the dogfood and as
  `ARG DC_PROJECT={{PROJECT_ID}}` in the template.
- **Patch #14 (firewall-docker-setup-add)** : the file already exists in
  both targets, so the patch is content-wise a no-op for us. But its
  existence in `updates-v2.1/` is a tell : verify `install.sh` actually
  copies it to downstream projects (session 5 audit).
- **Installer audit scope (session 5)** : `grep -rln "{{" templates/v2/`
  must match `copy_templated()` invocations exactly. Any file with a
  placeholder must be in the templated list ; any leftover `{{...}}` in
  a fresh install is a bug that gets fixed in session 5, not deferred.
- **Each session touches both surfaces simultaneously** : a patch lands
  in `templates/v2/` (templatised) AND `.devcontainer/` (concrete values)
  within the same session. Divergence between surfaces is the failure
  mode we are actively guarding against.

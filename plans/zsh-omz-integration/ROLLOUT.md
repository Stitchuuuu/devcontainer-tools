# Rollout — Zsh OMZ Integration

> Entry point of this plan directory. For the actionable session table,
> see [STATUS.md](STATUS.md). For the reasoned journal of delivered
> sessions, see [LOG.md](LOG.md). For the technical inventory, see
> [EXISTING.md](EXISTING.md).

## Goal

Add an Oh My Zsh team-wide base to the devcontainer (theme + git plugin
+ history opts + zsh-autosuggestions + zsh-syntax-highlighting + wtf
autocomplete), with a per-dev override mechanism via
`.devcontainer/zshrc.local` (gitignored, persisted with the workspace)
and persistent per-dev custom plugins via `.devcontainer/.zsh-custom/`
(ZSH_CUSTOM redirected). Today the devcontainer ships `zsh` as default
shell but vanilla — no theme, no plugins, no enhanced history.

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

- **Framework = Oh My Zsh** (not Starship, not bare zsh). User explicitly
  picked it for familiarity with their personal zshrc. Trade-off accepted :
  ~30 MB baked in image vs. instant prompt with rich git integration.
- **Extra plugins bakés = `zsh-autosuggestions` + `zsh-syntax-highlighting`**.
  These are the visible upgrade vs. vanilla OMZ (Fish-style grey
  suggestions + green/red command coloring). ~1 MB combined.
- **Per-dev mechanism = `.devcontainer/zshrc.local`** (gitignored, in
  workspace). Pattern aligned with existing `.devcontainer/LESSONS.local.md`.
  No Docker named volume → simpler, no extra mount to manage.
- **`ZSH_CUSTOM = /workspace/.devcontainer/.zsh-custom/`** — redirects the
  custom OMZ dir to the workspace, so per-dev plugin installs (e.g.
  `git clone ... $ZSH_CUSTOM/plugins/zsh-z`) survive container rebuilds.
- **Default theme = `robbyrussell`** (OMZ default, neutral). User's
  personal preference `eastwood` documented as opt-in in
  `zshrc.local.example`.
- **Aliases personnels du zshrc source NOT ported to base** (`drush`,
  `composer`, `mountd`, `tree=ncdu`, `dev()`, `wsl-alias()`, secrets like
  `LMSTUDIO_API_KEY`). They go in `zshrc.local` per-dev or
  project-specific `.wtfcmd.yaml`.
- **`wtf --autocomplete setup` INCLUDED in base** — `wtf` is baked in the
  image ([Dockerfile.base:104-114](../../templates/v2/Dockerfile.base#L104-L114)),
  so its completion is universal for any user.
- **Dual-edit pattern between `templates/v2/` and `.devcontainer/`** —
  no `install.sh` re-run on `/workspace` during this rollout. Every
  template change is applied in both locations. Justification :
  `install.sh` re-prompts PROJECT_ID with `slugify(basename("/workspace"))
  = "workspace"`, diverging from the stored `"devcontainer-tools"`, which
  would corrupt `{{PROJECT_ID}}` substitutions across devcontainer.json,
  docker-compose.yml, and 6 other templated files. Full audit in
  `/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md` ("Mode
  d'exécution" section). The shippable test of `install.sh` will happen
  later on a throwaway project, outside this rollout's scope.

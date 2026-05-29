# Session 2 — dockerfile-omz (dual-edit)

> **Effort** : ~0.5 day | **Dependencies** : session 1 delivered (commits `9fa7d25` + `40116ec` on main)

## Prompt to paste

`````
I'm starting session 2 of the `zsh-omz-integration` rollout.

Entry point : `/workspace/plans/zsh-omz-integration/ROLLOUT.md`
Read also :
- `STATUS.md` — session 1 ✅, session 2 📋 (this one), session 3 📋 (verify-rebuild)
- `LOG.md` — session 1 entry with full context
- `EXISTING.md` — current code inventory
- `sessions/session-2-dockerfile-omz.md` (this spec)
- `/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md` (master plan)

## What session 1 delivered

- `templates/v2/zshrc-base` + `.devcontainer/zshrc-base` — team-wide zsh
  config that sources `$ZSH/oh-my-zsh.sh`
- `templates/v2/zshrc.local.example` + `.devcontainer/zshrc.local.example`
- `templates/v2/shell-init.sh` + `.devcontainer/shell-init.sh` — head block
  sources zshrc-base, tail block sources zshrc.local (both zsh+interactive
  gated)
- `templates/v2/.gitignore` + `.devcontainer/.gitignore` — `zshrc.local` +
  `.zsh-custom/` ignored
- `install.sh` — copies the two new files into adopting projects'
  `.devcontainer/`

OMZ itself is NOT installed in the image yet → opening a zsh shell today
prints a "no such file or directory" on `source $ZSH/oh-my-zsh.sh` but
shell-init.sh keeps going (error is non-fatal). That ends today.

## ⚠️ Dual-edit pattern (still in force)

No `install.sh` re-run on `/workspace` — its wizard re-prompts PROJECT_ID
with a default (`workspace`) diverging from the stored value
(`devcontainer-tools`), which corrupts `{{PROJECT_ID}}` substitutions
across 8 templated files. Full audit in
`/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md` ("Mode
d'exécution" section).

For every edit to `templates/v2/Dockerfile.base`, apply the **same edit**
to `/workspace/.devcontainer/Dockerfile.base`. Run
`diff -q templates/v2/Dockerfile.base .devcontainer/Dockerfile.base`
BEFORE editing to confirm they're synchronized — if pre-existing drift
exists, stop and surface it.

## Session focus : install OMZ + 2 plugins in the base image

Concretely :

1. **Pre-edit sync check** :
   ```
   diff -q templates/v2/Dockerfile.base .devcontainer/Dockerfile.base
   ```
   Must show no diff. If it diffs, stop and report.

2. **Edit `templates/v2/Dockerfile.base`** — insert a new RUN block just
   after `USER $USERNAME` (currently at L271) and BEFORE the existing
   `RUN echo '[ -f /workspace/.devcontainer/shell-init.sh ]...'` line
   (L273-274). The new block runs AS NODE (the .oh-my-zsh tree must be
   owned by node).

   The block :

   ```dockerfile
   # -----------------------------------------------
   # Oh My Zsh + extra plugins (autosuggestions, syntax-highlighting)
   # Installed under $HOME/.oh-my-zsh — volatile across container
   # rebuilds, but baked into the image so first boot is instant.
   # ZSH_CUSTOM is redirected to /workspace/.devcontainer/.zsh-custom by
   # zshrc-base, so per-dev plugins survive rebuilds via the workspace.
   #
   # --unattended  : OMZ installer does not spawn an interactive shell.
   # rm -f .zshrc* : the installer writes a default .zshrc — remove it
   #                 so the next RUN block (shell-init injection)
   #                 starts from an empty file.
   # --depth=1     : shallow clone for the two plugins (~1 MB combined
   #                 vs ~5-10 MB full history).
   # -----------------------------------------------
   RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended \
    && rm -f $HOME/.zshrc $HOME/.zshrc.pre-oh-my-zsh \
    && git clone --depth=1 https://github.com/zsh-users/zsh-autosuggestions \
         $HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions \
    && git clone --depth=1 https://github.com/zsh-users/zsh-syntax-highlighting \
         $HOME/.oh-my-zsh/custom/plugins/zsh-syntax-highlighting
   ```

   Note : the firewall does NOT apply at `docker build` time (firewall
   is set up at container start via init-firewall.sh, AFTER the build).
   So `raw.githubusercontent.com` + `github.com/zsh-users/*` are free
   to fetch during build — no domains.txt change needed for this
   session.

3. **Dual-edit** : apply the identical block to
   `/workspace/.devcontainer/Dockerfile.base`.

4. **Post-edit sync check** :
   ```
   diff -q templates/v2/Dockerfile.base .devcontainer/Dockerfile.base
   ```
   Must show no diff.

5. **DO NOT rebuild the container in this session.** Session 3 owns
   end-to-end verification (rebuild + checklist : prompt, autosuggest,
   history, per-dev override, plugin persistence, bash fallback). If
   you rebuild now and something fails, the iteration loop bleeds into
   session 2 — keep them separate.

6. **DO NOT touch any other file** in `templates/v2/` or
   `.devcontainer/` except `Dockerfile.base` in both locations.

Surface-level scope guard : edits restricted to exactly TWO files :
- `templates/v2/Dockerfile.base`
- `.devcontainer/Dockerfile.base`

If you find drift or need to touch anything else (e.g. .env, domains.txt,
shell-init.sh), STOP and surface it as a new STATUS.md row.

## DoD at the end of this session

1. **Sync check** : `diff -rq templates/v2/ .devcontainer/` shows only the
   pre-existing drift listed in EXISTING.md (firewall/, knowledge/
   firewall.md, .gitignore-root, templated files, runtime artefacts).
   `Dockerfile.base` must NOT appear in the diff.
2. **STATUS.md** : flip session 2 row 📋 → ✅, prompt link → —, bump
   Delivered counter (1 → 2), refresh "Next focus" to session 3
   (verify-rebuild).
3. **LOG.md** : append `## 2 — dockerfile-omz` section dated today with
   files touched (just `Dockerfile.base` ×2), What / Why / Decisions /
   Gotchas / Tests / Commit.
4. **EXISTING.md** : update the "Status after session 1" → "Status after
   session 2" — OMZ framework / theme / plugins / compinit / wtf
   completion all flip from ❌ or ⏳ to ✅ baked but inert until rebuild.
5. **Propose 2 commits** (templating + container split, per the
   precedent from session 1) :
   - **Commit 1 — templating** : `templates/v2/Dockerfile.base` +
     `plans/zsh-omz-integration/*.md` (the rollout file updates from
     this session)
   - **Commit 2 — container** : `.devcontainer/Dockerfile.base`
   - Suggested message style for commit 1 :
     `feat(template): install Oh My Zsh + autosuggest + syntax-highlight in base image`
   - Suggested message style for commit 2 :
     `chore(dogfood): apply OMZ install to .devcontainer/Dockerfile.base`
6. Do NOT commit without explicit user confirmation.
`````

## Next session

After session 2 lands, create `sessions/session-3-verify-rebuild.md` with
the full verification checklist (rebuild container, then validate :
prompt has OMZ + git branch, autosuggestions work, syntax highlighting
works, history shared across terminals, wtf completion fires, per-dev
override loads, custom plugin install via $ZSH_CUSTOM persists across a
second rebuild, bash fallback prints banner without zsh errors). This
will be the moment of truth — if anything fails, iteration loops back
to session 2 or earlier.

# Session 1 — base-skeleton (dual-edit)

> **Effort** : ~0.5 day | **Dependencies** : none (first session)

## Prompt to paste

`````
I'm starting session 1 of the `zsh-omz-integration` rollout.

Entry point : `/workspace/plans/zsh-omz-integration/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory)
- `sessions/session-1-base-skeleton.md` (this spec)
- `/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md` (master plan with the install.sh audit that drove the dual-edit decision)

Goal of the rollout : add an Oh My Zsh team-wide base to the devcontainer
(theme + git plugin + history + autosuggestions + syntax-highlighting +
wtf completion), plus a per-dev override mechanism via
`.devcontainer/zshrc.local` (gitignored) and persistent custom plugins
via `.devcontainer/.zsh-custom/`.

## ⚠️ Dual-edit pattern

We do **NOT** re-run `install.sh` on `/workspace` during this rollout —
its wizard re-prompts PROJECT_ID with a default (`workspace`) that
diverges from the stored value (`devcontainer-tools`), which would
break devcontainer.json / docker-compose.yml regeneration. Instead, for
every file touched under `templates/v2/<X>`, **apply the same edit to
`/workspace/.devcontainer/<X>`** (= the dogfood test bed). Exception :
`install.sh` is at the repo root and gets a single edit.

Before editing each existing file, run `diff -q templates/v2/<X>
.devcontainer/<X>` to confirm they're already synchronized. If they
diverge already, surface it and stop — re-syncing pre-existing drift is
out of scope.

First session focus : **base-skeleton**. Concretely :

1. **Create `templates/v2/zshrc-base`** — team-wide zsh config. Contents :
   - `export ZSH=$HOME/.oh-my-zsh`
   - `export ZSH_CUSTOM=/workspace/.devcontainer/.zsh-custom`
   - `ZSH_THEME="${ZSH_THEME:-robbyrussell}"`
   - `plugins=(git zsh-autosuggestions zsh-syntax-highlighting)`
   - History opts : `HISTFILE`, `HISTSIZE=10000`, `SAVEHIST=10000`,
     `setopt SHARE_HISTORY HIST_IGNORE_DUPS HIST_IGNORE_SPACE
     HIST_REDUCE_BLANKS HIST_VERIFY`
   - `source "$ZSH/oh-my-zsh.sh"`
   - wtf autocomplete bootstrap (cache in `$HOME/.cache/zsh/wtf-completion.zsh`,
     generate-once-then-source pattern, guard with `command -v wtf`)
   - Dual : `cp templates/v2/zshrc-base .devcontainer/zshrc-base`

2. **Create `templates/v2/zshrc.local.example`** — committed onboarding
   doc. Show commented examples for : override theme (reload OMZ to
   apply), perso aliases (`ll`, `vi`), ports helpers (`lsport`,
   `killport`), per-dev plugin install pattern (`git clone ...
   $ZSH_CUSTOM/plugins/xxx` + `source ...plugin.zsh`), env vars.
   Add a short header explaining : copy to `.devcontainer/zshrc.local`,
   gitignored, sourced after `zshrc-base` by `shell-init.sh`.
   Dual : `cp templates/v2/zshrc.local.example .devcontainer/zshrc.local.example`

3. **Modify `templates/v2/shell-init.sh` AND `.devcontainer/shell-init.sh`** :
   - At top (before line 1) — add a zsh-gated block that : creates
     `/workspace/.devcontainer/.zsh-custom/{plugins,themes}/` if
     missing (first-boot skeleton), then sources `zshrc-base`.
   - At bottom (append) — add a zsh-gated block that sources
     `zshrc.local` if present.
   - **Before editing**, read the full file lines 50-221 to verify the
     banner section doesn't use idioms broken by OMZ's prompt. Note
     any concerns in EXISTING.md / LOG.md but DO NOT touch the banner
     code in this session.
   - Apply identical edits to both copies. Verify with
     `diff -q templates/v2/shell-init.sh .devcontainer/shell-init.sh`
     post-edit → must show no diff.

4. **Verify the gitignore template** : read both
   `templates/v2/.gitignore-root` and `templates/v2/.gitignore` to
   determine which one applies to the workspace root. Add to the
   correct one :
   ```
   # Per-dev shell config (managed by .devcontainer/zshrc.local pattern)
   .devcontainer/zshrc.local
   .devcontainer/.zsh-custom/
   ```
   Apply the same edit to `.devcontainer/<same-file>`. If the
   workspace root `/workspace/.gitignore` is what actually gates git's
   ignore behavior here, also propagate the entries there (verify via
   `git check-ignore -v .devcontainer/zshrc.local` after the edit).

5. **Single edit to `install.sh`** (no .devcontainer/ copy — root file).
   Add two `copy_verbatim` calls (around L260-265 in `install_baseline`,
   regrouped with existing shell-related copies) :
   ```bash
   copy_verbatim zshrc-base
   copy_verbatim zshrc.local.example
   ```
   This propagates the new files when **other** projects adopt the new
   template. It does NOT trigger anything on /workspace itself.

6. **Do NOT** modify `Dockerfile.base` in this session — that's
   session 2's scope. The new template files won't have any runtime
   effect yet (OMZ isn't installed in the image yet), which is
   expected — session 1 delivers the foundation.

Surface-level scope guard : edits restricted to these files :
- `templates/v2/zshrc-base` + `.devcontainer/zshrc-base` (new, dual)
- `templates/v2/zshrc.local.example` + `.devcontainer/zshrc.local.example` (new, dual)
- `templates/v2/shell-init.sh` + `.devcontainer/shell-init.sh` (edits at head + tail, dual)
- `templates/v2/.gitignore-root` + `.devcontainer/.gitignore-root` (one block, dual) — and possibly `/workspace/.gitignore` if needed
- `install.sh` (single)

If you find drift to other files (e.g. urge to update Dockerfile now),
**stop and surface it** — propose adding to STATUS.md for a later
session instead of folding in.

DoD at the end of this session :
1. **Sync check** : `diff -rq templates/v2/ .devcontainer/` shows only
   the known preexisting divergences (firewall/, knowledge/firewall.md,
   misc — listed in EXISTING.md). Files touched by this session must
   be identical between the two locations.
2. STATUS.md : flip session 1 row 📋 → ✅, prompt link → —, bump
   Delivered counter (0→1), refresh "Next focus" to session 2
   (dockerfile-omz).
3. LOG.md : append `## 1 — base-skeleton` section dated today with
   files touched + What / Why / Decisions / Gotchas / Tests / Commit.
4. EXISTING.md : update the "Files in templates/v2/ relevant to this
   rollout" table (zshrc-base / zshrc.local.example status now =
   "created"), and update the "Missing entirely" list (cross out
   what's now present in templates but not yet wired through Dockerfile).
5. Propose a commit (do NOT commit without explicit user confirmation).
   Suggested message style :
   `feat(shell): add zshrc-base + per-dev override skeleton (no OMZ install yet)`
`````

## Next session

After session 1 lands, create `sessions/session-2-dockerfile-omz.md`
with this scope :

- Add a `USER node` RUN block to `templates/v2/Dockerfile.base` (just
  before the current `# Shell init` block at L268) that runs the OMZ
  unattended installer + clones `zsh-autosuggestions` and
  `zsh-syntax-highlighting` into `$HOME/.oh-my-zsh/custom/plugins/`,
  with `--depth=1` to keep image size down.
- Dual-edit : apply same Dockerfile.base change to
  `.devcontainer/Dockerfile.base`.
- DO NOT rebuild yet — session 3 owns end-to-end verification.

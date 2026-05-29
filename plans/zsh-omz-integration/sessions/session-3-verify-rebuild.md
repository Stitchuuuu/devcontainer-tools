I'm starting session 3 of the `zsh-omz-integration` rollout.

Entry point : `/workspace/plans/zsh-omz-integration/ROLLOUT.md`
Read also :
- `STATUS.md` — sessions 1 ✅, 2 ✅, 3 📋 (this one — final)
- `LOG.md` — sessions 1 + 2 entries with full context
- `EXISTING.md` — current code inventory
- `sessions/session-3-verify-rebuild.md` (this spec)
- `/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md` (master plan)

## What sessions 1 + 2 delivered

- **Session 1** : `zshrc-base` + `zshrc.local.example` runtime files,
  `shell-init.sh` head (source zshrc-base) + tail (source zshrc.local)
  zsh-gated blocks, `.gitignore` entries for `zshrc.local` + `.zsh-custom/`,
  `install.sh` updated to copy the two new files into adopting projects.
- **Session 2** : `Dockerfile.base` RUN layer that installs OMZ unattended,
  removes the default `.zshrc`, then shallow-clones `zsh-autosuggestions`
  + `zsh-syntax-highlighting` into `$HOME/.oh-my-zsh/custom/plugins/`.

→ If the container has been rebuilt since commit `0363603`, OMZ is
live. If not, it's baked into the image but inert in the running
container.

## ⚠️ Pre-flight : was the container rebuilt ?

Run :
```
ls -d $HOME/.oh-my-zsh 2>/dev/null && echo "OMZ present — rebuild done"
```

- **OMZ present** → proceed with the checklist below.
- **Not present** → STOP. Ask the user to rebuild (VSCode :
  "Dev Containers: Rebuild Container", or from host :
  `docker compose -f .devcontainer/docker-compose.yml build --no-cache`
  then reopen). The rebuild terminates this Claude session — the
  next session paste-ins this same spec, and the pre-flight passes.

⚠️ **No `install.sh` re-run on `/workspace`** — same PROJECT_ID wizard
hazard as sessions 1 + 2. Full audit in
`/home/node/.claude/plans/zshrc-fais-moi-une-async-barto.md`.

## Verification checklist

Run each item. Report ✅ or ❌ with actual command output.

1. **Framework + plugins on disk** :
   ```
   test -d $HOME/.oh-my-zsh \
   && test -d $HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions \
   && test -d $HOME/.oh-my-zsh/custom/plugins/zsh-syntax-highlighting \
   && echo OK
   ```

2. **`.zshrc` is clean** : single source line, no OMZ default leftover.
   ```
   wc -l $HOME/.zshrc && cat $HOME/.zshrc
   ```
   Expected : 1 line, exactly :
   `[ -f /workspace/.devcontainer/shell-init.sh ] && source /workspace/.devcontainer/shell-init.sh`

3. **`shell-init.sh` head block sources `zshrc-base`** in interactive zsh :
   ```
   zsh -ic 'echo ZSH=$ZSH ; type omz 2>&1 | head -1'
   ```
   Expected : `ZSH=/home/node/.oh-my-zsh`, `omz is a shell function`.

4. **History config active** :
   ```
   zsh -ic 'echo HISTSIZE=$HISTSIZE SAVEHIST=$SAVEHIST HISTFILE=$HISTFILE'
   ```
   Expected : values match `zshrc-base` (non-zero HISTSIZE/SAVEHIST,
   HISTFILE=$HOME/.zsh_history).

5. **`compinit` autoloaded** :
   ```
   zsh -ic 'whence -w compinit'
   ```
   Expected : `compinit: function`.

6. **`wtf` completion sourced** : the cached completion file
   defines `_wtf_completion_loader` (bash-style ; zshrc-base runs
   `bashcompinit` so zsh honours the `complete -F` directive) :
   ```
   zsh -ic 'whence -w _wtf_completion_loader ; complete -p wtf'
   ```
   Expected : `_wtf_completion_loader: function` and
   `complete -F _wtf_completion_loader -o default wtf`.
   Tab-completion on `wtf <TAB>` works in an interactive terminal.

7. **zsh-autosuggestions active** (interactive — needs real TTY) :
   open a new terminal, type a partial command you've run before,
   confirm gray suggestion appears. Describe or screenshot.

8. **zsh-syntax-highlighting active** (interactive) : type
   `nonexistent_cmd_xyz`, confirm it appears in red ; type `ls`,
   confirm it appears in green/highlighted.

9. **Per-dev override** :
   ```
   cp .devcontainer/zshrc.local.example .devcontainer/zshrc.local
   # edit zshrc.local : set ZSH_THEME="eastwood" or add a custom alias
   ```
   Open a new zsh, confirm the override is applied (theme changed,
   alias works). Verify `zshrc.local` is gitignored (`git check-ignore -v`).

10. **Workspace-mounted plugin install** — *DEFERRED.*

    Session 3 dropped the `ZSH_CUSTOM` redirect from `zshrc-base`
    (and the matching skeleton-init block from `shell-init.sh`).
    OMZ now uses its default `$ZSH/custom/` — there is no longer a
    workspace-mounted plugin path wired to OMZ. Per-dev plugin
    support was deferred per user signal (« on s'en fou des autres
    plugins » mid-session, see LOG.md session 3).

    Workaround for any dev who wants ad-hoc plugins right now :
    clone the plugin anywhere (e.g. `$HOME/my-plugins/`) and
    `source` its `.plugin.zsh` from `.devcontainer/zshrc.local`.
    Survives in-container until the next rebuild ; that's it.

    A future session can add a real per-dev mechanism if needed
    (committed `.zsh-custom/plugins/` for team-shared + an
    `ZSH_CUSTOM` redirect + bridge if baked plugins coexist, OR
    move the plugin install OUT of `Dockerfile.base` and into a
    vendored `.zsh-custom/`).

11. **bash fallback** :
    ```
    bash -i -c 'echo "in bash : $0 ; ZSH_VERSION=${ZSH_VERSION:-unset}"'
    ```
    Expected : bash runs, banner appears, no error on the OMZ source
    line (zsh-gated blocks in shell-init.sh skip cleanly).

## Failure mode

If any item fails :
- **STOP** — don't silently patch.
- Surface the failure with command output.
- We decide together : (a) fix + re-run (rebuild possibly needed),
  or (b) defer to a session 4 if non-trivial.

If a Dockerfile.base or shell-init.sh edit is required to fix a
failure, the **dual-edit pattern still applies** (template +
`.devcontainer/`).

## DoD at the end of this session

1. All 11 checklist items reported ✅ / ❌ with evidence.
2. **STATUS.md** : flip row 3 `📋` → `✅` (all green) or `⚠️`
   (blocked). Delivered `2 / 3` → `3 / 3`. Next focus →
   "rollout complete" or describe the blocker.
3. **LOG.md** : append `## 3 — verify-rebuild` section dated today
   with checklist results, surprises, per-dev override findings,
   bash fallback behavior. **Commit** field follows the proposal
   below.
4. **EXISTING.md** : rename "Status after session 2" → "Status
   after session 3 (live)". Flip the "inert until rebuild" notes
   to "✅ live" (or ❌ if any item failed).
5. Clean up the verification artefacts created in step 9-10 :
   `.devcontainer/zshrc.local` (per-dev, not for commit),
   `.devcontainer/.zsh-custom/plugins/zsh-z/` (per-dev). Both are
   already gitignored — leave them on disk OR remove them ; just
   confirm `git status` shows no spurious staged changes.
6. **Propose 1 commit** (plan-file bookkeeping only, no source
   changes if all green) :
   - Files : `plans/zsh-omz-integration/{STATUS,LOG,EXISTING}.md`
     + this `sessions/session-3-verify-rebuild.md`
   - Suggested subject : `docs(rollout): zsh OMZ integration verified live`
   - If a source fix was needed, split per the templating /
     container precedent.
7. Do NOT commit without explicit user confirmation.

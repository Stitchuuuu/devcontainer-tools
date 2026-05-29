# Existing — technical inventory

> Snapshot of the code state at the start of this plan. Updated when a
> session adds / removes / restructures major files.
> For chronological history, see [LOG.md](LOG.md).
> For decisions and philosophy, see [ROLLOUT.md](ROLLOUT.md).

## Current shell setup in the devcontainer

### Image-level (baked in `templates/v2/Dockerfile.base`)

- **`zsh` installed** via apt-get ([Dockerfile.base:25](../../templates/v2/Dockerfile.base#L25))
- **`fzf` installed** ([Dockerfile.base:24](../../templates/v2/Dockerfile.base#L24)) — usable for shell history fuzzy-search but not currently wired into zsh keybindings.
- **`git-delta` installed** — git diff pager with color/syntax-highlight (orthogonal to shell config).
- **HOME directories created** for `node` user ([Dockerfile.base:122-124](../../templates/v2/Dockerfile.base#L122-L124)) :
  - `$HOME/.zsh/`, `$HOME/.cache/zsh/`, `$HOME/.zsh_history` (touched)
- **Default shell** : `ENV SHELL=/bin/zsh` ([Dockerfile.base:279](../../templates/v2/Dockerfile.base#L279))
- **`wtf` task runner installed** ([Dockerfile.base:104-114](../../templates/v2/Dockerfile.base#L104-L114))
  — provides `wtf --autocomplete setup` (zsh completion script) and a `.wtfcmd.yaml`-driven UX.
- **`USER node`** is the active user at end of Dockerfile.

### Shell init wiring

[Dockerfile.base:271-274](../../templates/v2/Dockerfile.base#L271-L274) injects into both `~/.bashrc` and `~/.zshrc` :

```dockerfile
USER $USERNAME
RUN echo '[ -f /workspace/.devcontainer/shell-init.sh ] && source /workspace/.devcontainer/shell-init.sh' >> $HOME/.bashrc && \
    echo '[ -f /workspace/.devcontainer/shell-init.sh ] && source /workspace/.devcontainer/shell-init.sh' >> $HOME/.zshrc
```

→ Single source line per shell. Modifications to `shell-init.sh`
require no rebuild (workspace-mounted).

### `templates/v2/shell-init.sh` (current, 221 lines)

Shell-agnostic init sourced at each interactive shell. Responsibilities :

1. Show post-start log path ([shell-init.sh:1-6](../../templates/v2/shell-init.sh#L1-L6))
2. Claude credentials conflict resolution ([shell-init.sh:8-29](../../templates/v2/shell-init.sh#L8-L29))
3. Auto-sync Claude credentials ([shell-init.sh:31-34](../../templates/v2/shell-init.sh#L31-L34))
4. GitHub CLI auto-auth on first shell with TTY ([shell-init.sh:36-50](../../templates/v2/shell-init.sh#L36-L50))
5. (Lines 51-221 not re-read here — banner, mitm CA setup, claude-local fallback, scan-deps reminder. Read in session 1.)

**No zsh-specific logic anywhere.** Pure POSIX/bash-compatible. We'll add zsh-gated blocks at the top and bottom in session 1.

### Status after session 3 (live)

- ✅ Oh My Zsh framework — live at `$HOME/.oh-my-zsh` post-rebuild
- ✅ Theme `eastwood` (team default after session 3 flip) — live ; per-dev override via `zshrc.local` verified
- ✅ Plugins `zsh-autosuggestions` + `zsh-syntax-highlighting` — discoverable at OMZ's default `$ZSH/custom/plugins/`. `_zsh_autosuggest_start` + `_zsh_highlight` defined.
- ⚠️ History tuning — wired in `zshrc-base` but `HISTSIZE=10000` is overridden by OMZ's `lib/history.zsh` (sets 50000). Both values sane ; cosmetic, see LOG session 3.
- ✅ `compinit` autoload — live via OMZ
- ✅ `wtf` completion — live ; `bashcompinit` added in session 3 to honour wtf's bash-style `complete -F`. `complete -p wtf` shows the registration.
- ✅ Per-dev override mechanism — `zshrc.local` sourcing wired by `shell-init.sh`, gitignored, live
- ⏸️ Per-dev workspace-mounted plugin path — **deferred**. Session 3 dropped the `ZSH_CUSTOM` redirect that powered this (it hid the baked plugins). Workaround : clone anywhere, `source` from `zshrc.local`. Re-enabling cleanly requires a vendor-or-bridge design choice for the baked plugins ; future session.

## Files in `templates/v2/` relevant to this rollout

| Path | Role | Status |
|---|---|---|
| `Dockerfile.base` | Build the base image | ✅ Session 2 — RUN block installs OMZ + 2 plugins (inert until rebuild in session 3) |
| `Dockerfile` / `Dockerfile.php` | Stack-specific images on top of base | No change expected (OMZ lives in base) |
| `shell-init.sh` | Runtime shell init, sourced from `~/.zshrc`/`~/.bashrc` | ✅ Session 1 — head block (source zshrc-base) + tail block (source zshrc.local), zsh+interactive gated |
| `post-start.sh` | postStartCommand hook | No change |
| `.gitignore-root` | Workspace-root `.gitignore` template (root scope) | Not used — zshrc.local + .zsh-custom paths live inside `.devcontainer/` |
| `.gitignore` | Devcontainer-local `.gitignore` template (scope = `.devcontainer/`) | ✅ Session 1 — added `zshrc.local` + `.zsh-custom/` (paths relative to .devcontainer/) |
| `zshrc-base` | Team-wide zsh config sourced by `shell-init.sh` | ✅ Session 1 — created |
| `zshrc.local.example` | Per-dev override template, committed | ✅ Session 1 — created |

## `install.sh` — how templates land in `.devcontainer/`

- [install.sh:261](../../install.sh#L261) : loop that `chmod +x` lifecycle scripts. `zshrc-base` and `zshrc.local.example` are **sourced**, not executed → no entry needed here.
- [install.sh:420](../../install.sh#L420) : line that copies the runtime template files (`post-start.sh`, `shell-init.sh`, `install-extensions.sh`) from `templates/v2/` to `.devcontainer/`. Must be extended with `zshrc-base` and `zshrc.local.example`.
- (Surrounding lines 400-430 to be re-read in session 2 to confirm the exact copy mechanism — `cp` direct vs. `for` loop.)

## Live runtime layout (rollout end-state)

```
/home/node/.oh-my-zsh/                      # baked in image (volatile across rebuilds, instant boot)
├── plugins/git/                            # built-in OMZ
├── custom/plugins/
│   ├── zsh-autosuggestions/                # baked, discovered at OMZ default path
│   └── zsh-syntax-highlighting/            # baked, discovered at OMZ default path
└── …

/workspace/.devcontainer/
├── shell-init.sh                           # sourced from ~/.zshrc and ~/.bashrc
├── zshrc-base                              # sourced by shell-init.sh if $ZSH_VERSION (theme: eastwood, plugins: git + 2 baked, bashcompinit for wtf)
├── zshrc.local                             # per-dev, gitignored, sourced last (overrides anything above)
├── zshrc.local.example                     # committed onboarding doc
└── .zsh-custom/                            # gitignored, INERT — no code path references it post-session-3.
                                            # Kept on disk + in .gitignore for a future per-dev mechanism.
```

## Open questions (resolved)

1. **`.gitignore-root` vs `.gitignore`** — resolved session 1 : `.devcontainer/.gitignore` covers the two new paths (scope = `.devcontainer/`). `.gitignore-root` untouched.
2. **OMZ prompt vs banner ANSI escapes** — verified session 3 : `shell-init.sh` banner uses `echo`/`printf`/Python `print` ; OMZ only sets `PROMPT` (applies after shell-init returns). No collisions observed.
3. **OMZ unattended installer `RUNZSH=no CHSH=no`** — resolved session 2 : `--unattended` covers both. The `ENV SHELL=/bin/zsh` from the Dockerfile stays authoritative.

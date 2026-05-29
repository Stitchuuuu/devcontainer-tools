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

### Status after session 2

- ✅ Oh My Zsh framework — baked in image at `$HOME/.oh-my-zsh`, inert until session 3 rebuild
- ✅ Theme `robbyrussell` — baked via OMZ default, inert until rebuild
- ✅ Plugins `zsh-autosuggestions` + `zsh-syntax-highlighting` — shallow-cloned into `$HOME/.oh-my-zsh/custom/plugins/`, inert until rebuild
- ✅ History tuning — wired in `zshrc-base`, will activate once OMZ loads at next rebuild
- ✅ `compinit` autoload — sourced via OMZ in `zshrc-base`, inert until rebuild
- ✅ `wtf` completion source — bootstrap in `zshrc-base`, inert until OMZ available post-rebuild
- ✅ Per-dev override mechanism — `zshrc.local` sourcing wired by `shell-init.sh`, gitignored (unchanged from session 1)

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

## Target runtime layout (after rollout)

```
/home/node/.oh-my-zsh/                      # baked in image (volatile across rebuilds, instant boot)
├── plugins/git/                            # built-in OMZ
├── custom/plugins/
│   ├── zsh-autosuggestions/                # baked
│   └── zsh-syntax-highlighting/            # baked
└── …

/workspace/.devcontainer/
├── shell-init.sh                           # sourced from ~/.zshrc and ~/.bashrc
├── zshrc-base                              # NEW — sourced by shell-init.sh if $ZSH_VERSION
├── zshrc.local                             # NEW (per-dev, gitignored) — sourced last
├── zshrc.local.example                     # NEW (committed, onboarding doc)
└── .zsh-custom/                            # NEW (gitignored, ZSH_CUSTOM points here)
    ├── plugins/                            # per-dev clones (zsh-z, etc.)
    └── themes/                             # per-dev themes custom
```

## Open questions (resolve early)

1. `.gitignore-root` vs `.gitignore` in `templates/v2/` — which one applies to the workspace root vs. `.devcontainer/` itself ? Inspect both at session 1 start.
2. shell-init.sh lines 50-221 may contain banner code using raw ANSI escapes — does OMZ's prompt clobber any of them ? Check at session 3 verification.
3. Does the OMZ unattended installer support `RUNZSH=no CHSH=no` ? (we need it to NOT launch zsh and NOT change default shell — the Dockerfile already set ENV SHELL=/bin/zsh). The `--unattended` flag should cover this but confirm at session 2 implementation.

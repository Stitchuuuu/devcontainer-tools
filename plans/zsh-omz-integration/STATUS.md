# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | base-skeleton — create `zshrc-base` + `zshrc.local.example`, wire `shell-init.sh` sourcing, gitignore entries — **dual-edit** | ✅ | — |
| 2 | dockerfile-omz — add OMZ + plugin installs to `Dockerfile.base` — **dual-edit** (template + .devcontainer/) | ✅ | — |
| 3 | verify-rebuild — rebuild container then checklist (build, prompt, autosuggest, history, per-dev override, rebuild persistence). No `install.sh` re-run on self. | 📋 | _(create after session 2)_ |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 2 / 3
- **Next focus** : session 3 (verify-rebuild) — write `sessions/session-3-verify-rebuild.md`, rebuild the container, then run the checklist (build success, prompt, autosuggest, syntax-highlight, history tuning, per-dev override via `zshrc.local`, plugin persistence under `.zsh-custom/`, bash fallback).

# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | base-skeleton — create `zshrc-base` + `zshrc.local.example`, wire `shell-init.sh` sourcing, gitignore entries — **dual-edit** | ✅ | — |
| 2 | dockerfile-omz — add OMZ + plugin installs to `Dockerfile.base` — **dual-edit** (template + .devcontainer/) | ✅ | — |
| 3 | verify-rebuild — rebuild container then checklist (build, prompt, autosuggest, history, per-dev override, rebuild persistence). No `install.sh` re-run on self. | ✅ | [→ prompt](sessions/session-3-verify-rebuild.md) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 3 / 3
- **Next focus** : rollout complete. Per-dev workspace-mounted plugin support deferred (no user need yet) — see session 3 LOG entry for the future-session unblock recipe.

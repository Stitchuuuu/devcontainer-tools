# Status — Actionable sessions

> Click `→ prompt` to open the `sessions/session-NN.md` file to paste into
> a fresh Claude Code session.
> For the detailed history (reasons, files touched, gotchas), see
> [LOG.md](LOG.md). For current code state, see [EXISTING.md](EXISTING.md).

| Session | Brief | Status | Prompt |
|---|---|---|---|
| 1 | doc-cosmetic — patches #1, #5, #6, #12 (CLAUDE-project rename, prepare-plan wrapper, CLAUDE-dev registry, banner widths) | ✅ | — |
| 2 | shell-and-settings — patches #2, #3, #4, #7 (OMZ + zshrc override, port-forward ignore, settings.local seeding, remove gh auth block) | 📋 | [→ prompt](sessions/session-2-shell-and-settings.md) |
| 3 | dockerfile-cache-split — patch #13 (Dockerfile.base 6-RUN cache split for Claude install) ; skip superseded #8, #9 | 📋 | _to create after S2_ |
| 4 | notify-daemon — patches #10, #11 (notify daemon + vscode-ext-patchs refactor, ~74 files, PROJECT_NAME templatisation) | 📋 | _to create after S3_ |
| 5 | per-project-tag-and-installer-audit — patch #15 + exhaustive installer audit (firewall-docker-setup copy + grep `{{` filet) | 📋 | _to create after S4_ |
| 6 | version-bump-and-changelog — TEMPLATE_VERSION 2.0.0→2.1.0, README version line, CHANGELOG 2.1.0 entry | 📋 | _to create after S5_ |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⚠️ blocked · ❌ cancelled

## Progress

- **Delivered** : 1 / 6
- **Next focus** : session 2 (shell-and-settings)

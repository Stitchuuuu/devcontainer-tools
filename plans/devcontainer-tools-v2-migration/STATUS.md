# Status — Actionable sessions

> Click `→ prompt` to open the session file to paste into a fresh
> Claude Code session. For the detailed history, see [LOG.md](LOG.md).
> For the Part 1 file scope, see [SCOPE.md](SCOPE.md).

## Part 1 — `install.sh` v2 for new projects

| # | Brief | Status | Prompt |
|---|---|---|---|
| 1 | scope-audit — freeze the Part 1 file list (~84 files : core + lifecycle + firewall + policy.d/ baseline + 5 generic skills + 5 claude/ rules + knowledge/ + 4 docs + claude-bridge/ + host-helpers/), confirm the templating model (shell expansion + 3 placeholders), drop list, sync inheritance | ✅ | — |
| 2 | install-redesign — rewrite `install.sh` v2 (wizard 4 prompts, copy verbatim + sed only on devcontainer.json + .env.example, regenerate `.gitignore`, set exec perms) + sync `templates/` from `.devcontainer/` (added 20 missing files, dropped 7, renamed Dockerfile.custom→Dockerfile, scrubbed all ragnarok/cyro/portal42) | ✅ | — |
| 3 | ~~firewall-skills-wiring~~ — **folded into session 2** : the templates/ sync in session 2 already brought `firewall/` (addons, policy.d/, compile-policy.py, mitm-init.sh, firewall-blocks) and `skills/` (5 generic + sync-skills.sh) up to baseline. | ❌ | — (folded) |
| 3 | firewall-layer-split — move 4 project-specific COPYs (domains.txt, domains.local.txt.example, policy.d/, policy.local.d.example/) from `Dockerfile.base` to the project layer (`Dockerfile` + `Dockerfile.php`) so `claude-devcontainer-base:${VERSION}` stays project-agnostic and truly shared across projects. Discovered during fresh-install validation. Includes a migration section for already-deployed v2-beta instances (manual edit of 2 Dockerfiles, 1 base rebuild). | ✅ | — |
| 3b | gitignore-architecture-refactor — split gitignore between a shipped `.devcontainer/.gitignore` (scoped to `.devcontainer/`) and a slimmed `update_gitignore()` for root-scope entries only. Relocate `LESSONS.md` / `LESSONS.local.md` into `.devcontainer/` (with root symlink, same pattern as `CLAUDE.md`). Whitelist `.vscode/settings.json` + `.vscode/extensions.json` so the `post-start.sh` `skip-worktree` trick has tracked files to act on. Discovered during fresh-install validation. Independent of session 3 — can run in parallel. | 📋 | [→ prompt](sessions/part-1-session-3b-gitignore-architecture-refactor.md) |
| 3c | firewall-write-protection (security) — close the in-container tampering vector : Claude (node user) can currently modify any `.devcontainer/firewall/*` file via the writable workspace bind mount ; modifications are picked up at next user-initiated restart via `post-start.sh → sudo init-firewall.sh`, enabling policy or DNS hijacking. Two options on the table : (A) `:ro` overlay shadowing the workspace mount at the firewall subpath (1 line YAML, preserve live-edit UX) or (B) remove the bind mount entirely + bake everything in the image (immutable, edit-requires-rebuild). Recommendation : option B per the "lock-down devcontainer side, no root for Claude" requirement. Discovered during P1-S3 review. Independent of S3b/S4. | 📋 | [→ prompt](sessions/part-1-session-3c-firewall-write-protection.md) |
| 4 | fresh-install-test — run `install.sh` v2 against a sandbox project (full `Reopen in Container` cycle, not just file-copy smoke test) ; verify base image builds, sync-creds + sync-skills auto-trigger at post-start, firewall comes up clean, gitignore split + LESSONS symlink behave correctly, write-protection invariants hold (P1-S3c) | 📋 | [→ prompt](sessions/part-1-session-4-fresh-install-test.md) |
| 5 | bump-changelog — write `CHANGELOG.md` v2.0.0 entry (TEMPLATE_VERSION already bumped in session 2), document the breaking drops, refresh `README.md`, tag `v2.0.0` locally | 📋 | [→ prompt](sessions/part-1-session-5-bump-changelog.md) |

## Part 2 — Claude session prompt for 1.3 → 2.0 migration

| # | Brief | Status | Prompt |
|---|---|---|---|
| 1 | TBD — design a paste-into-Claude prompt that walks an existing 1.3 project through the upgrade, handling per-file reconciliation with human-in-the-loop validation | ⏸️ | — (specced after Part 1 ships) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⏸️ deferred · ⚠️ blocked · ❌ cancelled

## Progress

- **Part 1** : 3 / 7 delivered (original session 3 = `firewall-skills-wiring` folded into session 2 ; new session 3 = `firewall-layer-split` ✅, new session 3b = `gitignore-architecture-refactor` and new session 3c = `firewall-write-protection` all discovered during P1-S2/S3 validation ; total scope now 7 = 1, 2, 3, 3b, 3c, 4, 5)
- **Part 2** : 0 / 1 — deferred until Part 1 ships
- **Next focus** : Part 1 session 3c (`firewall-write-protection` — security fix for in-container tampering vector, blocks v2.0.0 release if shipped insecurely) ; sessions 3b and 3c parallelisable

## Blockers

- _None._ The session-H blocker (`wtf` + firewall domains + `knowledge/wtf.md`) was verified delivered before Part 1 session 2.

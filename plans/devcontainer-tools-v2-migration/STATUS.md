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
| 3 | firewall-layer-split — move 4 project-specific COPYs (domains.txt, domains.local.txt.example, policy.d/, policy.local.d.example/) from `Dockerfile.base` to the project layer (`Dockerfile` + `Dockerfile.php`) so `claude-devcontainer-base:${VERSION}` stays project-agnostic and truly shared across projects. Discovered during fresh-install validation. | 📋 | [→ prompt](sessions/part-1-session-3-firewall-layer-split.md) |
| 4 | fresh-install-test — run `install.sh` v2 against a sandbox project (full `Reopen in Container` cycle, not just file-copy smoke test) ; verify base image builds, sync-creds + sync-skills auto-trigger at post-start, firewall comes up clean | 📋 | [→ prompt](sessions/part-1-session-4-fresh-install-test.md) |
| 5 | bump-changelog — write `CHANGELOG.md` v2.0.0 entry (TEMPLATE_VERSION already bumped in session 2), document the breaking drops, refresh `README.md` | 📋 | — (created at end of session 4) |

## Part 2 — Claude session prompt for 1.3 → 2.0 migration

| # | Brief | Status | Prompt |
|---|---|---|---|
| 1 | TBD — design a paste-into-Claude prompt that walks an existing 1.3 project through the upgrade, handling per-file reconciliation with human-in-the-loop validation | ⏸️ | — (specced after Part 1 ships) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⏸️ deferred · ⚠️ blocked · ❌ cancelled

## Progress

- **Part 1** : 2 / 5 delivered (original session 3 = `firewall-skills-wiring` folded into session 2 ; new session 3 = `firewall-layer-split` discovered during fresh-install validation ; total scope back up to 5)
- **Part 2** : 0 / 1 — deferred until Part 1 ships
- **Next focus** : Part 1 session 3 (`firewall-layer-split` — base image must stay project-agnostic)

## Blockers

- _None._ The session-H blocker (`wtf` + firewall domains + `knowledge/wtf.md`) was verified delivered before Part 1 session 2.

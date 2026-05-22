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
| 3b | gitignore-architecture-refactor — split gitignore between a shipped `.devcontainer/.gitignore` (scoped to `.devcontainer/`) and a slimmed `update_gitignore()` for root-scope entries only. Relocate `LESSONS.md` / `LESSONS.local.md` into `.devcontainer/` (with root symlink, same pattern as `CLAUDE.md`). Whitelist `.vscode/settings.json` + `.vscode/extensions.json` so the `post-start.sh` `skip-worktree` trick has tracked files to act on. Discovered during fresh-install validation. Independent of session 3 — can run in parallel. | ✅ | — |
| 3c | firewall-write-protection (security) — close the in-container tampering vector : option B (drop bind mount + bake firewall into image) chosen and **delivered via the parallel `devcontainer-security-hardening` (v1 + v2) rollouts**. See [hardening-v1 STATUS](../devcontainer-security-hardening/STATUS.md) § S1 (bake-firewall + drop bind mount), § S2 (drop env injection), § S6 (adversarial gate) ; [hardening-v2 STATUS](../devcontainer-security-hardening-v2/STATUS.md) § S3 (dnsmasq strict), § S4 (PoC #9 replay gate). Templates/v2 ↔ dogfood `.devcontainer/` parity confirmed (commits `231d3ec`, `cb0301f`, `2cd3cd6` touch both paths). New adopters inherit via install.sh. | ✅ | — |
| 4 | fresh-install-test — runtime + file-level validation done **incrementally** across the iteration cycle, not as a single dedicated session : S3 build gates (5/5), S3b smoke install gates (10) + check-ignore (6) + symlink mode 120000, hardening v1 S6 adversarial replay (0 SUCCESS, 3 threat-model criteria held), hardening v2 S4 PoC #9 replay on HEAD `2cd3cd6` (REFUSED, `test-dns-strict.sh` 6/0/1). No outstanding template work ; v2.0.0 release-ready. | ✅ | — |
| 5 | bump-changelog — `CHANGELOG.md` v2.0.0 entry written (breaking drops + renames + new incl. hardening) ; root `README.md` created for the v2 install flow (previously absent) ; `TEMPLATE_VERSION` confirmed at `"2.0.0"` ; tag `v2.0.0` posed locally (push deferred to user) ; `ROADMAP.md` synced to released state | ✅ | — |

## Part 2 — Claude session prompt for 1.3 → 2.0 migration

| # | Brief | Status | Prompt |
|---|---|---|---|
| 1 | TBD — design a paste-into-Claude prompt that walks an existing 1.3 project through the upgrade, handling per-file reconciliation with human-in-the-loop validation | ⏸️ | — (specced after Part 1 ships) |

## Legend

✅ delivered · 🚧 in progress · 📋 planned · ⏸️ deferred · ⚠️ blocked · ❌ cancelled

## Progress

- **Part 1** : 7 / 7 delivered ✅ — **v2.0.0 ready to release** (tag posed locally, push deferred to user). S1 scope-audit, S2 install-redesign, S3 firewall-layer-split, S3b gitignore-architecture-refactor, S3c firewall-write-protection [via hardening rollouts], S4 fresh-install-test [validated incrementally], S5 bump-changelog. Original session 3 = `firewall-skills-wiring` folded into session 2 ; total scope = 7 sessions (1, 2, 3, 3b, 3c, 4, 5).
- **Part 2** : 0 / 1 — deferred ; will be specced after a few new projects install v2.0.0 in the wild (real upgrade requests surface the edge cases worth handling).
- **Next focus** : `git push origin v2.0.0` at user's discretion ; then begin Part 2 spec when v2.0.0 has settled.

## Blockers

- _None._ The session-H blocker (`wtf` + firewall domains + `knowledge/wtf.md`) was verified delivered before Part 1 session 2.

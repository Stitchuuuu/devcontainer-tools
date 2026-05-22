# Part 1 — session 4 — fresh-install-test

> **Effort** : ~2-3 h | **Dependencies** : Part 1 sessions 2
> (install-redesign), 3 (firewall-layer-split), 3b
> (gitignore-architecture-refactor) delivered. Requires a host with
> Docker + VS Code with the Dev Containers extension to run the full
> `Reopen in Container` cycle (session 2 only validated file-copy +
> assertions via smoke test ; this session validates the runtime).
>
> ⚠ Cette session était numérotée "3" à l'origine. Renumérotée en 4
> quand session 3 = `firewall-layer-split` et session 3b =
> `gitignore-architecture-refactor` ont été insérées suite à la
> validation fresh-install initiale.

## Why this session

Session 2's smoke test validated file copy, exec perms, marker writes,
.env / .gitignore mutations, placeholder substitution, and the
detect-existing paths. It did **not** validate :
- The base image actually builds (`Dockerfile.base` → `claude-devcontainer-base:${VERSION}`)
- The project layer builds (`Dockerfile`, `FROM claude-devcontainer-base`)
- The container actually starts (`docker compose up`)
- `on-create.sh` / `post-create.sh` / `post-start.sh` run cleanly
- `init-firewall.sh` brings dnsmasq + iptables + mitmproxy up
- `sync-creds.sh` triggers at `post-start.sh`, `shell-init.sh`, and
  Claude Code's `Stop`/`SessionEnd` hooks
- `sync-skills.sh` merges `*/hooks.json` into `~/.claude/settings.json`
  and copies `*.skill.md` to `~/.claude/commands/`
- The `claude` CLI works (`claude --version` returns ; OAuth volume
  is mountable)
- Optional : `claude-bridge` sidecar boots (only if a local Ollama is
  configured)

These are runtime concerns that need a real Docker daemon and a real
VS Code instance to exercise.

## Prompt to paste

`````
Je démarre la Part 1 session 4 (fresh-install-test) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `/workspace/plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `STATUS.md` (Part 1 progress, blockers)
- `LOG.md` § P1-S2, P1-S3, P1-S3b (install.sh v2 + templates/ delivery
  + firewall layer split + gitignore architecture)
- `SCOPE.md` (file inventory + scrub rule)
- `sessions/part-1-session-4-fresh-install-test.md` (this spec)

Goal : run `bash /workspace/devcontainer-tools/install.sh` against
a brand-new sandbox project, then validate the full container build +
Reopen-in-Container cycle end-to-end. Doit aussi valider les changements
de S3 (firewall layer split, base image project-agnostic) et de S3b
(gitignore split, LESSONS symlink, `.vscode/settings.json` whitelisté).

Session 4 scope :

1. **Create a sandbox project** outside `/workspace/.devcontainer/`,
   e.g. `~/sandbox/dctest-v2-$(date +%s)`. The sandbox must NOT
   collide with this dev container's own `.devcontainer/`.

2. **Run install.sh against the sandbox** with the default wizard
   answers (PROJECT_ID = slugified basename, DISPLAY_NAME =
   titlecase, PROJECT_TYPE = node, creds volume = the same shared
   volume this devcontainer uses, so OAuth carries over).

3. **Validate file artefacts** — re-run the LOG.md § P1-S2 assertion
   block against the sandbox `.devcontainer/`.

4. **Open in VS Code → Reopen in Container** :
   - `initialize.sh` runs on the host (builds base image if needed)
   - On-create / post-create / post-start chain executes
   - `init-firewall.sh` brings the firewall up
   - `sync-skills.sh` runs, merges hooks, copies skill cmds
   - `sync-creds.sh` runs, OAuth volume mounted
   - VS Code finishes "Reopen in Container" with no error

5. **Smoke-test the inner shell** :
   - `claude --version` returns the expected version
   - `which claude` resolves
   - `wtf foo` returns the help (or a "not found" graceful message)
   - `cat /etc/claude-source` echoes the failsafe variant marker
   - `cat ~/.claude/settings.json | jq '.hooks'` shows merged hooks
   - `ls ~/.claude/commands/` shows the skill commands

6. **Verify firewall** (Reopen succeeded means dnsmasq + iptables
   are up — confirm with `bash test-firewall.sh`).

7. **Test the v1.3 abort path** with a separate sandbox dir that has
   a pre-planted `.configured-setup` with `VERSION="1.3.0"` — install.sh
   should abort cleanly with the Part 2 pointer.

8. **Validate S3b artefacts** (gitignore + LESSONS) :
   - `test -f <sandbox>/.devcontainer/.gitignore` (shipped)
   - `test -L <sandbox>/LESSONS.md` + `readlink` retourne
     `.devcontainer/LESSONS.md`
   - `git -C <sandbox> init && git -C <sandbox> add -A && git -C <sandbox> ls-files -s LESSONS.md`
     → mode `120000`
   - Root `.gitignore` ne contient PAS d'entrées `.devcontainer/...`
     (déléguées au `.devcontainer/.gitignore`)
   - Root `.gitignore` contient `.vscode/*` + `!.vscode/settings.json`
   - `git -C <sandbox> check-ignore -v .devcontainer/logs/foo.log`
     matche `.devcontainer/.gitignore`
   - `git -C <sandbox> check-ignore .vscode/settings.json` non-matché
     (whitelisted)

9. **Validate S3 artefacts** (firewall layer split) :
   - `docker run --rm claude-devcontainer-base:${VERSION} ls /etc/devcontainer-firewall/`
     ne liste PAS domains.txt / policy.d / policy.local.d.example
   - `docker exec <sandbox-ctr> cat /etc/devcontainer-firewall/domains.txt`
     matche `<sandbox>/.devcontainer/firewall/domains.txt`

Validation (manual, end of session) :
- All steps 1-7 green
- Container boots in < 2 min (warm cache) or < 8 min (cold)
- No silent skips, no permission denials in lifecycle logs
- `docker compose down` cleans up without orphan volumes (besides the
  shared creds volume which is intentional)

DoD at end of this session :
1. STATUS.md : flip Part 1 session 4 row 📋 → ✅, prompt link → —,
   bump "Delivered" counter (5/6 attendu si S3 + S3b déjà delivered),
   set "Next focus" → Part 1 session 5 (bump-changelog).
2. LOG.md : append `## P1-S4 — fresh-install-test` section
   (~80-120 lines) with : sandbox path, build timings, lifecycle log
   highlights, any gotchas discovered, what (if anything) needs a
   fix-it commit before session 5.
3. SCOPE.md : amend only if a runtime issue surfaces a missing file.
4. La spec session 5 (`sessions/part-1-session-5-bump-changelog.md`)
   existe déjà — pas besoin de la créer. Vérifier qu'elle est toujours
   alignée avec ce qui a été delivered.
5. Propose a commit (do NOT commit without explicit user
   confirmation). If session 4 only validates and finds nothing to
   change, the commit may be skipped entirely (just LOG.md +
   STATUS.md updates, batched into the next session's commit).
`````

## Next session

`part-1-session-5-bump-changelog.md` — write CHANGELOG.md v2.0.0
entry (TEMPLATE_VERSION already bumped in session 2), document the
breaking drops (`gh-secure/`, `Dockerfile.node`, `master-review/`,
`KNOWLEDGE.md`, `test-db.php`, `gitignore-entries.txt`, `update.sh`),
the gitignore split + LESSONS relocate (S3b), the firewall layer split
(S3), refresh `README.md` for v2 install flow, tag `v2.0.0` locally.

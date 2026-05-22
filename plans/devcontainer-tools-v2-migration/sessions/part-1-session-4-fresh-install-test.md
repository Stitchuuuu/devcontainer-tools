# Part 1 — session 3 — fresh-install-test

> **Effort** : ~2-3 h | **Dependencies** : Part 1 session 2
> (install-redesign) delivered. Requires a host with Docker + VS Code
> with the Dev Containers extension to run the full `Reopen in
> Container` cycle (session 2 only validated file-copy + assertions
> via smoke test ; this session validates the runtime).

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
Je démarre la Part 1 session 3 (fresh-install-test) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `/workspace/plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `STATUS.md` (Part 1 progress, blockers)
- `LOG.md` § P1-S2 (install.sh v2 + templates/ delivery context)
- `SCOPE.md` (file inventory + scrub rule)
- `sessions/part-1-session-3-fresh-install-test.md` (this spec)

Goal : run `bash /workspace/devcontainer-tools/install.sh` against
a brand-new sandbox project, then validate the full container build +
Reopen-in-Container cycle end-to-end.

Session 3 scope :

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

Validation (manual, end of session) :
- All steps 1-7 green
- Container boots in < 2 min (warm cache) or < 8 min (cold)
- No silent skips, no permission denials in lifecycle logs
- `docker compose down` cleans up without orphan volumes (besides the
  shared creds volume which is intentional)

DoD at end of this session :
1. STATUS.md : flip Part 1 session 3 row 📋 → ✅, prompt link → —,
   bump "Delivered" (2/4 → 3/4), set "Next focus" → Part 1 session 4
   (bump-changelog).
2. LOG.md : append `## P1-S3 — fresh-install-test` section
   (~80-120 lines) with : sandbox path, build timings, lifecycle log
   highlights, any gotchas discovered, what (if anything) needs a
   fix-it commit before session 4.
3. SCOPE.md : amend only if a runtime issue surfaces a missing file.
4. Create `sessions/part-1-session-4-bump-changelog.md`.
5. Propose a commit (do NOT commit without explicit user
   confirmation). If session 3 only validates and finds nothing to
   change, the commit may be skipped entirely (just LOG.md +
   STATUS.md updates, batched into the next session's commit).
`````

## Next session

`part-1-session-4-bump-changelog.md` — write CHANGELOG.md v2.0.0
entry (TEMPLATE_VERSION already bumped in session 2), document the
breaking drops (`gh-secure/`, `Dockerfile.node`, `master-review/`,
`KNOWLEDGE.md`, `test-db.php`, `gitignore-entries.txt`, `update.sh`),
refresh `README.md` for v2 install flow. To be created at end of
session 3.

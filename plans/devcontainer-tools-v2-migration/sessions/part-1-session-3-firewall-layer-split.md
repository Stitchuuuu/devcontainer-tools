# Part 1 — session 3 — firewall-layer-split

> **Effort** : ~1-2 h | **Dependencies** : Part 1 session 2 (install-redesign) delivered.

## Why this session

Discovered during fresh-install validation : `Dockerfile.base` COPYs 4 project-specific firewall files (`domains.txt`, `domains.local.txt.example`, `policy.d/`, `policy.local.d.example/`) into the shared `claude-devcontainer-base:${CLAUDE_CODE_VERSION}` image. Multiple projects share the same tag → if project A rebuilds the base after editing its domains, the tag now points to A's content → project B silently inherits A's domains at its next start. **Breaks the "stable shared base across projects" invariant.**

The fix : move the 4 COPYs out of `Dockerfile.base` into the project layer (`Dockerfile` + `Dockerfile.php`). After the split, `claude-devcontainer-base:2.1.X` contains only project-agnostic tooling. Project layer rebuilds quickly per-project on domain/policy changes without invalidating the shared base.

## Where this session runs

The **canonical edit target** is `devcontainer-tools/templates/v2/Dockerfile{.base,Dockerfile,.php}` — the shipped baseline that downstream projects pull via `install.sh`. The session can run from either side of the sync pair :

- **Recommended** : `devcontainer-tools-v2/` (host repo) directly — clean separation, commits land where they ship from. `.devcontainer/` paths below refer to `devcontainer-tools-v2/.devcontainer/` (if you dogfood the repo with its own devcontainer).
- **Alternative** : from the originating project's in-container working copy (e.g. `/workspace/devcontainer-tools/`) — work the edits there, then `rsync devcontainer-tools/ ../devcontainer-tools-v2/` to propagate.

Adopting projects (cyro-live, portal42, etc.) inherit the fix the next time they re-run `install.sh` (or by manually applying the same edits to their own `.devcontainer/Dockerfile{.base,.php}`). That's NOT part of this session.

## Prompt to paste

`````
Je démarre la Part 1 session 3 (firewall-layer-split) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `plans/devcontainer-tools-v2-migration/ROLLOUT.md` (relative to
the repo root — whichever side of the sync pair you're working in).
Read also :
- `STATUS.md` (Part 1 progress, 2 / 5 delivered, next focus = this session)
- `LOG.md` § P1-S2 (session 2 context + commit `2bc0227` in originating project)
- `SCOPE.md` (file inventory, templating model, sync mechanism)
- `sessions/part-1-session-3-firewall-layer-split.md` (this spec)

Goal : `claude-devcontainer-base:${CLAUDE_CODE_VERSION}` must be a
truly stable image, shared by ALL projects on the same host — zero
project-specific data baked in. Move 4 firewall COPYs out of
`Dockerfile.base` into the project layer (`Dockerfile` +
`Dockerfile.php`) so they rebuild per-project (~5s each) without
invalidating the base.

## Path convention

This session works on **the devcontainer-tools repo** (host-side
`devcontainer-tools-v2/` OR the in-container working copy
`/workspace/devcontainer-tools/` — pick one). All paths in this
prompt are relative to the repo root.

Canonical edit targets :
- `templates/v2/Dockerfile.base`  ← remove 4 COPYs
- `templates/v2/Dockerfile`        ← add 4 COPYs + chown/chmod
- `templates/v2/Dockerfile.php`    ← add 4 COPYs + chown/chmod

If the repo has its own `.devcontainer/` for dogfooding (i.e.
`.devcontainer/Dockerfile.base`, `.devcontainer/Dockerfile`,
`.devcontainer/Dockerfile.php` all exist), mirror the same edits
there too. Otherwise skip the `.devcontainer/` step entirely.

Session 3 scope :

1. **Edit `templates/v2/Dockerfile.base`** :
   - Remove the 4 project-specific COPYs (currently lines 239-242) :
     - `COPY firewall/domains.txt /etc/devcontainer-firewall/domains.txt`
     - `COPY firewall/domains.local.txt.example /etc/devcontainer-firewall/...`
     - `COPY firewall/policy.d/ /etc/devcontainer-firewall/policy.d/`
     - `COPY firewall/policy.local.d.example/ /etc/devcontainer-firewall/policy.local.d.example/`
   - In the chmod 755 list (lines 249-253), remove the `policy.d`
     + `policy.local.d.example` directories.
   - Remove `touch /etc/devcontainer-firewall/domains.local.txt` (line
     245) — project layer recreates it.
   - Keep everything else (tools, addons/, tests/, dnsmasq.conf,
     compile-policy.py, init-firewall.sh, sudoers, ast.parse
     validation on tool code).

2. **Edit `templates/v2/Dockerfile`** (project layer). Currently bare
   `FROM` + comment. Append :

   ```dockerfile
   USER root

   # Project-specific firewall data — rebuilds per-project ; does NOT affect
   # the shared claude-devcontainer-base image.
   COPY firewall/domains.txt                /etc/devcontainer-firewall/domains.txt
   COPY firewall/domains.local.txt.example  /etc/devcontainer-firewall/domains.local.txt.example
   COPY firewall/policy.d/                  /etc/devcontainer-firewall/policy.d/
   COPY firewall/policy.local.d.example/    /etc/devcontainer-firewall/policy.local.d.example/

   RUN touch /etc/devcontainer-firewall/domains.local.txt && \
       chown -R root:root /etc/devcontainer-firewall/domains.txt \
                          /etc/devcontainer-firewall/domains.local.txt \
                          /etc/devcontainer-firewall/domains.local.txt.example \
                          /etc/devcontainer-firewall/policy.d \
                          /etc/devcontainer-firewall/policy.local.d.example && \
       chmod 644 /etc/devcontainer-firewall/domains.txt \
                 /etc/devcontainer-firewall/domains.local.txt \
                 /etc/devcontainer-firewall/domains.local.txt.example && \
       chmod -R 644 /etc/devcontainer-firewall/policy.d \
                    /etc/devcontainer-firewall/policy.local.d.example && \
       chmod 755 /etc/devcontainer-firewall/policy.d \
                 /etc/devcontainer-firewall/policy.local.d.example

   USER node
   ```

3. **Edit `templates/v2/Dockerfile.php`** (PHP variant). Same block as
   step 2, appended after the existing `USER node` line at the end of
   the PHP install.

4. **Mirror in dogfooding `.devcontainer/`** (only if it exists) :
   if the repo where you're working has `.devcontainer/Dockerfile.base`
   + `.devcontainer/Dockerfile` + `.devcontainer/Dockerfile.php`,
   apply the SAME edits to those 3 files. This keeps the dogfooding
   container in sync with the shipped baseline.
   If `.devcontainer/` is absent, skip — adopting projects pick up
   the fix on their next `install.sh` upgrade.

5. **Verify path consistency** : `init-firewall.sh` reads from
   `/etc/devcontainer-firewall/` (line 26, `FIREWALL_CONFIG_DIR`
   default). The project Dockerfile COPYs into the same path. No
   path change needed in `init-firewall.sh`.

6. **Verify `/prepare-research` compatibility** : the skill copies
   the project's `.devcontainer/firewall/` verbatim into the bundle.
   The bundle's project Dockerfile inherits the same COPY block →
   bundle works post-split. No skill changes needed. Sanity-grep
   `templates/v2/skills/prepare-research/templates/*.yaml` for any
   reference to firewall paths that's now stale.

Validation (manual, end of session) — grep-only, NO install.sh
re-run (that's session 4's job) :
```bash
# Run from the repo root (devcontainer-tools-v2/ OR
# /workspace/devcontainer-tools/, whichever side you're working on).

# 1. Dockerfile.base no longer COPYs the 4 project files
! grep -E '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
  templates/v2/Dockerfile.base

# 2. Dockerfile DOES COPY them (count = 4)
[ "$(grep -cE '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
  templates/v2/Dockerfile)" -eq 4 ] && echo "templates/v2/Dockerfile OK"

# 3. Same for Dockerfile.php
[ "$(grep -cE '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
  templates/v2/Dockerfile.php)" -eq 4 ] && echo "templates/v2/Dockerfile.php OK"

# 4. If dogfooding .devcontainer/ exists, same gates apply there
if [ -d .devcontainer ]; then
    ! grep -E '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
      .devcontainer/Dockerfile.base
    [ "$(grep -cE '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
      .devcontainer/Dockerfile)" -eq 4 ] && echo ".devcontainer/Dockerfile OK"
    [ "$(grep -cE '^COPY firewall/(domains|policy\.d|policy\.local\.d\.example)' \
      .devcontainer/Dockerfile.php)" -eq 4 ] && echo ".devcontainer/Dockerfile.php OK"
fi
```

Multi-project sharing test (optional, host-side, recommended for v2.0.0
release confidence) :
1. Rebuild current devcontainer → note `docker image inspect
   claude-devcontainer-base:2.1.145 --format '{{.Id}}'`
2. Build a sandbox with a different `domains.txt` (e.g. add example.com)
3. Re-inspect base ID → MUST be unchanged (sharing OK ✅)
4. In current container : `cat /etc/devcontainer-firewall/domains.txt`
   → MUST NOT show sandbox additions (isolation OK ✅)

DoD at end of this session :
1. STATUS.md : flip Part 1 session 3 row 📋 → ✅, prompt link → —,
   bump "Delivered" counter (2/5 → 3/5), set "Next focus" → Part 1
   session 4 (fresh-install-test).
2. LOG.md : append `## P1-S3 — firewall-layer-split` section dated
   today with : files touched (3 templates/v2/Dockerfiles +
   optionally 3 .devcontainer/Dockerfiles if dogfooding),
   architectural rationale, grep verification output, any gotchas.
3. KNOWLEDGE.md : update "Architecture" + "Copy logic" sections.
   Add new "Image layer split" section explaining base (tools-only)
   vs project layer (firewall data).
4. SCOPE.md : small amendment — the 4 firewall data files are now
   described as "shipped via the project Dockerfile layer, not baked
   in the base image".
5. ROADMAP.md : mark session 3 ✅, bump delivered counter, set next
   focus = session 4 (fresh-install-test).
6. If working in the originating project (the in-container working
   copy) : rsync to the host repo before proposing the commit there :
   ```bash
   # From the originating project root (host-workspace) :
   rsync -av --delete \
     --exclude='.git' --exclude='.github' --exclude='*.bak' \
     devcontainer-tools/ ../devcontainer-tools-v2/
   ```
   Then commit happens in `../devcontainer-tools-v2/` (host).
7. Propose ONE commit in `devcontainer-tools-v2/` (DO NOT commit
   without explicit user confirmation) :
   ```
   Move project-specific firewall data out of base image

   - Dockerfile.base no longer COPYs firewall/{domains.txt,
     domains.local.txt.example,policy.d/,policy.local.d.example/} ;
     the shared claude-devcontainer-base:${VERSION} stays stable
     across projects sharing the same CLAUDE_CODE_VERSION
   - Dockerfile + Dockerfile.php now own the project-specific
     firewall data via their own COPY block, with idempotent
     chown/chmod ; project layer rebuilds in ~5s on data change
     without invalidating the base image
   ```
   If dogfooding `.devcontainer/` was also edited, mention it as a
   second bullet ("mirror in dogfooding devcontainer for layer
   inspection"). If working in the originating project too, mention
   the rsync in commit message.
`````

## Next session

`part-1-session-4-fresh-install-test.md` — full `Reopen in Container`
cycle on a sandbox project to validate the v2 baseline end-to-end
(base image build, lifecycle, firewall up, sync-creds, sync-skills).
Was originally session 3 ; renumbered when session-3 split was
inserted.

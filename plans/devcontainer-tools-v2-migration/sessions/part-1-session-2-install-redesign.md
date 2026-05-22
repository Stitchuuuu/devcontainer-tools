# Part 1 — session 2 — install-redesign

> **Effort** : ~3-4 h | **Dependencies** : Part 1 session 1
> (scope-audit) delivered ; session H of `devcontainer-v2/
> phase3-rollout` delivered (so `wtf` + 4 firewall domains +
> `knowledge/wtf.md` are present in the v2 baseline we copy from).

## Prompt to paste

`````
Je démarre la Part 1 session 2 (install-redesign) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `/workspace/plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `SCOPE.md` (single source of truth for the file list — DO NOT
  amend during this session unless a scope hole is discovered)
- `LOG.md` (read P1-S1 entry for the sync wiring references)
- `sessions/part-1-session-2-install-redesign.md` (this spec)

Goal : rewrite `/workspace/devcontainer-tools/install.sh` (currently
v1.3, 28 KB, 13 prompts, heavy sed substitution) as a v2 installer
that ships the ~84 files frozen in SCOPE.md, with a collapsed wizard
and minimal text substitution.

Session 2 scope :

1. **Wizard design (4 prompts)** :
   - `PROJECT_ID` (slug, used as `DC_PROJECT` in .env) — default :
     slugified basename of target dir
   - `PROJECT_DISPLAY_NAME` — default : title-cased PROJECT_ID
   - `PROJECT_TYPE` (node / php / custom) — default : node ; drives
     `Dockerfile = Dockerfile.base + Dockerfile.<type>` cat
   - **Shared Claude creds volume** : list existing `claude-creds-*`
     docker volumes ; if none found, propose the default name
     `claude-credentials-shared` ; user accepts, types another name,
     or types `n` for per-project isolation
   No timezone prompt — `Europe/Paris` baked into the v2 baseline
   (`.env.example` + `docker-compose.yml` default expansion).
   Other optional `.env` edits stay post-install :
   - `EXTRA_NETWORK` (external docker network)
   - firewall mode (defaults to `strict`, user flips via
     `firewall-mode.sh` post-install)

2. **Detect existing `.devcontainer/`** :
   - If found with `.configured-setup` (v1.3 marker), abort with a
     message pointing to Part 2 (Claude session prompt) — install.sh
     v2 does NOT auto-migrate
   - If found without `.configured-setup` (v2 marker), offer
     [1] Reinstall (overwrites) / [2] Abort

3. **Copy logic** :
   - `copy_verbatim` helper : `cp $SRC $DEST` for ~81 of 84 files
   - `copy_templated` helper : sed on 2 placeholders for the 3
     templated files (`devcontainer.json`, `.env.example`, and the
     `claude/CLAUDE-project.md` stub)
   - `copy_dir` helper : `cp -r` for `firewall/addons/`,
     `firewall/policy.local.d.example/`, each skill dir,
     `knowledge/`
   - All copied from `templates/` which mirrors the v2 baseline
     (sync from `.devcontainer/` happens via a separate
     dev-only `sync-templates.sh` — out of session 2 scope, that's
     a maintainer concern)

4. **Generate `.devcontainer/.env`** from `.env.example` :
   - `cp .env.example .env` (full documented template, all vars
     commented)
   - Set `DC_PROJECT=<PROJECT_ID>` at the top of the file
   - If user picked a shared creds volume : uncomment + set
     `CLAUDE_CREDS_VOLUME=<name>` ; otherwise leave commented
   - Everything else stays commented for the user to tweak
     post-install (TZ baked Europe/Paris, no need to set)

5. **Generate `.gitignore` entries** (embedded inline, no separate
   gitignore-entries.txt file in v2) :
   ```
   # DevContainer (v2)
   .devcontainer/.env
   .devcontainer/.configured-*
   .devcontainer/logs/
   .devcontainer/pending/
   .devcontainer/pr-drafts/
   .devcontainer/research-bundles/
   .devcontainer/scan-deps/
   .devcontainer/firewall/domains.local.txt
   .devcontainer/firewall/domains.d/
   .devcontainer/firewall/policy.d/
   .devcontainer/firewall/policy.local.d/
   .devcontainer/skills/**/*.local/
   .vscode/
   .claude/
   ```

6. **Exec perms** : `chmod +x` on every `.sh`, on
   `firewall/firewall-blocks`, on `firewall/compile-policy.py`.

7. **Summary printout** : target dir, project ID/display name,
   project type, "Reopen in Container" instructions, link to
   `templates/README.md` for advanced config.

8. **Bump `TEMPLATE_VERSION`** to "2.0.0" in install.sh (session 5
   does CHANGELOG.md + README — out of scope here, but the version
   constant lives in install.sh so it's natural to bump here).

9. **Update `templates/`** : copy/sync from `.devcontainer/` the
   files listed in SCOPE.md (in-scope only — DO drop gh-secure/,
   Dockerfile.node, master-review/, KNOWLEDGE.md, test-db.php,
   gitignore-entries.txt from templates/).

Validation (manual, end of session) :
- `bash install.sh /tmp/test-new-project` runs without error
- Resulting `.devcontainer/` has all SCOPE.md in-scope files
- `.env` contains `DC_PROJECT=test-new-project` and `TZ=...`
- `devcontainer.json` has the display name interpolated
- `.gitignore` has the v2 entries
- All `.sh` and `.py` files are executable

DoD at end of this session :
1. STATUS.md : flip Part 1 session 2 row 📋 → ✅, prompt link → —,
   bump "Delivered" counter (0→1 for Part 1), set "Next focus" →
   Part 1 session 3 (firewall-skills-wiring).
2. LOG.md : append `## P1-S2 — install-redesign` section (~100-150
   lines) with : install.sh diff summary, templates/ delta, wizard
   prompt list, validation evidence.
3. SCOPE.md : amend only if a scope hole surfaced during the
   rewrite (document in commit message if so).
4. Create `sessions/part-1-session-3-firewall-skills-wiring.md`.
5. Propose a commit (do NOT commit without explicit user
   confirmation). Suggested message :
   ```
   Rewrite install.sh as v2 (4-prompt wizard + minimal sed)

   - Collapse wizard from 13 prompts to 4 (PROJECT_ID, DISPLAY,
     TYPE, TIMEZONE) ; shell expansion handles the rest at runtime
   - Drop sed substitution except for devcontainer.json and
     .env.example (3 placeholders : PROJECT_ID, DISPLAY_NAME, TZ)
   - Detect existing .devcontainer ; v1.3 markers point to Part 2
     migration prompt instead of auto-updating
   - Bump TEMPLATE_VERSION to 2.0.0
   ```
`````

## Next session

`part-1-session-3-firewall-skills-wiring.md` — bring `templates/
firewall/` and `templates/skills/` up to v2 baseline (addons/,
compile-policy.py, 5 generic skills, sync-skills.sh). To be created
at end of session 2.

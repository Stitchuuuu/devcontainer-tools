# Part 1 — session 1 — scope-audit

> **Effort** : ~1-2 h | **Dependencies** : none. Can run before Part 1
> session 2 (install-redesign) and before session H of
> `devcontainer-v2/phase3-rollout` — but H must be delivered before
> Part 1 session 2 starts.
>
> **Status** : ✅ delivered 2026-05-22. Prompt kept for reproducibility
> (e.g. if SCOPE.md needs to be re-audited later).

## Prompt to paste

`````
Je démarre la Part 1 session 1 (scope-audit) du rollout
`devcontainer-tools-v2-migration`.

Entry point : `/workspace/plans/devcontainer-tools-v2-migration/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (chronological journal)
- `SCOPE.md` (single source of truth for Part 1 file list — refine
  here during this session)
- `EXISTING.md` (legacy v1.3 snapshot — already frozen)
- `sessions/part-1-session-1-scope-audit.md` (this spec)

Goal : freeze the Part 1 file inventory (~84 files) + templating
model before install.sh v2 is rewritten in session 2. No diff -rq
exhaustif, no file-per-file reconciliation — just enough to
validate that nothing critical is missing or surprising.

Session 1 scope :

1. **Top-level inventory** : `ls -la /workspace/.devcontainer/` and
   confirm the SCOPE.md in/out classification is complete (no file
   silently missing from either column).

2. **Placeholder check** : `grep -RE '\{\{[A-Z_]+\}\}'` on the
   in-scope files. Confirm the only placeholders left are
   `{{PROJECT_ID}}`, `{{PROJECT_DISPLAY_NAME}}`, `{{TIMEZONE}}`.
   Flag any surprise placeholder (regression or v1.3 leftover).

3. **Sync inheritance** : verify the two automatic syncs in v2 are
   actually wired :
   - `claude/sync-creds.sh` called from `post-start.sh`,
     `shell-init.sh`, and hooks (`Stop`, `SessionEnd`) via
     `skills/sync-skills.sh`
   - `skills/sync-skills.sh` called from `post-start.sh`
   Document the trigger points in SCOPE.md (already done — verify).

4. **Skills delta** : confirm the 5 generic skills shipped
   (`prepare-pr`, `watch-log`, `prepare-research`, `scan-deps`,
   `prepare-plan`) all exist in `.devcontainer/skills/` with their
   `hooks.json` + `*.skill.md` files. Confirm the 2 `.local` skills
   (`hours.local`, `claude-limits.local`) are **excluded** from
   install.sh (per-user, manual add post-install).

5. **Drops confirmation** : v1.3 templates that v2 drops :
   `gh-secure/`, `Dockerfile.node`, `master-review/`, `KNOWLEDGE.md`
   (single file), `test-db.php`, `gitignore-entries.txt`. Confirm
   each is in SCOPE.md "OUT — dropped" section.

6. **Update the plan files** :
   - `SCOPE.md` : if step 1-5 surfaces anything missing, amend
   - `ROLLOUT.md`, `STATUS.md`, `EXISTING.md` : already restructured
     for the 2-part rollout — verify consistency

7. **Create session 2 prompt** :
   `sessions/part-1-session-2-install-redesign.md` (follows the shape
   of this file). Covers : wizard design (4 prompts), copy-verbatim
   helpers, sed substitution on the 2 templated files, .env
   generation, .gitignore handling, exec perms, summary printout.

Validation :
- SCOPE.md is frozen (no later session amends it without an explicit
  scope-change in the commit message)
- All in-scope files exist in `.devcontainer/` and have correct
  permissions (executables flagged)
- Sync mechanisms documented with file:line references
- Session 2 prompt is ready to paste

DoD at end of this session :
1. STATUS.md : flip Part 1 session 1 row 🚧 → ✅, prompt link → —,
   set "Next focus" → Part 1 session 2 (install-redesign).
2. LOG.md : append `## P1-S1 — scope-audit` section dated today
   (~60-100 lines) with : files inspected, placeholder grep result,
   sync wiring verification (file:line), skills delta, drops
   confirmation, any SCOPE.md amendment.
3. SCOPE.md : final frozen version.
4. Create `sessions/part-1-session-2-install-redesign.md`.
5. Propose a commit (do NOT commit without explicit user confirmation).
   Suggested message :
   ```
   Pivot devcontainer-tools v2 migration to 2-part rollout

   - Drop file-per-file porting (sessions 2-7) in favor of install.sh
     v2 redesign as Part 1 (5 sessions) + deferred Part 2 (Claude
     session prompt for 1.3 to 2.0 migration)
   - Add SCOPE.md as single source of truth for Part 1 file list
     (~35 files : core + lifecycle + firewall + 5 generic skills +
     full knowledge/)
   - Confirm templating model collapses to 3 placeholders + shell
     variable expansion (down from 11 in v1.3)
   ```
`````

## Next session

`part-1-session-2-install-redesign.md` — rewrite `install.sh` v2 :
wizard (4 prompts), copy + minimal sed, `.env` generation,
`.gitignore`, exec perms, summary. To be created at end of session 1.

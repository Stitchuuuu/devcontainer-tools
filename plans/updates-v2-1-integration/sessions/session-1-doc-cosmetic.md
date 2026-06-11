# Session 1 — doc-cosmetic

> **Effort** : ~0.5 day | **Dependencies** : none (first session)

## Prompt to paste

`````
I'm starting session 1 of the `updates-v2-1-integration` rollout.

Entry point : `/workspace/plans/updates-v2-1-integration/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — empty on session 1)
- `EXISTING.md` (current code inventory + recent commits + uncommitted state)
- `sessions/session-1-doc-cosmetic.md` (this spec)

## Scope of session 1

Land the **4 low-risk doc/cosmetic patches** from `updates-v2.1/`, on
**both** `templates/v2/` and `.devcontainer/`, with re-templatisation of
`Symptems` → `{{PROJECT_DISPLAY_NAME}}` on the template side.

Patches in scope :

| # | Patch | Effect |
|---|-------|--------|
| 1 | `20260529-0813-claude-project-display-name-rename.patch` | `claude/CLAUDE-project.md` header → use placeholder template-side, literal `Devcontainer Tools` (or whatever the dogfood DC_PROJECT resolves to) dogfood-side |
| 5 | `20260604-1022-prepare-plan-collapse-session-wrapper.patch` | `skills/prepare-plan/prepare-plan.skill.md` drops 5-backtick fence wrapper |
| 6 | `20260604-1023-claude-dev-query-registry-for-versions.patch` | `claude/CLAUDE-dev.md` adds `npm view` / `composer show -a` guidance |
| 12 | `20260610-0938-align-post-start-banner-widths.patch` | `post-start.sh` normalises 4 warning boxes to 64-char inner width |

## Constraints (from ROLLOUT.md Decisions section)

- **Both surfaces in the same session.** A change lands in `templates/v2/`
  (templatised) AND `.devcontainer/` (concrete values) before the session
  ends. Do not split between sessions.
- **No `Symptems` / `symptems` survives in `templates/v2/`** — anywhere.
- **Existing commits already overlap** : the recent commits in EXISTING.md
  show patches #2, #3, #4 were partially landed. Verify whether any of the
  4 patches in this session are also partially landed before applying.
  Use `git log --oneline -20` + targeted greps.
- **No scope creep.** If you spot an issue outside these 4 patches, log
  it in LOG.md (Gotchas) and propose a session row instead of folding it
  in.

## Concrete steps

1. **Inventory pre-state** — read each of the 4 patches and check whether
   their hunks are already applied (look at the target file's current
   content). Report partial state in LOG.md.
2. **Decide on the uncommitted state** noted in EXISTING.md (M README.md,
   M CHANGELOG.md, etc.). Ask the user if unclear ; default is to leave
   the uncommitted state untouched and work in a clean sub-area.
3. **Apply patch #1** :
   - On `.devcontainer/claude/CLAUDE-project.md` : keep current dogfood
     value (likely `Devcontainer Tools` or current first line).
   - On `templates/v2/claude/CLAUDE-project.md` : use `{{PROJECT_DISPLAY_NAME}}`.
4. **Apply patch #5** : on both `skills/prepare-plan/prepare-plan.skill.md`
   copies (template + dogfood). Verify the skill still parses correctly.
5. **Apply patch #6** : on both `claude/CLAUDE-dev.md` copies. Pure doc,
   verbatim copy.
6. **Apply patch #12** : on both `post-start.sh` copies. Visual check —
   the 4 warning boxes must have uniform 64-char inner width.
7. **Verification** :
   - `grep -rn "Symptems\|symptems" templates/v2/ .devcontainer/` → must
     return zero results for these 4 files.
   - Visual diff `diff -ru .devcontainer/claude/CLAUDE-dev.md templates/v2/claude/CLAUDE-dev.md` should show only project-id substitutions, no other drift.
   - If feasible (post-start.sh runs at container boot only), eyeball the
     banner widths in the patch hunks rather than rebuilding.

## DoD at the end of this session

1. **STATUS.md** : flip session 1 row 📋 → ✅, prompt link → `—`, bump
   Delivered counter (0 → 1), refresh "Next focus" to session 2
   (shell-and-settings).
2. **LOG.md** : append `## 1 — doc-cosmetic` section dated 2026-06-11
   with files touched + What / Why / Decisions / Gotchas / Tests /
   Commit. Surface any partial-landed state encountered, and any drift
   between `.devcontainer/` and `templates/v2/` you had to bridge.
3. **EXISTING.md** : update if the patch contents revealed inventory
   inaccuracies. Otherwise skip.
4. **Create `sessions/session-2-shell-and-settings.md`** following the
   same template as this file, scoped to patches #2, #3, #4, #7. Mark
   in STATUS.md as `[→ prompt](sessions/session-2-shell-and-settings.md)`.
5. **Propose a commit** with a message focused on doc/cosmetic patches
   from updates-v2.1 (don't reference the rollout plan ID per
   CLAUDE.md §10). Do NOT commit without explicit user confirmation.
`````

## Next session

Session 2 — shell-and-settings : patches #2 (OMZ + zshrc), #3 (port-forward
ignore), #4 (settings.local.json seeding), #7 (remove gh auth block). Higher
risk than session 1 because shell-init.sh and post-create.sh are touched.

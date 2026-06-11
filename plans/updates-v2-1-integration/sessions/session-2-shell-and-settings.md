# Session 2 — shell-and-settings

> **Effort** : ~1 day | **Dependencies** : session 1 (doc-cosmetic) delivered
> **Risk** : higher than S1 — touches `shell-init.sh` + `post-create.sh`,
> which run on every boot / first-create respectively.

## Prompt to paste

`````
I'm starting session 2 of the `updates-v2-1-integration` rollout.

Entry point : `/workspace/plans/updates-v2-1-integration/ROLLOUT.md`
Read also :
- `STATUS.md` (where we are)
- `LOG.md` (what's been done so far — session 1 delivered, see `## 1 — doc-cosmetic`)
- `EXISTING.md` (current code inventory + recent commits + uncommitted state)
- `sessions/session-2-shell-and-settings.md` (this spec)

## Scope of session 2

Land the **4 shell-and-settings patches** from `updates-v2.1/`, on
**both** `templates/v2/` and `.devcontainer/`, with re-templatisation of
`Symptems` → `{{PROJECT_DISPLAY_NAME}}` / `symptems` → `{{PROJECT_ID}}` on
the template side.

Patches in scope :

| # | Patch | Effect |
|---|-------|--------|
| 2 | `20260529-1657-install-oh-my-zsh-per-dev-override.patch` | `Dockerfile.base` + `zshrc-base` + `zshrc.local.example` + `shell-init.sh` + `.gitignore` — install OMZ + per-dev zshrc override |
| 3 | `20260529-1700-port-forward-default-ignore.patch` | `devcontainer.json` + `README.md` — `otherPortsAttributes.onAutoForward = "ignore"` |
| 4 | `20260529-1735-auto-seed-claude-settings-local-json.patch` | `.claude/settings.local.json.example` (already-shipped after S1) + `post-create.sh` + `.gitignore` — auto-seed `.claude/settings.local.json` on first boot if missing |
| 7 | `20260604-1038-remove-git-ref-shell-init.patch` | `shell-init.sh` — strip the interactive `gh auth login` prompt block |

## Constraints (from ROLLOUT.md Decisions section)

- **Both surfaces in the same session.** A change lands in `templates/v2/`
  (templatised) AND `.devcontainer/` (concrete values) before the session
  ends. Do not split between sessions.
- **No `Symptems` / `symptems` survives in `templates/v2/`** — anywhere.
- **Existing commits already overlap heavily for patches #2, #3, #4** :
  recent commits in EXISTING.md (`89f1f00`, `009d9eb`, `0f97e18`, `98498a8`,
  `4f4983f`, `bb9c7fa`, `0363603`, `40116ec`, `9fa7d25`) show partial
  landings on both surfaces. **The first task this session is to
  audit which hunks of each patch still need to land** versus what's
  already in main. Use `git log --oneline -30` + targeted greps against
  the patches' hunks. Patch #7 has no overlap (still fully pending).
- **No scope creep.** If you spot an issue outside these 4 patches, log
  it in LOG.md (Gotchas) and propose a session row instead of folding it
  in.

## Concrete steps

1. **Inventory pre-state** — read each of the 4 patches and check which
   hunks are already applied (look at the target file's current content
   and compare against the patch's "+" lines). Report the per-hunk landed
   / pending matrix in LOG.md.
2. **Decide on the uncommitted state** noted in EXISTING.md. Default is
   to leave it untouched and work in a clean sub-area — patch #4 may
   surface a partial landing on `.claude/settings.local.json.example`
   (already extended in S1 with `npm view*` + `npm search*`), so a
   second extension must not regress that.
3. **Apply patch #2** (OMZ + per-dev zshrc override) on remaining hunks
   only :
   - `templates/v2/Dockerfile.base` — verify Oh My Zsh install block
     (commits `bb9c7fa` + `0363603` may have landed parts).
   - `templates/v2/zshrc-base` + `templates/v2/zshrc.local.example` —
     verify per-dev override sourcing.
   - `templates/v2/shell-init.sh` — verify ZSH_CUSTOM redirect drop
     (commit `98498a8` did this).
   - `templates/v2/.gitignore` — verify `.devcontainer/zshrc` ignore.
   - Mirror to `.devcontainer/` side.
4. **Apply patch #3** (port-forward ignore) on remaining hunks only :
   - `templates/v2/devcontainer.json` + `.devcontainer/devcontainer.json` —
     verify `otherPortsAttributes.onAutoForward = "ignore"` (commit
     `4f4983f` did this).
   - `templates/v2/README.md` + `.devcontainer/README.md` (the dogfood
     `.devcontainer/README.md` may not exist — check) — verify
     port-forward policy documented.
5. **Apply patch #4** (settings.local.json auto-seed) on remaining hunks
   only :
   - `templates/v2/.claude/settings.local.json.example` — was extended in
     S1, do NOT regress. Patch #4 hunk 1 adds this file with a different
     initial content — reconcile to keep S1 additions.
   - `templates/v2/post-create.sh` + `.devcontainer/post-create.sh` —
     verify auto-seed block (commits `89f1f00` + `009d9eb`).
   - `templates/v2/.gitignore` + workspace-root `.gitignore` — verify
     `.claude/settings.local.json` ignore.
6. **Apply patch #7** (remove `gh auth login` prompt block) on
   `templates/v2/shell-init.sh` + `.devcontainer/shell-init.sh` —
   strip the interactive block referenced in the patch.
7. **Verification** :
   - `grep -rn "Symptems\|symptems" templates/v2/ .devcontainer/` →
     zero results.
   - `diff -u` on each `.devcontainer/<file>` ↔ `templates/v2/<file>`
     pair touched — should differ only on `{{PROJECT_ID}}` /
     `{{PROJECT_DISPLAY_NAME}}` placeholders.
   - Rebuild check (if feasible) : restart the devcontainer and confirm
     shell-init.sh runs without prompting for `gh auth login`. If not
     feasible mid-session, eyeball the script flow.
   - Sanity-check `.claude/settings.local.json.example` still contains
     S1's `Bash(npm view*)` + `Bash(npm search*)` entries after patch #4
     reconcile.

## DoD at the end of this session

1. **STATUS.md** : flip session 2 row 📋 → ✅, prompt link → `—`, bump
   Delivered counter (1 → 2), refresh "Next focus" to session 3
   (dockerfile-cache-split).
2. **LOG.md** : append `## 2 — shell-and-settings` section dated today
   with files touched + What / Why / Decisions / Gotchas / Tests /
   Commit. Surface the per-hunk landed/pending matrix from step 1, plus
   any drift between `.devcontainer/` and `templates/v2/` you had to
   bridge.
3. **EXISTING.md** : update if the patch contents revealed inventory
   inaccuracies (e.g. files added/removed not previously catalogued).
4. **Create `sessions/session-3-dockerfile-cache-split.md`** following
   the same template as this file, scoped to patch #13 (skip the
   superseded #8, #9). Mark in STATUS.md as
   `[→ prompt](sessions/session-3-dockerfile-cache-split.md)`.
5. **Propose a commit** with a message focused on shell + settings
   patches from updates-v2.1 (don't reference the rollout plan ID per
   CLAUDE.md §10). Do NOT commit without explicit user confirmation.
`````

## Next session

Session 3 — dockerfile-cache-split : patch #13 (Dockerfile.base 6-RUN
cache split for Claude install). Smaller scope than S2, single file
touched, but rebuild-sensitive — must validate cache layering keeps
the Claude install reproducible.

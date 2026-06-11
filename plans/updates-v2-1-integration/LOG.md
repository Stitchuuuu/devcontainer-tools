# Log — Updates v2.1 integration

> Append-only journal. One section per delivered session. Newest at the
> bottom. Each section follows the same shape :

```
## <Session ID> — <Title>

**Date** : YYYY-MM-DD
**Files touched** :
- path/to/file1
- path/to/file2

**What** : one-paragraph summary of the change.

**Why** : the reason / constraint that drove this scope.

**Decisions** :
- _bullet — short rationale_

**Gotchas** :
- _bullet — surprise or pitfall encountered_

**Tests** :
- _command run + expected outcome_

**Commit** : `<short hash> — <commit subject>` (or "not committed yet")
```

---

## 1 — doc-cosmetic

**Date** : 2026-06-11
**Files touched** :
- `.devcontainer/claude/CLAUDE-project.md`
- `.devcontainer/claude/CLAUDE-dev.md`
- `templates/v2/claude/CLAUDE-dev.md`
- `.devcontainer/skills/prepare-plan/prepare-plan.skill.md`
- `templates/v2/skills/prepare-plan/prepare-plan.skill.md`
- `templates/v2/.claude/settings.local.json.example`
- `.devcontainer/post-start.sh`
- `templates/v2/post-start.sh`

**What** : Landed the 4 low-risk doc/cosmetic patches from `updates-v2.1/` —
#1 (CLAUDE-project header placeholder), #5 (prepare-plan skill drops the
session-prompt wrapper), #6 (CLAUDE-dev §13 registry-query guidance + npm
view/search allowlist), #12 (post-start.sh banner widths normalised to
64-char). Both surfaces (`templates/v2/` + `.devcontainer/`) updated in
lockstep; template-side already correct on patch #1.

**Why** : Kick off the v2.1 integration with the lowest-risk batch first.
These patches are doc / cosmetic only — no runtime behaviour change, no
shell-init or post-create touched (deferred to S2). Lets the rollout warm
up without compounding risk on the first session.

**Decisions** :
- Patch #6 applied as a coherent unit (both hunks) per user choice — the
  CLAUDE-dev guidance to run `npm view` / `npm search` would otherwise
  trigger permission prompts on first use in downstream projects.
- Patch #1 template-side : no-op, the file was already
  `# {{PROJECT_DISPLAY_NAME}} — Project rules`. Only dogfood needed the
  literal substitution (`Devcontainer Tools`).
- Patch #5 hunks 1 & 2 (the `single`/`multi` mode block deletions) :
  no-op locally — the block was never added on this side, it was a
  Symptems-fork addition. Only the wrapper removal + help-text +
  callout-replacement parts had effect.
- Uncommitted state from EXISTING.md left untouched (M README.md,
  M CHANGELOG.md, M plans/.../STATUS.md, M domains.txt × 2,
  D MIGRATION-1.1.0.md, untracked items). None intersect with the
  4 patches in scope.

**Gotchas** :
- `prepare-plan.skill.md` was internally inconsistent before this session :
  help text at line 197 said "4-backtick fences" but the actual session
  template at line 365 used 5-backtick fences. Patch #5 resolves both by
  dropping the wrapper entirely (no fences at all).
- `.claude/settings.local.json.example` exists **only** at
  `templates/v2/.claude/` — no workspace-root copy and no
  `.devcontainer/.claude/` copy. The dogfood gets seeded via post-create
  reading from the template path at first boot. So patch #6 hunk 1 was a
  single-file edit, not dual.
- Banner-width spot-check confirmed all 4 boxes now align at 64-char inner
  width (boxes 1 + 2 were already at 64; boxes 3 + 4 expanded).
- **Template hygiene audit (user-requested)** : confirmed S1 introduced
  **zero** project-specific literals in `templates/v2/`. The 5 files I
  touched template-side contain no `Symptems`/`symptems`,
  `Devcontainer Tools`, `devcontainer-tools`, or `portal-*` literals.
  However, `grep -rn "devcontainer-tools" templates/v2/` surfaces 13
  pre-existing references across 5 files **outside S1 scope** —
  `tests/README.md`, `tests/lib.sh`, `knowledge/INDEX.md`,
  `skills/master-review.local/{install-manual.sh,README.md}`. These are
  **upstream-tool-name references** (the downstream user types
  `bash /path/to/devcontainer-tools/update.sh` to update their install,
  not their project name), not identity leaks. **Flag for session 5
  installer audit** : confirm `tests/` even ships to downstream, and
  that the upstream-tool naming reads correctly from a `portal-xxx`
  downstream perspective.

**Tests** :
- `grep -rn "Symptems\|symptems" templates/v2/ .devcontainer/` → zero
  results (clean baseline preserved).
- `diff -q` on all three identical pairs (CLAUDE-dev.md, prepare-plan.skill.md,
  post-start.sh) → empty (still byte-identical post-edit).
- `diff -u` on the CLAUDE-project.md pair → only line 1 differs (the
  expected `Devcontainer Tools` vs `{{PROJECT_DISPLAY_NAME}}` divergence).
- `git diff --stat` → 8 files changed in scope (10 reported total includes
  the 2 unrelated `domains.txt` files left over from the uncommitted state).
- Visual banner check on the 4 boxed warnings in post-start.sh → uniform
  64-char alignment confirmed.

**Commit** : not committed yet — proposed message awaiting user confirmation.

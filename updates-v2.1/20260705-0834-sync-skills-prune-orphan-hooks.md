# 20260705-0834 — `sync-skills.sh` prunes orphan **and** disabled hooks + backs up `settings.json`

**Affects** : v2.1 devcontainers whose `sync-skills.sh` only merges
`hooks.json` from every skill but never prunes stale handlers. Two
distinct causes lead to the same runtime failure :

1. A skill is renamed/removed after its hook was merged — the orphan
   handler survives in `~/.claude/settings.json` and points at a
   missing target file.
2. A skill is disabled by renaming `<name>.skill.md` →
   `<name>.skill.disabled.md` (soft-off) — its `hook.js` still exists
   on disk, so the previous prune logic keeps the merged hook alive
   and the disabled skill still fires.

**Symptom** :

- Claude Code crashes at session start with an error like
  `no such file or directory: /workspace/.devcontainer/skills/<gone>/hook.js`
  when a hook target is missing.
- Or : a "disabled" skill (renamed to `.skill.disabled.md`) still runs
  its hooks and stays installed as a slash-command in
  `~/.claude/commands/<name>.md`, because sync-skills only reacted to
  file deletions, not to the disable convention.

**Fix** : extend `sync-skills.sh` on two fronts.

1. **Prune-orphan (missing target)** — walk `settings.json.hooks`,
   parse each handler's `command` to extract its target path, drop
   the handler if the file doesn't exist. Print `⚠ pruned N hook(s):`
   with per-line `event: [skill] path (missing)`.
2. **Detect + prune disabled skills** — a skill directory containing
   `*.skill.disabled.md` and **no** active `*.skill.md` is treated as
   disabled. For each disabled skill : (a) delete any stale
   `~/.claude/commands/<name>.md` left over from a previous sync,
   (b) skip its `hooks.json` during merge, (c) prune any hook already
   merged into `settings.json` that references its directory —
   labelled `(disabled)` in the log.
3. **Backup on write** — back up `settings.json` before any mutation
   to a timestamped `.bak` under `.devcontainer/claude/settings-backups/`
   (gitignored) so a mis-prune can be reversed.

Disable/re-enable workflow becomes a one-liner :

`````bash
mv .devcontainer/skills/foo/foo.skill.md .devcontainer/skills/foo/foo.skill.disabled.md
bash .devcontainer/skills/sync-skills.sh
`````

→ command removed, `hooks.json` skipped, merged hooks pruned. Reverse
the `mv` + re-run to re-enable. `hook.js` stays on disk untouched.

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the
[Targeted updates bootstrap](../UPGRADE-v2.md#targeted-updates-updatesname)
populates `.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/20260705-0834-sync-skills-prune-orphan-hooks.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/20260705-0834-sync-skills-prune-orphan-hooks.patch

git add .devcontainer/skills/sync-skills.sh .gitignore
git commit -m "fix(sync-skills): prune orphan + disabled hooks, backup settings.json"
`````

The patch touches `.devcontainer/skills/sync-skills.sh` and `.gitignore`
(adds the `settings-backups/` ignore). No daemon to restart : the
next post-start (or a manual run) applies the new logic.

## Verify

- [ ] `grep -c 'BAK_DIR=' .devcontainer/skills/sync-skills.sh` → `1`.
- [ ] `grep -c 'disabled_skills' .devcontainer/skills/sync-skills.sh` → `≥ 3`
      (declaration + merge-skip + prune).
- [ ] `grep -c '1bis. Remove stale commands' .devcontainer/skills/sync-skills.sh` → `1`.
- [ ] `grep -c 'settings-backups' .gitignore` → `1`.
- [ ] `bash .devcontainer/skills/sync-skills.sh` on a clean state
      prints `✓ hooks already up to date` (or the merge banner). No error.
- [ ] **Orphan test** — insert a fake hook in `~/.claude/settings.json`
      pointing at `/workspace/.devcontainer/skills/ghost/x.js`. Re-run
      the script. Expected : `⚠ pruned 1 hook(s):` and the log line
      ends with `(missing)`. A `settings.<ts>.bak` file appears under
      `.devcontainer/claude/settings-backups/`.
- [ ] **Disabled test** — pick a real skill (e.g. `foo`) with a
      `hooks.json`, rename `foo.skill.md` → `foo.skill.disabled.md`,
      re-run. Expected : `✗ skill foo.md removed (disabled)` and
      `⚠ pruned N hook(s):` lines ending with `(disabled)`. Reverse
      the `mv` + re-run → the hooks come back.

## Rollback

`````bash
git revert <commit-hash>
`````

If a prune-run wiped a legitimate hook, restore the pre-run backup :

`````bash
cp .devcontainer/claude/settings-backups/settings.<latest-ts>.bak /home/node/.claude/settings.json
`````

# 20260716-0812 — `initialize.sh` split into `initialize/` helpers

**Affects** : v2.x devcontainers carrying `initialize.sh` before this
update — i.e. anything based on `devcontainer-tools` `main` at or before
commit `cf3e20c` (Android/Capacitor firewall block + Mac virtiofs stub).

**Symptom** : none functionally — `initialize.sh` grew to ~1050 lines,
40 % of which is either purely opt-in diagnostic (`dump_rebuild_context`,
gated behind `DEBUG_REBUILD_CONTEXT=1`) or a self-contained subsystem
(the entire notify daemon spawn + fallback node diagnostic). Reading /
diffing / navigating the entry-point file became noisy.

**Cause** : two pieces incrementally grew inside the same file over
several sessions. The notify daemon started small (~20 lines) then
picked up spawn-outcome classification, a ~180-line node-not-found
diagnostic, and a channel-readback loop. `dump_rebuild_context` was
added once to reverse-engineer VS Code's `--no-cache` propagation and
stayed as reference — but it only ever runs opt-in.

**Resolution** : new `initialize/` folder next to `initialize.sh`
carrying two sourced helpers, one edit at each call site.

- `initialize/notify-daemon.sh` — 357 lines. Contains
  `_notify_daemon_diag()` (host / shell / env / install-locations dump
  triggered when `node` is absent from PATH) and `spawn_notify_daemon()`
  (idempotent detached spawn + post-spawn outcome classification +
  channel readback). Sourced unconditionally right before the call in
  `initialize.sh` since the daemon runs on every rebuild.
- `initialize/rebuild-debug.sh` — 64 lines. Contains
  `dump_rebuild_context()`. Sourced only when
  `DEBUG_REBUILD_CONTEXT=1` — zero cost on the default hot path.

`initialize.sh` drops from ~1050 to ~650 lines (−38 %). Behaviour is
byte-identical : the extracted functions read the same env vars
(`DEVCONTAINER_DIR`, `PROJECT_DIR`, `HOST_KIND`, `ORIG_STDOUT_TTY`),
have no top-level side effects, and are called at the same moments.

**Upstream commits** :
- (this update ships as one bundle — no separate upstream commit yet).

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname)
populates `.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260716-0812-initialize-split-into-helpers.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260716-0812-initialize-split-into-helpers.patch

git add .devcontainer/initialize.sh \
        .devcontainer/initialize/notify-daemon.sh \
        .devcontainer/initialize/rebuild-debug.sh
git commit -m "refactor(devcontainer): split initialize.sh into initialize/ helpers"
`````

No rebuild required — the split takes effect on the next
`initializeCommand` run (any VS Code container start). If the notify
daemon is currently running (`.devcontainer/notify/queue/.daemon.pid`
present with a live PID), it keeps running — the daemon binary itself
is untouched, only the spawn helper moved.

## Verify

- [ ] `bash -n .devcontainer/initialize.sh` → no output (syntax OK).
- [ ] `bash -n .devcontainer/initialize/notify-daemon.sh` → no output.
- [ ] `bash -n .devcontainer/initialize/rebuild-debug.sh` → no output.
- [ ] `wc -l .devcontainer/initialize.sh` → ~650 lines (was ~1050).
- [ ] `grep -c '^dump_rebuild_context\|^spawn_notify_daemon\|^_notify_daemon_diag' .devcontainer/initialize.sh`
      → `0` (all 3 functions extracted).
- [ ] `grep -F 'initialize/notify-daemon.sh' .devcontainer/initialize.sh`
      → 1 match (source line present).
- [ ] `grep -F 'initialize/rebuild-debug.sh' .devcontainer/initialize.sh`
      → 1 match (opt-in source line present).
- [ ] Next VS Code Rebuild Container : `.devcontainer/logs/initialize-*.log`
      still shows the same lines (`✓ Notify daemon spawned ...` or the
      diagnostic block if `node` is missing).

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
rm -rf .devcontainer/initialize
`````

The `rm -rf` is needed because `git revert` on the modification hunk
restores the inlined functions in `initialize.sh` but doesn't remove
the now-unused `.devcontainer/initialize/` folder (git tracks the
files individually, revert removes the additions but the folder
persists if empty after untracking).

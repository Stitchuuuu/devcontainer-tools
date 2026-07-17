# notify-daemon nvm auto-source + node-missing diagnostic

**Upstream commit:** `33fbd31` (2026-07-16 09:07 UTC)

## Why this recipe exists

The follow-up recipe `20260716-0812-initialize-split-into-helpers` was
generated from a base that INCLUDED commit `33fbd31`. Downstream forks that never
applied `33fbd31` will hit a hunk mismatch in the split recipe (the "before" state
of `initialize.sh` is 214 lines shorter than what the split expects).

This intermediate recipe bumps `.devcontainer/initialize.sh` from the state at
commit `eefa6b2` to the state at `33fbd31`, so the split recipe applies cleanly on
top.

## What it changes

`.devcontainer/initialize.sh` — 3 hunks around the notify-daemon spawn block:

1. **Auto-source nvm when `node` isn't on PATH.** VS Code's `initializeCommand`
   runs a non-login shell, so `nvm`-installed node binaries aren't visible without
   sourcing `$NVM_DIR/nvm.sh`. Best-effort with `|| true` to survive `set -eE`.

2. **Wide diagnostic dump in the "node not found" fallback.** New helper
   `_notify_daemon_diag()` collects identity, host/kernel info, parent-process
   chain (6 levels), editor markers, full PATH (verbatim + numbered), node-manager
   env vars, locale, filtered env, scan of known install locations (nvm, brew,
   volta, fnm, asdf, mise, proto, Windows paths), adjacent tooling. Wrapped in
   `( set +e; ... )` so a diagnostic failure can't abort `initialize.sh`.

3. **`spawn_notify_daemon()` calls the new helper** before giving up when node
   is unreachable.

## Apply

```sh
git apply 20260716-0800-notify-daemon-nvm-autosource.patch
```

## Verification

```sh
grep -n '_notify_daemon_diag' .devcontainer/initialize.sh
# Expect 3 matches: function definition, doc pointer, call site.
```

## Base / target blob hashes

- `.devcontainer/initialize.sh` : `06ab7c9` → `bcc5afb`

The target hash matches the "before" hash of the immediately following recipe
`20260716-0812-initialize-split-into-helpers`.

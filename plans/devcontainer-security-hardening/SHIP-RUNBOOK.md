# Ship Runbook — session 1 (bake-firewall-config)

> Self-contained handoff. Each rebuild kills the Claude session running
> in the container, but this file + the validation state log
> (`.devcontainer/logs/session-1-validation.log`) persist via the
> workspace bind mount. New Claude session = same plan.

## Where we are

- **Code changes** : done (45 unit tests pass).
- **Commit 1** : `7b9ff66 — docs(security): add SECURITY-AUDIT-2026-05 (13 vectors)` — already committed.
- **Commit 2** : pending — must validate post-rebuild before staging.
- **Validation strategy** : full multi-mode matrix (strict, basic, off).

## Runbook — 4 rebuilds

### Step 1 — Rebuild #1 (strict mode, default)

```bash
# In VS Code : Cmd+Shift+P → "Dev Containers: Rebuild Container"
# After ~5-10 min you're in the rebuilt container.
```

Then, inside the new container :

```bash
bash .devcontainer/tests/validate-session-1.sh
```

Expected : integration suite passes for mode=strict. Script prints
"Next : flip to basic + rebuild".

### Step 2 — Rebuild #2 (basic mode)

```bash
bash .devcontainer/firewall-mode.sh basic
# Cmd+Shift+P → Rebuild Container
```

After rebuild :

```bash
bash .devcontainer/tests/validate-session-1.sh
```

Expected : passes for mode=basic. Script prints "Next : flip to off".

### Step 3 — Rebuild #3 (off mode)

```bash
bash .devcontainer/firewall-mode.sh off
# Cmd+Shift+P → Rebuild Container
```

After rebuild :

```bash
bash .devcontainer/tests/validate-session-1.sh
```

Expected : passes for mode=off. Script prints
"✅ All 3 modes validated. Ready to commit."

### Step 4 — Rebuild #4 (back to strict, sanity)

```bash
bash .devcontainer/firewall-mode.sh strict
# Cmd+Shift+P → Rebuild Container
```

Optional sanity re-run :

```bash
bash .devcontainer/tests/validate-session-1.sh
```

### Step 5 — Commit 2

From inside the container (or host — both work since it's just `git`) :

```bash
cd /workspace
git status   # sanity check — expected files listed below

git add \
  .devcontainer/.gitignore \
  .devcontainer/Dockerfile \
  .devcontainer/docker-compose.yml \
  .devcontainer/firewall-mode.sh \
  .devcontainer/firewall/default-mode \
  .devcontainer/firewall/direct-tcp-allow.txt \
  .devcontainer/firewall/policy.local.d/ \
  .devcontainer/host-helpers/claude-switch \
  .devcontainer/init-firewall.sh \
  .devcontainer/initialize.sh \
  .devcontainer/on-create.sh \
  .devcontainer/post-start.sh \
  .devcontainer/shell-init.sh \
  .devcontainer/test-firewall.sh \
  .devcontainer/tests/ \
  install.sh \
  plans/devcontainer-security-hardening/ \
  templates/v2/.gitignore \
  templates/v2/Dockerfile \
  templates/v2/Dockerfile.php \
  templates/v2/docker-compose.yml \
  templates/v2/firewall-mode.sh \
  templates/v2/firewall/default-mode \
  templates/v2/firewall/direct-tcp-allow.txt \
  templates/v2/firewall/policy.local.d/ \
  templates/v2/host-helpers/claude-switch \
  templates/v2/init-firewall.sh \
  templates/v2/initialize.sh \
  templates/v2/on-create.sh \
  templates/v2/post-start.sh \
  templates/v2/shell-init.sh \
  templates/v2/test-firewall.sh \
  templates/v2/tests/

git rm -r templates/v2/firewall/ranges.d/ 2>/dev/null || true
# (already staged for deletion — line above is a no-op safety net)

git commit -m "$(cat <<'EOF'
security: bake firewall config in image, drop runtime bind mount

The :ro mount at /etc/devcontainer-firewall/ was cosmetic — the same
host inodes were RW-accessible via the workspace mount, so writes
through /workspace/.devcontainer/firewall/* propagated to /etc at the
next read. Closes the 6 critical vectors that share this root flaw
(#1, #2, #3, #8, #12, #13 from SECURITY-AUDIT-2026-05).

Bake-only changes:
- Drop the ./firewall:/etc/devcontainer-firewall:ro bind mount from
  docker-compose. The runtime view comes from the image only.
- Project Dockerfile + Dockerfile.php use a single recursive
  COPY firewall/ /etc/devcontainer-firewall/ (replaces granular COPYs).
- New baked files in firewall/: default-mode (strict by default,
  read by init-firewall.sh instead of FIREWALL_MODE env var) and
  direct-tcp-allow.txt (host:port per line, replaces the .env
  CLAUDE_CODE_FIREWALL_ALLOWED CSV).
- init-firewall.sh, test-firewall.sh: read from the baked paths.
- post-start.sh, on-create.sh: drop the FIREWALL_MODE +
  CLAUDE_CODE_FIREWALL_ALLOWED env-injection (only debug toggle
  remains in /tmp/.firewall-env; session 2 will remove the source).
- firewall-mode.sh, shell-init.sh, initialize.sh: relocate the
  mode flag from workspace .configured-firewall-mode to baked
  firewall/default-mode (with idempotent legacy migration in
  install.sh + initialize.sh for adopting projects).
- host-helpers/claude-switch: also rewrites firewall/direct-tcp-allow.txt
  on each mode change, so the TCP allowlist tracks the active backend.

Housekeeping:
- Remove templates/v2/firewall/ranges.d/ (orphan WIP, no consumer).
- Add templates/v2/firewall/policy.local.d/ as an empty committed
  directory (placeholder .keep tracked via !pattern in gitignore).
- Restructure .gitignore: ignore patterns grouped first, then all
  negations, instead of alternating.
- install.sh declares migrate_legacy_firewall(): one-shot move of
  .configured-firewall-mode → firewall/default-mode and
  CLAUDE_CODE_FIREWALL_ALLOWED → firewall/direct-tcp-allow.txt.

Tests:
- New templates/v2/tests/ harness (lib.sh assertions, run.sh runner)
  with unit/ (static repo invariants, ~45 assertions) and
  integration/ (post-rebuild boundary checks + per-mode invariants).
- Multi-rebuild plan in plans/devcontainer-security-hardening/TEST-PLAN.md.
- validate-session-1.sh orchestrates the strict/basic/off matrix
  across rebuilds, tracking progress in .devcontainer/logs/.

Vectors blocked (per audit): #1, #2, #3, #8, #12, #13. Vector #4
(/tmp/.firewall-env source-as-root) is the target of session 2.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"

git status   # verify commit landed clean
```

## If validation fails

Don't commit. Investigate. The failure output in
`.devcontainer/logs/session-1-validation.log` shows exactly which
assertion(s) tripped.

Common causes :
- `findmnt` missing → script falls back to `/proc/self/mountinfo`, should still work.
- Curl probes flaky → re-run ; if persistent, check that init-firewall.sh actually applied (look in postStart logs).
- Mode flip not picked up → did you rebuild AFTER `firewall-mode.sh basic` ? The flag is baked, reload is not enough.

## Rollback (if firewall breaks runtime)

```bash
# On the host, from this repo :
git status            # see what's staged/uncommitted
git stash             # save current state if any
git revert HEAD       # undo commit 2 (creates inverse commit)
# OR
git reset --hard HEAD^   # ⚠ destructive ; only if HEAD points to bake commit and you're sure

# Then Rebuild Container in VS Code. The base image still has the
# pre-bake code paths cached, so rebuild succeeds.
```

If the container is so broken it won't start, the rebuild itself
restores by re-reading the (now-reverted) Dockerfile + scripts.

## Files NOT staged for this commit (pre-existing, unrelated)

```
M  ROADMAP.md
M  plans/devcontainer-tools-v2-migration/LOG.md
M  plans/devcontainer-tools-v2-migration/STATUS.md
M  plans/devcontainer-tools-v2-migration/sessions/part-1-session-3-firewall-layer-split.md
M  plans/devcontainer-tools-v2-migration/sessions/part-1-session-4-fresh-install-test.md
?? .vscode/
?? plans/devcontainer-tools-v2-migration/sessions/part-1-session-3b-gitignore-architecture-refactor.md
?? plans/devcontainer-tools-v2-migration/sessions/part-1-session-5-bump-changelog.md
```

These are leftovers from a prior rollout. Leave untouched.

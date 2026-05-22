# How-to — Test install.sh v2 on host (session 3)

> Quick host-side runbook for Part 1 session 3 (fresh-install-test).
> For the full Claude session prompt, see
> [sessions/part-1-session-3-fresh-install-test.md](sessions/part-1-session-3-fresh-install-test.md).
> For the rollout context, see [ROLLOUT.md](ROLLOUT.md).

## Why a separate sandbox

This devcontainer's own `.devcontainer/` is occupied — testing the
installer against `/workspace` would either skip (existing v2 marker)
or overwrite the live setup. Run the test against a **brand new
directory on the host**, with its own VS Code instance.

## Prerequisites

- Docker daemon running on host
- VS Code + Dev Containers extension
- The real `devcontainer-tools` repo sits at `../devcontainer-tools-v2/`
  on host (sibling of the project root). `/workspace/devcontainer-tools/`
  inside this container is a **copy** of that — work happens here, then
  syncs back to the real repo (see [§ Sync container → host repo](#sync-container--host-repo) below).
- ~8 min slack for the first base image build (cold cache)

## Sync container → host repo

The `devcontainer-tools/` directory inside `/workspace/` is bind-mounted
from host (so the host already SEES the in-container edits at
`<host-workspace>/devcontainer-tools/`), but the **actual repo** where
commits land is outside the bind-mount at `../devcontainer-tools-v2/`.
Sync is a host-side rsync :

```bash
# Run on host, from the project root (<host-workspace>) :
rsync -av --delete \
  --exclude='.git' --exclude='.github' --exclude='*.bak' \
  devcontainer-tools/ ../devcontainer-tools-v2/
```

Trailing slash on `devcontainer-tools/` is significant — copies
*contents* into the target, not the dir itself.

**`--delete` is important** — propagates removals (e.g. `update.sh`
dropped, `gh-secure/` deleted from `templates/`, Dockerfile.custom →
Dockerfile rename). Without it, removed files survive in the target.

**Excluded** : `.git/` (target has its own history), `.github/` (CI
config that may diverge), `*.bak` (sed backup leftovers if any).

Then commit on host in `../devcontainer-tools-v2/` with the message
proposed at the end of the session (see [LOG.md § P1-S2](LOG.md#p1-s2--install-redesign)).


## Steps

### 1. Create a sandbox project on host

```bash
SANDBOX=~/sandbox/dctest-v2-$(date +%s)
mkdir -p $SANDBOX
cd $SANDBOX
git init   # optional, but useful to see .gitignore impact
```

### 2. Run the installer

```bash
bash /path/to/devcontainer-tools/install.sh $SANDBOX
```

Wizard prompts (Enter for default everywhere) :
- Project slug → defaults to slugified basename
- Display name → defaults to titlecase
- Project type → 1 = Node.js (default)
- Shared creds volume → enter the same volume name your current
  devcontainer uses (e.g. `claude-creds-shared-boa`) so OAuth carries
  over and you don't need a fresh login

Confirm summary → `Y`.

### 3. File-copy sanity check

```bash
ls $SANDBOX/.devcontainer/
grep '^DC_PROJECT=' $SANDBOX/.devcontainer/.env
grep VERSION $SANDBOX/.devcontainer/.configured-setup
test -x $SANDBOX/.devcontainer/initialize.sh && echo "exec ok"
```

Expected : ~90 files, `DC_PROJECT=dctest-v2-NNNN`,
`VERSION="2.0.0"`, exec bit set.

### 4. Open in VS Code → Reopen in Container

```bash
code $SANDBOX
```

Then Command Palette → "Dev Containers: Reopen in Container".

Watch the build log for :
- ✅ `initialize.sh` runs on host (builds `claude-devcontainer-base`
  if not already cached — ~5-8 min first time)
- ✅ `on-create.sh` → firewall init
- ✅ `post-create.sh` → symlink CLAUDE.md, smoke test
- ✅ `post-start.sh` → sync-creds (silent), sync-skills, firewall re-init

### 5. Inside the container, smoke-test

```bash
claude --version           # version pin matches CLAUDE_CODE_VERSION
which claude               # /usr/local/bin/claude
wtf foo                    # help or "not found" graceful message
cat /etc/claude-source     # failsafe variant marker
cat ~/.claude/settings.json | jq '.hooks'   # merged hooks present
ls ~/.claude/commands/     # skill commands copied
bash .devcontainer/test-firewall.sh   # firewall up
```

### 6. Test the v1.3 abort path (separate sandbox)

```bash
SANDBOX2=~/sandbox/dctest-v13-$(date +%s)
mkdir -p $SANDBOX2/.devcontainer
echo 'VERSION="1.3.0"' > $SANDBOX2/.devcontainer/.configured-setup
bash /path/to/devcontainer-tools/install.sh $SANDBOX2
# Expected : aborts with "Detected legacy v1 devcontainer" + Part 2 pointer
```

### 7. Cleanup

```bash
# After validating
docker compose -f $SANDBOX/.devcontainer/docker-compose.yml down
rm -rf $SANDBOX $SANDBOX2
# Optional : prune the test base image if it's a one-off VERSION
docker images | grep dctest-v2
docker rmi <tag>
```

## If something breaks (sandbox install)

- **Lifecycle log per phase** — `$SANDBOX/.devcontainer/logs/` (gitignored)
- **Base image build log** — `~/.devcontainer-build/<version>.log` or
  similar (host path depends on `initialize.sh`)
- **Firewall debug** — `bash $SANDBOX/.devcontainer/test-firewall.sh`,
  then inspect `mitmproxy-*` container logs
- **Container that won't start** — `docker compose logs` for clues

If `install.sh` itself blew up (rather than the container) — the
script uses `set -euo pipefail`, so any failure exits immediately
with the offending line in the trace.

## Rollback if THIS devcontainer's rebuild fails

Separate concern from the sandbox above : after commit `2bc0227`
(scrub project-specific identity from devcontainer baseline), the
expected runtime impact is **zero** — `.env` always wins over the
`${DC_PROJECT:-default}` fallback, and all other changes are docs /
archive / example files. But if a Rebuild Container on `/workspace`
fails anyway :

### Pre-flight check (optional, before rebuild)

```bash
grep -E '^DC_PROJECT=|^CLAUDE_CREDS_VOLUME=' /workspace/.devcontainer/.env
# Expected : DC_PROJECT=ragnarokonline-secured + CLAUDE_CREDS_VOLUME=claude-creds-shared-boa
# If missing → fix .env first, don't rebuild
```

### Rollback options (ranked safest first)

1. **Revert commit, keep history** (recommended) :
   ```bash
   cd /workspace
   git revert 2bc0227 --no-edit
   ```
   Creates a new commit undoing the 21 files. Working tree
   pre-existing changes (`firewall/domains.d/*.txt`, `log-proxy.txt`,
   `robrowser/minimap-debug.md`) stay intact. Re-rebuild after.

2. **Restore one specific file** (if you've narrowed down the
   culprit) :
   ```bash
   git checkout 27266d5 -- .devcontainer/<offending-file>
   ```
   Surgical — pulls just that file from the pre-commit state. Useful
   if `docker-compose.yml` or `initialize.sh` is the suspect.

3. **DO NOT** use `git reset --hard 27266d5` — destructive ; would
   also discard the pre-existing modifications in your working tree.

### Where to look when a rebuild fails

| Symptom | Where to look |
|---|---|
| `docker compose up` errors out before container starts | host terminal output (compose log direct to stderr) |
| Container starts then exits | `docker compose -f .devcontainer/docker-compose.yml logs` |
| `initialize.sh` (host hook) fails | re-run manually : `bash .devcontainer/initialize.sh` ; trace will surface the line |
| Lifecycle phase fails inside container | `.devcontainer/logs/<phase>-<ts>.log` (gitignored) — captures `on-create` / `post-create` / `post-start` |
| Firewall init fails | `bash .devcontainer/test-firewall.sh` from inside container ; inspect `mitmproxy-*` container logs |
| Image build fails | `.devcontainer/logs/build-base-<version>-<ts>.log` (verbose output of the `Dockerfile.base` build) |

### Expected non-impact of commit 2bc0227

Sanity check that should pass before AND after the commit (proves
the runtime is unchanged) :

```bash
# Container & volume names resolved from .env (not from the fallback)
docker compose -f /workspace/.devcontainer/docker-compose.yml config | grep -E 'name|claude-creds'
# Should show ragnarokonline-secured-* everywhere — same as pre-commit
```

If those names show `dc-project-*` instead, `.env` is NOT being read
— that's the real bug, fix `.env` first, not the rollback.

## Reporting back

Capture for the session-3 LOG.md entry :
- Sandbox path
- Wizard answers chosen
- Build timings (base image cold / warm, container start)
- Any lifecycle warnings
- Smoke-test pass/fail per command
- Verdict : ship v2 / fix-it commit needed

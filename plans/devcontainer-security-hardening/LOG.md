# Log — Devcontainer Security Hardening

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

## 1 — bake-firewall-config

**Date** : 2026-05-22

**Files touched** :
- `.devcontainer/SECURITY-AUDIT-2026-05.md` (NEW — committed in commit 1)
- `templates/v2/docker-compose.yml` (drop `./firewall:/etc/...:ro` bind mount)
- `templates/v2/Dockerfile`, `Dockerfile.php` (2 granular COPYs → 1 recursive `COPY firewall/`)
- `templates/v2/firewall/default-mode` (NEW, `strict\n`)
- `templates/v2/firewall/direct-tcp-allow.txt` (NEW, commented examples)
- `templates/v2/firewall/policy.local.d/.keep` (NEW placeholder dir)
- `templates/v2/firewall/ranges.d/` (DELETED — orphan, no consumer)
- `templates/v2/init-firewall.sh` (read `default-mode` + `direct-tcp-allow.txt` from baked path)
- `templates/v2/test-firewall.sh` (parallel updates so probes stay aligned)
- `templates/v2/post-start.sh`, `on-create.sh` (drop FIREWALL_MODE + CLAUDE_CODE_FIREWALL_ALLOWED env injection ; keep CLAUDE_CODE_FIREWALL_DEBUG)
- `templates/v2/firewall-mode.sh` (FLAG_FILE now `firewall/default-mode` + rebuild required wording)
- `templates/v2/shell-init.sh` (banner reads `/etc/devcontainer-firewall/default-mode`)
- `templates/v2/initialize.sh` (FW_FLAG repointed + touch/mkdir safety for new baked files + legacy migration)
- `templates/v2/.gitignore` (regroup ignores then negations + `firewall/policy.local.d/*` + `!firewall/policy.local.d/.keep`)
- `templates/v2/host-helpers/claude-switch` (sync_direct_tcp_allow helper, called from each mode)
- `templates/v2/tests/{lib.sh, run.sh, README.md, unit/test-bake-firewall.sh, integration/test-bake-firewall.sh, integration/test-firewall-modes.sh}` (NEW test framework)
- `install.sh` (copy new baked files + copy_dir policy.local.d + copy_dir tests + migrate_legacy_firewall function)
- `plans/devcontainer-security-hardening/TEST-PLAN.md` (NEW — multi-rebuild matrix)
- `.devcontainer/*` mirrors of all the above (incl. `.devcontainer/tests/`)

**What** : Removed the `./firewall:/etc/devcontainer-firewall:ro` bind mount and replaced it with a recursive `COPY firewall/` in the project Dockerfile. The whole firewall tree (rules, addons, dnsmasq.conf, policies, plus 2 new files `default-mode` + `direct-tcp-allow.txt`) is now baked into the image. Modifying any firewall config requires a rebuild. Migrated the firewall mode flag from workspace `.configured-firewall-mode` to baked `firewall/default-mode`, and the direct-TCP allowlist from `.env CLAUDE_CODE_FIREWALL_ALLOWED` (CSV) to baked `firewall/direct-tcp-allow.txt` (line-per-entry, `.env`-style conventions, `host` keyword for host.docker.internal). install.sh handles legacy migration idempotently.

**Why** : The `:ro` mount was cosmetic — the same host inodes were RW-accessible via the workspace mount. `node` (Claude potentially prompt-injected, or SSH-direct attacker) could modify `domains.local.txt`, `policy.d/`, `addons/*.py`, `dnsmasq.conf`, or write `off` to `.configured-firewall-mode` and have it picked up at the next reload — exfil-libre after a "Reload Window" gesture. Six 🔴 vectors collapsed under one root flaw (R0). Bake-only is the simplest fix that satisfies the threat model "node untrusted, host trusted, rebuild = conscious barrier".

**Decisions** :
- _Option A retained_ : keep `sudoers.d/node-firewall` entry for `init-firewall.sh` (re-confirmed with user before execution). Session 2 hardens the script itself by dropping `source /tmp/.firewall-env`. Option B (move init to root-native lifecycle) deferred — more invasive without security gain after session 2.
- _ranges.d/ removed_ : `bunnycdn.sh` + `github.sh` had 0 consumers in the code (orphan WIP). User-requested cleanup, included in commit 2.
- _.gitignore restructured_ : per user style — group all `xxx/` ignore patterns first, then all `!xxx/.keep` negations. Applied to skill scratch dirs + firewall section. Adds `!firewall/policy.local.d/.keep` to keep the placeholder dir tracked.
- _policy.local.d/.keep committed_ in both `templates/v2/` and `.devcontainer/` so the COPY recursive embarks the empty dir (gives projects a target for local overrides without manual mkdir).
- _SoT relocation_ : `.configured-firewall-mode` (workspace flag) is no longer read. New SoT = `firewall/default-mode` (baked). `firewall-mode.sh`, `shell-init.sh`, `initialize.sh`, `post-start.sh`, `on-create.sh`, `init-firewall.sh`, `test-firewall.sh` all updated. Migration in install.sh + initialize.sh (one-shot copy if `firewall/default-mode` empty).
- _test-firewall.sh adapted in lockstep_ : reads `default-mode` from baked + parses `direct-tcp-allow.txt` into a CSV for downstream `optin_port()` / TCP probe loop. Avoids invasive rewrite of the existing CSV-based logic.

**Gotchas** :
- `templates/v2/install.sh` does not exist — install.sh lives at the repo root (`/workspace/install.sh`). The session-1 spec assumed otherwise ; adjusted in the execution plan upfront.
- `policy.local.d/` was absent everywhere before this session (only `policy.local.d.example/` existed). Had to create the empty dir with `.keep` so `copy_dir` + recursive `COPY` find something to embark.
- `domains.d/` mentioned in the spec doesn't exist anywhere in the code — skipped (no consumer).
- `claude-switch` keeps naming `local` / `local-proxy` / `cloud` (current convention), not the spec's hypothetical `local-bridge` / `local-direct`. Behavior matches the spec : each mode writes the appropriate `host:port` to `direct-tcp-allow.txt`, cloud writes empty.

**Tests** :
- Added `templates/v2/tests/` framework : `lib.sh` (bash assertion helpers), `run.sh` (discovers `unit/` + `integration/`), `unit/test-bake-firewall.sh` (45 static assertions), `integration/test-bake-firewall.sh` (6 runtime checks for boundary + vectors #12/#13), `integration/test-firewall-modes.sh` (per-mode invariants).
- Mirrored to `.devcontainer/tests/`. `install.sh install_files()` adds `copy_dir tests` so adopting projects inherit the suite.
- Multi-rebuild matrix documented in `plans/devcontainer-security-hardening/TEST-PLAN.md` (1 rebuild for merge gate, 4 rebuilds for full mode coverage, claude-switch mini-matrix).
- Pre-rebuild run from inside the current (pre-bake) container : **45 pass / 0 fail / 9 skip** (skips = runtime tests that auto-skip when bind mount still active). Post-rebuild full validation deferred to user's first rebuild.

**Commit** : commit 1 = `7b9ff66 — docs(security): add SECURITY-AUDIT-2026-05 (13 vectors)`. Commit 2 = bake firewall (proposed, pending user confirm).

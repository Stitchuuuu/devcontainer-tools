# Log — Node 24 Bump

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

## 1 — bump-and-verify

**Date** : 2026-05-22
**Files touched** (10 code + 3 plan) :
- `.devcontainer/Dockerfile.base` (FROM + L117 comment)
- `templates/v2/Dockerfile.base` (mirror)
- `.devcontainer/host-helpers/verify-slim-base` (check 3 + check 7 recalibration)
- `templates/v2/host-helpers/verify-slim-base` (mirror)
- `.devcontainer/README.md` (L148)
- `templates/v2/README.md` (mirror)
- `.devcontainer/knowledge/docker-base-image.md` (L52 + L65)
- `templates/v2/knowledge/docker-base-image.md` (mirror)
- `templates/v2/Dockerfile.php` (Sury wire-up + comment)
- `plans/node-24-bump/ROLLOUT.md` (premise revision)
- `plans/node-24-bump/EXISTING.md` (post-bump snapshot)
- `plans/node-24-bump/STATUS.md` (delivered flip)

**What** : Bumped the v2.1 base image FROM tag to `node:24-bookworm-slim` (explicit pin), drove Node from 20.20.2 → 24.16.0 inside the devcontainer. Wired the Sury APT repo on the PHP variant as forward-compat (and for fresher point releases on bookworm — observed PHP 8.2.31 vs bookworm main's older patch). Recalibrated `verify-slim-base` so the 9-gate returns 9/9 PASS on the new image. Measured baseline : 1.08 GiB (unchanged from node:20 era — slim deltas absorb in the noise).

**Why** : Node 20 EOL was 2026-04-30, ~3 weeks before this rollout opened.

**Decisions** (chronological, with mid-rollout pivots) :
- *Initial premise* : ROLLOUT.md asserted a Debian distro jump bookworm → trixie / glibc 2.36 → 2.40 because we expected `node:24-slim` to ship trixie. **Premise turned out false**. Docker Hub still ships `node:24-slim` on bookworm today.
- *Sury wire-up kept (not reverted)* : Even though bookworm main has php8.2-*, Sury provides fresher patch releases (8.2.31 observed) and is forward-compat for an eventual trixie rebase. `lsb_release -sc` makes the block distro-agnostic. Comment rewrote with explicit future-proof framing.
- *Explicit base pin to `node:24-bookworm-slim`* : the floating `node:24-slim` would silently rebase to trixie someday and break our docs + assumptions. Pin makes a future jump a deliberate session. Cost : we miss silent Debian bug-fix rebases. Accepted.
- *verify-slim-base recalibration (×2 mirror)* : Check 3 — `du -sb /home/node` now excludes `.vscode-server`, restoring the 5 MiB cap's meaning as a `.npm` squat detector (the v2.1-1 VSIX bake was already putting ~233 MiB at that path, the test had always silently failed). Check 7 — package count threshold raised from `< 200` to `< 300` because `node:24-bookworm-slim` ships ~255 pkgs vs `node:20-slim` ~150. Same Debian base, just more Node 24 deps pre-installed by upstream.
- *Mirror enforcement* : every code edit landed dual-tree (`.devcontainer/` + `templates/v2/`). Five `diff` checks pass empty post-session.

**Gotchas** :
- *verify-slim-base CLI arg* : The script expects just the version (`2.1.145`), not the full tag (`claude-devcontainer-base:2.1.145`). Passing the full tag results in `claude-devcontainer-base:claude-devcontainer-base:2.1.145` and an "image not found" error. Fixed in our rebuild script after first run.
- *node:24-slim is still bookworm* : Original session spec wrong on this. Surfaced live in the rebuilt devcontainer via `node -v ; cat /etc/debian_version` returning `v24.16.0 ; 12.14`.
- *`/home/node` runtime vs image-baseline* : image alone = 232 MiB (baked VSIX). Running devcontainer fs = 342 MiB (VS Code adds runtime extensions like github.copilot-chat). Plus the VS Code DevContainers extension creates a separate persistent volume at `/vscode/vscode-server` containing ~5.8 GiB of cumulative cruft (8 old Claude VSIX versions cached: 2.1.19 → 2.1.119 totalling ~740 MiB + ~2.6 GiB of VS Code Server binaries). **Out of scope for this rollout** — cleanup is a follow-up session.
- *Dockerfile.php path stale in original session spec* : spec said `.devcontainer/Dockerfile.php` ; actual file is in `templates/v2/Dockerfile.php` (the dogfood is Node-only). Caught during initial exploration before any code edit.

**Tests** :
- 5 mirror `diff` checks (Dockerfile.base, verify-slim-base, README.md, RUNBOOK.md, docker-base-image.md) → all empty (PASS)
- Host rebuild #1 (initial bump) : `docker build --no-cache` PASS, MEASURED_SIZE=1.08 GiB
- Host rebuild #2 (bookworm-pin) : same MEASURED_SIZE, base content byte-equivalent (today)
- `verify-slim-base 2.1.145` → **9/9 PASS** post-recalibration (was 6 PASS / 1 FAIL / 2 INFO pre-recalibration with stale thresholds)
- Claude failsafe Scenario 1 (optimal) confirmed via `/etc/claude-source` → `extension:/home/node/.vscode-server/extensions/anthropic.claude-code-2.1.145-linux-arm64/resources/native-binary/claude`
- Third-party tools (mitmproxy 12.2.3, git 2.39.5, delta 0.18.2, gh 2.92.0, wtf) all working under Node 24
- PHP variant build via `templates/v2/Dockerfile.php` : PHP 8.2.31, Composer 2.9.8, 8/8 extensions present
- Native postinstalls : `sharp` + `bcrypt` install clean (`added 9 packages in 3s`) — glibc 2.36 ABI confirmed unchanged
- Live devcontainer post-rebuild : `node --version = v24.16.0`, `/etc/debian_version = 12.14`, glibc 2.36, `/etc/claude-source` = Scenario 1

**Commit** : *(proposed, not yet committed — awaiting user confirmation)*

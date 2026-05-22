# Runbook — DevContainer Niveau 1 strict

Operational procedures, step-by-step. Each section is a recipe: do these steps in this order. For background, read [README.md](README.md); for the threat model, [SECURITY.md](SECURITY.md); for internals, [knowledge/INDEX.md](knowledge/INDEX.md).

Conventions:
- `[host]` — run on the host machine (terminal outside the container)
- `[container]` — run inside the dev container (VS Code terminal or `docker exec`)
- When unspecified, default is `[container]`

## Table of contents

1. [Add a read-only domain](#1-add-a-read-only-domain)
2. [Add a POST on a third-party API](#2-add-a-post-on-a-third-party-api)
3. [Troubleshoot a blocked curl](#3-troubleshoot-a-blocked-curl)
4. [Switch firewall mode](#4-switch-firewall-mode)
5. [Routine cleanup](#5-routine-cleanup)
6. [Regenerate mitmproxy CA](#6-regenerate-mitmproxy-ca)
7. [Diagnose all-green](#7-diagnose-all-green)
8. [Reset auth or claude mode](#8-reset-auth-or-claude-mode)
9. [Force a fresh `/scan-deps`](#9-force-a-fresh-scan-deps)
10. [Reinstall VS Code extensions](#10-reinstall-vs-code-extensions)
11. [Inspect / rotate Claude OAuth credentials](#11-inspect--rotate-claude-oauth-credentials)
12. [Inspect the audit trail](#12-inspect-the-audit-trail)
13. [Quick commands reference](#13-quick-commands-reference)
14. [Bump Claude version](#14-bump-claude-version) — v2.1
15. [Troubleshoot Claude failsafe scenarios](#15-troubleshoot-claude-failsafe-scenarios) — v2.1
16. [Analyze base image breakdown](#16-analyze-base-image-breakdown) — v2.1
17. [Force rebuild base no-cache](#17-force-rebuild-base-no-cache) — v2.1
18. [Adopt the PHP variant](#18-adopt-the-php-variant) — v2.1-3
19. [Regen `extensions.json` if VS Code redownloads](#19-regen-extensionsjson-if-vs-code-redownloads) — v2.1

---

## 1. Add a read-only domain

You need to fetch a docs site, a static file, a registry the team doesn't already allow.

1. **Edit** `[host]` `.devcontainer/firewall/domains.local.txt`:
   ```
   docs.example.com                          # bare = GET only
   [GET] static.example.com                  # explicit
   [GET] api.example.com/v1/public           # path-restricted
   ```
2. **Recompile + reload** — two options:
   - **In-place** `[container]`: `sudo /usr/local/bin/init-firewall.sh` (reloads policy, no restart)
   - **Full rebuild** `[host]`: VS Code → `Dev Containers: Rebuild Container`
3. **Verify** `[container]`:
   ```bash
   curl -sI https://docs.example.com/ | head -1
   # → HTTP/2 200
   ```

If the curl still fails:
- Check `cat .devcontainer/.configured-firewall-mode` is `strict` or `basic`
- Check the host is in compiled output: `grep example.com /var/run/devcontainer-firewall/policy.compiled.yaml`
- Check the mitmproxy log: `tail -50 /var/log/mitmproxy.log`

For a POST host, **stop**. Use procedure 2 instead.

---

## 2. Add a POST on a third-party API

You cannot extend the main allowlist with new POST hosts. The threat model (see [SECURITY.md § POST allowlist](SECURITY.md#post-allowlist-the-exact-list)) limits main POST to 4 targets. Other POST requirements must run in an isolated research project.

1. **In Claude** `[container]`:
   ```
   /prepare-research stripe-payments-integration
   ```
   This writes a 5-file bundle under `.devcontainer/research-bundles/stripe-payments-integration/`.
2. **Review** `[host]` the bundle (all 5 files):
   ```bash
   cat .devcontainer/research-bundles/stripe-payments-integration/instructions.md
   cat .devcontainer/research-bundles/stripe-payments-integration/domains.local.txt
   cat .devcontainer/research-bundles/stripe-payments-integration/policy.local.d.example/*.yaml
   cat .devcontainer/research-bundles/stripe-payments-integration/files-to-copy.txt
   cat .devcontainer/research-bundles/stripe-payments-integration/secrets.env.template
   ```
3. **Spawn the research container** `[host]`:
   ```bash
   cp -r .devcontainer/research-bundles/stripe-payments-integration/ \
         ../stripe-payments-integration/
   cd ../stripe-payments-integration/
   # Fill secrets:
   nano .devcontainer/.env.local   # set STRIPE_API_KEY_TEST=sk_test_...
   code .                          # Reopen in Container
   ```
4. **Work in the research container** — Claude reads `INSTRUCTIONS.md`, writes output to `/output/`.
5. **Bring the result back** `[host]`:
   ```bash
   .devcontainer/host-helpers/bring-back-result stripe-payments-integration
   # → archives ../stripe-payments-integration/result/ into the main bundle
   ```
6. **Cleanup** `[host]` after >7 days (or sooner):
   ```bash
   .devcontainer/host-helpers/research-cleanup            # dry-run by default
   .devcontainer/host-helpers/research-cleanup --apply    # actually delete
   ```

Full flow + bundle anatomy: [RESEARCH.md](RESEARCH.md).

---

## 3. Troubleshoot a blocked curl

A request fails with timeout, REJECT, 503, or NXDOMAIN.

1. **Identify the mode** `[container]`:
   ```bash
   cat /workspace/.devcontainer/.configured-firewall-mode
   ```
   - `off` → no filter active, something else is wrong (typo, network down)
   - `basic` → only DNS allowlist active, no path/method filtering
   - `strict` (default) → full L1-L6 stack
2. **Check L1 (DNS allowlist)** `[container]`:
   ```bash
   dig +short example.com @127.0.0.53
   # → empty = NXDOMAIN = not in allowlist
   ```
   If empty: the host isn't allowed. Add it (procedure 1) or spawn research (procedure 2).
3. **Check L2-L6 (mitmproxy + addons)** `[container]`:
   ```bash
   grep example.com /var/run/devcontainer-firewall/policy.compiled.yaml
   ```
   If empty: same as above, not in policy.
4. **Inspect mitmproxy logs** `[container]`:
   ```bash
   sudo tail -50 /var/log/mitmproxy.log          # CONNECT events + errors
   sudo tail -50 /var/log/mitmproxy-writes.log   # POST/PUT/PATCH/DELETE audit
   ```
   Look for `403`, `503`, `path not allowed`, `method not allowed`, `body size`.
5. **Verbose request** `[container]`:
   ```bash
   curl -v https://example.com/ 2>&1 | head -40
   ```
   Reads the TLS chain — `mitmproxy` should appear if the proxy intercepts.
6. **Test the same URL with a known-good baseline**:
   ```bash
   curl -sI https://api.anthropic.com/ | head -3   # should 200/401
   ```
   If even this fails, mitmproxy itself is down — see procedure 6.

---

## 4. Switch firewall mode

1. **Flip the flag** `[host]`:
   ```bash
   bash .devcontainer/firewall-mode.sh strict   # default, secure max
   bash .devcontainer/firewall-mode.sh basic    # DNS-only, no L7
   bash .devcontainer/firewall-mode.sh off      # kill-switch (debug only)
   ```
   This edits `.configured-firewall-mode` AND `.env` (HTTPS_PROXY, CA env vars) consistently.
2. **Rebuild the container** `[host]`:
   - VS Code → `Dev Containers: Rebuild Container`
3. **Verify** `[container]` after reopen:
   ```bash
   cat /tmp/post-start.log | grep -i 'firewall mode'
   ```

Deprecated aliases still accepted with a stderr warn: `okeish` → `basic`, `paranoid` → `strict`. Update your muscle memory at your leisure.

---

## 5. Routine cleanup

Drafts, pending scripts, research projects, audit trails grow over time. Periodic cleanup:

```bash
# PR drafts older than 7 days
find .devcontainer/pr-drafts/ -mtime +7 -name "*.md" -delete
find .devcontainer/pr-drafts/ -mtime +7 -name "*.yaml" -delete

# /watch-log pending scripts (auto-cleaned at post-start, but for safety):
.devcontainer/host-helpers/watch-log-cleanup

# Research projects (sibling dirs) older than 7 days
.devcontainer/host-helpers/research-cleanup --apply

# Research bundles older than 7 days
find .devcontainer/research-bundles/ -mindepth 1 -maxdepth 1 -mtime +7 -type d -exec rm -rf {} +

# Scan-deps audit markdown older than 30 days
find .devcontainer/scan-deps/ -name "*.md" -mtime +30 -delete
```

Consider a cron / `launchd` job on the host if your project moves fast.

---

## 6. Regenerate mitmproxy CA

The CA cert lives in volume `mitmproxy-${DC_PROJECT}`. If the volume is corrupted (cert expired, file permissions broken) or you want to start fresh:

1. **Stop the container** `[host]`: VS Code → close the window or `docker compose -f .devcontainer/docker-compose.yml down`.
2. **Delete the volume** `[host]`:
   ```bash
   # find the project-specific volume
   docker volume ls | grep mitmproxy
   # output: <project>_mitmproxy-<project>
   docker volume rm <project>_mitmproxy-<project>
   ```
3. **Reopen in Container** `[host]`: VS Code → `Dev Containers: Reopen in Container`.
4. **CA regenerates** at first strict boot. `init-firewall.sh` calls `mitm-init.sh` which runs `mitmdump --certs` once if the volume is empty.
5. **Verify** `[container]`:
   ```bash
   curl -sI https://api.anthropic.com/ | head -1   # 401 or 200 → CA OK
   ```

The mitmproxy binary itself is baked into the image since A3, so this reset only affects the cert — no re-download.

---

## 7. Diagnose all-green

Before any commit that touches `.devcontainer/`, run the full test suite. **From the host**, not from inside the container (the script refuses with exit 2 if invoked in-container — it needs `docker exec` to drive the target).

```bash
bash .devcontainer/tests/diagnose.sh
```

~180 tests, sectioned by phase (Phase 1 / Phase 2 / A1.1 / A2 strict / A4 / A5 / B / C / D / F / F2 / E). Exit 0 = all green.

For debug iterations, capture per-mode snapshots:

```bash
bash .devcontainer/tests/diag-a2.sh        # writes tests/diag-a2-<mode>.log
bash .devcontainer/tests/diagnose.sh --verbose   # + state dumps inline
```

If a single test fails, the script prints the failing assertion and the relevant context. Re-run with `--verbose` to get the full mitmproxy log / ipset state / iptables rules.

---

## 8. Reset auth or claude mode

```bash
# Reset GitHub auth (standard vs advanced)
rm .devcontainer/.configured-auth
# → VS Code: Rebuild Container → initialize.sh re-prompts

# Reset Claude mode (dev vs reviewer)
rm .devcontainer/.configured-claude-mode
# → Rebuild → re-prompts; post-create.sh resymlinks /workspace/CLAUDE.md accordingly

# Re-run Claude first-prompt rules analysis
rm .devcontainer/.configured-claude-rules
# → next /init prompt re-analyses the project and updates CLAUDE.md

# Reset firewall mode (rewrites strict silently)
rm .devcontainer/.configured-firewall-mode
# → Rebuild → initialize.sh writes strict
```

---

## 9. Force a fresh `/scan-deps`

The boot banner uses `scan-deps/.last-scan.json` (per-manifest `ts` + `ignored_until`) to decide whether to nag. To force a fresh scan even if nothing changed:

```bash
rm .devcontainer/scan-deps/.last-scan.json
# In Claude:
/scan-deps
```

This re-runs both `extract-auto-dependencies` (bash, deterministic) and the AI review layer.

To **silence** the banner without scanning (e.g. you're sure the deps are fine, you'll deal with it later):

```bash
# Edit .devcontainer/scan-deps/.last-scan.json, set ignored_until to a future
# unix timestamp (e.g. now + 7 days):
jq '.ignored_until = (now + 7*86400)' .devcontainer/scan-deps/.last-scan.json \
   > /tmp/scan-deps.json && mv /tmp/scan-deps.json .devcontainer/scan-deps/.last-scan.json
```

---

## 10. Reinstall VS Code extensions

The post-start safety net normally handles missing extensions, but if you need to force:

```bash
bash .devcontainer/install-extensions.sh
```

Idempotent — extensions already installed are skipped. Reads pinned versions from `devcontainer.json` `customizations.vscode.extensions`. Useful after `vscode-server` corruption or after editing the extensions list.

To force a re-install (skip the skip-if-installed check), uninstall first:

```bash
code --list-extensions | grep -i claude
code --uninstall-extension anthropic.claude-code
bash .devcontainer/install-extensions.sh
```

---

## 11. Inspect / rotate Claude OAuth credentials

The `claude-creds` volume is shared across projects (`external: true`). Token is in `/home/node/.claude-creds/.credentials.json` (shared) and `/home/node/.claude/.credentials.json` (local copy). Sync logic in [knowledge/INDEX.md § Claude OAuth sync flow](knowledge/INDEX.md#claude-oauth-sync-flow).

```bash
# Inspect token expiry
jq -r '.claudeAiOauth.expiresAt / 1000 | todate' /home/node/.claude-creds/.credentials.json
jq -r '.claudeAiOauth.expiresAt / 1000 | todate' /home/node/.claude/.credentials.json

# Manual sync (decide which side wins)
DEBUG=1 .devcontainer/claude/sync-creds.sh

# Resolve a conflict the prompt flagged
rm /tmp/.claude-creds-conflict
cp /home/node/.claude-creds/.credentials.json /home/node/.claude/.credentials.json
chmod 600 /home/node/.claude/.credentials.json
```

To **fully revoke and re-auth** (if you suspect the token leaked):

1. Visit https://console.anthropic.com → API Keys / OAuth → revoke
2. `rm /home/node/.claude/.credentials.json /home/node/.claude-creds/.credentials.json`
3. `claude` → triggers OAuth device flow → re-paste the new token

---

## 12. Inspect the audit trail

What did the container POST today?

```bash
sudo tail -200 /var/log/mitmproxy-writes.log | jq -s '
   group_by(.host) | map({host: .[0].host, count: length, total_bytes: (map(.size) | add)})
'
```

Spot anomalies:

```bash
# Hosts outside the expected POST allowlist
sudo cat /var/log/mitmproxy-writes.log | jq -r '.host' | sort -u | \
  grep -v -E '^(api\.anthropic\.com|.*\.statsig\.com|sentry\.io|github\.com)$'
# → should be empty in main; non-empty = investigate
```

For research projects, the audit lives in the research container's own volume — separate audit trail per scope.

---

## 13. Quick commands reference

```bash
# Where am I? Which mode? Any pending overrides?
cat .devcontainer/.configured-firewall-mode
cat .devcontainer/.configured-claude-mode
ls -la .devcontainer/firewall/domains.local.txt 2>/dev/null
ls -la .devcontainer/firewall/policy.local.d/ 2>/dev/null

# Recompile policy without restarting
sudo /usr/local/bin/init-firewall.sh

# Show all allowed hosts after merge+overrides
sudo cat /var/run/devcontainer-firewall/policy.compiled.yaml | yq '.domains | keys'

# Show what overrides are active (machine-readable)
sudo yq '.runtime._overrides_applied' /var/run/devcontainer-firewall/policy.compiled.yaml

# Live tail mitmproxy
sudo tail -F /var/log/mitmproxy.log

# Replay post-start without rebuilding
bash .devcontainer/post-start.sh
```

---

## 14. Bump Claude version

The base image bakes Claude Code at build time, pinned by `CLAUDE_CODE_VERSION` in `.env`. A bump rebuilds only the Claude layer (~30s on arm64); all other layers stay cached.

1. **Edit the pin** `[host]`:
   ```bash
   # .devcontainer/.env
   CLAUDE_CODE_VERSION=2.1.146     # ← new version
   ```
2. **Also bump `devcontainer.json` extension pin** `[host]` (the JSONC comment in the file reminds you):
   ```jsonc
   "customizations": {
     "vscode": {
       "extensions": [
         // Pin = runtime fallback for the build-time VSIX bake.
         // Sync with CLAUDE_CODE_VERSION in .env.
         "anthropic.claude-code@2.1.146"
       ]
     }
   }
   ```
3. **Rebuild Container** `[host]`: VS Code → `Dev Containers: Rebuild Container`.
   - `initialize.sh build_base_if_missing()` detects the new tag is missing and rebuilds `Dockerfile.base` (~30s — only the Claude layer is invalidated).
   - Build log captured in `.devcontainer/logs/build-base-2.1.146-<ts>.log` (gitignored).
4. **Verify** `[container]`:
   ```bash
   claude --version                                # → 2.1.146
   cat /etc/claude-source                          # → extension:<path> (Scenario 1) ideally
   ls /etc/claude-fallback-warn 2>/dev/null && echo "FALLBACK" || echo "OK"
   ```
5. **Sanity check** `[host]` (optional):
   ```bash
   bash .devcontainer/host-helpers/verify-slim-base
   # → 9/9 OK if size/Claude pin/etc. still within bounds
   ```

If `/etc/claude-source` is now `npm-fallback (VSIX baked, Phase B path issue)` (Scenario 2), Anthropic moved the binary path inside the extension. See [procedure 15](#15-troubleshoot-claude-failsafe-scenarios).

`post-start.sh check_claude_update()` proactively queries `registry.npmjs.org/@anthropic-ai/claude-code` at boot and prints a yellow 1-line banner when a newer published version is available.

---

## 15. Troubleshoot Claude failsafe scenarios

You see the yellow loud banner at boot or `claude` isn't working. The build itself should ALWAYS succeed — if it doesn't, the issue is elsewhere (Docker daemon, disk space, firewall reaching dpkg/apt, etc.), not Claude.

### Step 1 — figure out which scenario fired

```bash
docker exec <ctr> cat /etc/claude-source
```

| Output | Scenario | Meaning |
|---|---|---|
| `extension:<path>` | 1 (optimal) | VSIX baked + Phase B symlink OK. Should NOT see the banner. |
| `npm-fallback (VSIX baked, Phase B path issue)` | 2 | VSIX baked but `claude --version` from ext binary didn't match the build ARG, or binary path moved. |
| `npm-fallback (no VSIX, runtime ext install via Marketplace)` | 3 | VSIX DL failed at build time. VS Code installs the extension at runtime via `devcontainer.json` pin. |

### Step 2 — sanity checks

```bash
# Phase B path probe — where does the symlink point?
docker exec <ctr> readlink -f /usr/local/bin/claude

# Extension dir presence + size
docker exec <ctr> ls -lah /home/node/.vscode-server/extensions/ | grep claude

# Sentinel
docker exec <ctr> ls /etc/claude-fallback-warn 2>/dev/null && echo "sentinel ON" || echo "sentinel OFF"

# claude itself
docker exec <ctr> claude --version
```

### Step 3 — fix path

| Symptom | Likely cause | Fix |
|---|---|---|
| Scenario 2, "version mismatch" in `.devcontainer/logs/build-base-*.log` | Ext binary in the VSIX is older than the published version | Try a different `CLAUDE_CODE_VERSION` (one minor older or newer). |
| Scenario 2, "$BIN missing/not executable" | Anthropic moved the binary inside the extension tree (was at `resources/native-binary/claude`) | `docker exec <ctr> find /home/node/.vscode-server/extensions/anthropic.claude-code-* -name claude -type f -executable 2>/dev/null` — find the new path, update `BIN=` in `Dockerfile.base` line ~149, rebuild base. |
| Scenario 3, but the VSIX should be available | Marketplace down OR version retired/replaced | Re-run the probe : `for P in linux-x64 linux-arm64; do curl -fsSL --compressed -A 'VSCode/devcontainer' --range 0-1023 -o /dev/null -w "$P %{http_code}\n" "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/anthropic/vsextensions/claude-code/${V}/vspackage?targetPlatform=${P}"; done` — if 404, the version is gone, pick another. |
| Scenarios 1 or 2 but `claude --version` fails inside the container | Symlink target gone (rare — only if someone rm'd the extension dir at runtime) | `docker exec <ctr> ls -la /usr/local/bin/claude` then re-symlink manually OR `BUILD_BASE_NO_CACHE=1 bash .devcontainer/initialize.sh` (see [procedure 17](#17-force-rebuild-base-no-cache)). |
| Build fails entirely with "Unsupported arch" | Running on something other than amd64/arm64 (e.g. armv7) | Add the arch to the case statement in BOTH RUN blocks of `Dockerfile.base`. |
| VS Code re-downloads Claude on first start (du shows 2 dirs) | `extensions.json` malformed → see [procedure 19](#19-regen-extensionsjson-if-vs-code-redownloads) |

### Step 4 — last-resort manual fixes (in-container, no rebuild)

- **Force Phase B manually** (Scenario 2 fix without rebuild) :
  ```bash
  docker exec -u root <ctr> ln -sf /home/node/.vscode-server/extensions/anthropic.claude-code-*/resources/native-binary/claude /usr/local/bin/claude
  docker exec -u root <ctr> rm /etc/claude-fallback-warn
  # Note: doesn't survive container rebuild — apply a proper Dockerfile.base fix for persistence.
  ```
- **Force runtime VSIX re-install** (Scenario 1 or 2 if `extensions.json` is corrupted) :
  ```bash
  docker exec <ctr> rm /home/node/.vscode-server/extensions/extensions.json
  docker exec <ctr> code --install-extension anthropic.claude-code@$(grep CLAUDE_CODE_VERSION .devcontainer/.env | cut -d= -f2) --force
  ```

---

## 16. Analyze base image breakdown

When the base image grows unexpectedly between bumps, `host-helpers/analyze-base-image` breaks it down per-layer + per-directory + per-package so you can spot the offender.

```bash
bash .devcontainer/host-helpers/analyze-base-image
```

The helper outputs three sections:

1. **`docker history claude-devcontainer-base:${V}`** — layer-by-layer size with the RUN/COPY/ARG/ENV that produced each layer
2. **`du -sh /<top-dirs>/`** — disk usage of `/usr`, `/var`, `/home`, `/opt`, `/tmp` inside the image (run via temporary `docker run --rm`)
3. **`dpkg-query -W -f='${Installed-Size}\t${Package}\n' | sort -nr | head -30`** — top 30 apt packages by installed size

Use cases :
- "Image went from 1.1 GB to 1.4 GB after I changed something" → run the helper before AND after, diff the per-layer column to localize the regression.
- "Where do the 240 MB of layer 7 come from?" → look at the per-dir section (`/home/node/.vscode-server` = VSIX baked, `/usr/local/bin/claude` = symlink target).
- "Did the chown trap come back?" → if you see two adjacent layers at 240+ MB each on the VSIX tree, it's the chown-in-separate-RUN duplication pattern (see [knowledge/INDEX § Modifications interdites / fragiles](knowledge/INDEX.md#modifications-interdites--fragiles)).

The helper is **host-only** (refuses to run inside the container — it needs `docker history` against the host daemon). Portable awk fallback works on macOS without `numfmt`.

---

## 17. Force rebuild base no-cache

Sometimes the layer cache lies — Anthropic re-publishes the same version with a fix, a Marketplace asset gets updated under the same URL, etc. Bypass the cache for the next base build:

```bash
# One-shot via env var
BUILD_BASE_NO_CACHE=1 bash .devcontainer/initialize.sh

# OR via .env (auto-consumed after one rebuild)
echo "BUILD_BASE_NO_CACHE=1" >> .devcontainer/.env
# Then: VS Code → Dev Containers: Rebuild Container
```

After the rebuild, the flag is auto-removed from `.env` so subsequent rebuilds use the cache normally.

`initialize.sh` also **auto-detects** `--build-no-cache` requests by walking the parent process ancestry (matches `devcontainer / docker / compose / buildkit / Code Helper` case-insensitively). When you click "Rebuild Container Without Cache" in VS Code, the flag propagates to the base build without needing the env var.

For debugging the auto-detection itself:
```bash
DEBUG_REBUILD_CONTEXT=1 bash .devcontainer/initialize.sh
# Dumps process tree + env to .devcontainer/logs/rebuild-context-<ts>.log (gitignored)
```

---

## 18. Adopt the PHP variant

For PHP projects, `.devcontainer/Dockerfile.php` provides a slim variant `FROM claude-devcontainer-base:${VERSION}` + PHP 8.2 + Composer 2 + 13 extensions. Adoption is per-project:

1. **Copy the variant into the target project** `[host]`:
   ```bash
   cp .devcontainer/Dockerfile.php <php-project>/.devcontainer/Dockerfile.php
   ```
2. **Point compose at it** `[host]` — edit `<php-project>/.devcontainer/docker-compose.yml`:
   ```yaml
   services:
     app:
       build:
         context: .
         dockerfile: Dockerfile.php           # ← was Dockerfile
   ```
3. **Rebuild Container** `[host]`: VS Code → `Dev Containers: Rebuild Container`.
4. **Smoke test** `[container]`:
   ```bash
   php --version                              # → PHP 8.2.x
   composer --version                         # → Composer 2.x
   php -r 'foreach (["curl","gd","mbstring","xml","zip","soap","intl","pdo_mysql","readline","bcmath","sockets","Phar"] as $e) echo $e . ": " . (extension_loaded($e) ? "OK" : "MISSING") . PHP_EOL;'
   ```

**Layer dedup happens automatically** through Docker's content-addressable cache: every PHP project sharing the same `FROM` + same `RUN apt install` reuses the PHP layer at ~150 MB delta (no per-project duplication on disk).

To **add other extensions one-off** for a specific PHP project, edit that project's local `Dockerfile.php` and append to the apt install. Only bump the shared `.devcontainer/Dockerfile.php` if ≥2 PHP projects need the same addition.

To **add a different variant** (Python ML, Go, Rust, etc.) → [knowledge/extension-points.md § Add a new Dockerfile variant](knowledge/extension-points.md#add-a-new-dockerfile-variant).

---

## 19. Regen `extensions.json` if VS Code redownloads

`Dockerfile.base` bakes `~/.vscode-server/extensions/extensions.json` with hardcoded UUIDs at build time so VS Code skips the Marketplace check at boot. If you see `du -sh /home/node/.vscode-server/extensions/` reporting two adjacent `anthropic.claude-code-*` dirs (one baked + one downloaded), the baked JSON drifted from VS Code's expected schema.

1. **Inspect the baked file** `[container]`:
   ```bash
   python3 -m json.tool /home/node/.vscode-server/extensions/extensions.json
   ```
   Expected single entry with `identifier.uuid=3c13ae49-babe-45fe-8c48-5e45077a62bf` and `metadata.publisherId=89769da0-cc4b-40b0-8216-93ffb5a96b56`.
2. **Compare to a fresh runtime install** `[container]`:
   ```bash
   # Force a clean re-install to see what VS Code writes naturally
   rm /home/node/.vscode-server/extensions/extensions.json
   code --install-extension anthropic.claude-code@$(grep CLAUDE_CODE_VERSION /workspace/.devcontainer/.env | cut -d= -f2) --force
   python3 -m json.tool /home/node/.vscode-server/extensions/extensions.json
   ```
3. **Diff** — if VS Code adds a new required field, port it to `Dockerfile.base` lines ~180-195 (`printf` with `%s` placeholders generating the JSON).
4. **Rebuild base** `[host]`:
   ```bash
   BUILD_BASE_NO_CACHE=1 bash .devcontainer/initialize.sh
   # → VS Code: Rebuild Container
   ```

Reference UUIDs (stable per-extension/per-publisher, do NOT change unless Anthropic republishes under a new publisher account):

```jsonc
{
  "identifier": {
    "id": "anthropic.claude-code",
    "uuid": "3c13ae49-babe-45fe-8c48-5e45077a62bf"
  },
  "metadata": {
    "publisherId": "89769da0-cc4b-40b0-8216-93ffb5a96b56",
    "publisherDisplayName": "Anthropic"
  }
}
```

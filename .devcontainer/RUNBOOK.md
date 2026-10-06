# Runbook — DevContainer on the published base image

Operational procedures, step-by-step. Each section is a recipe: do these steps in this order. For background, read [README.md](README.md); for the threat model, [SECURITY](docs/SECURITY.md); for the image's internals, `/opt/devcontainer/base/knowledge/INDEX.md`; for the image's own how-tos, `/opt/devcontainer/base/docs/`.

Conventions:
- `[host]` — run on the host machine (terminal outside the container)
- `[container]` — run inside the dev container (VS Code terminal or `docker exec`)
- When unspecified, default is `[container]`
- This tree runs the firewall in `basic` mode. Steps marked *(strict)* only apply when `firewall/default-mode` says `strict`.

## Table of contents

1. [Add a read-only domain](#1-add-a-read-only-domain)
2. [Add a POST on a third-party API](#2-add-a-post-on-a-third-party-api)
3. [Troubleshoot a blocked curl](#3-troubleshoot-a-blocked-curl)
4. [Switch firewall mode](#4-switch-firewall-mode)
5. [Routine cleanup](#5-routine-cleanup)
6. [Regenerate mitmproxy CA](#6-regenerate-mitmproxy-ca)
7. [Run the test suites](#7-run-the-test-suites)
8. [Reset the Claude mode](#8-reset-the-claude-mode)
9. [Reinstall VS Code extensions](#9-reinstall-vs-code-extensions)
10. [Inspect / rotate Claude OAuth credentials](#10-inspect--rotate-claude-oauth-credentials)
11. [Inspect the audit trail](#11-inspect-the-audit-trail)
12. [Quick commands reference](#12-quick-commands-reference)
13. [Bump Claude Code or the base image](#13-bump-claude-code-or-the-base-image)
14. [Update the extension patchers](#14-update-the-extension-patchers)

---

## 1. Add a read-only domain

You need to fetch a docs site, a static file, a registry the team doesn't already allow.

1. **Find the hostname** `[container]` — it is in the error message, not in a log: DNS refusals are silent by design.
   ```bash
   getent hosts docs.example.com      # empty = not allowed
   ```
2. **Edit** the file for the right audience:
   - `.devcontainer/firewall/domains.local.txt` — yours alone, or still experimenting (gitignored)
   - `.devcontainer/firewall/domains.d/<eco>.txt` — the team needs it too (committed, reviewed in PR)
   ```
   # what needed it — an entry nobody can explain is one nobody dares remove
   docs.example.com                          # bare = GET only
   [GET] static.example.com                  # explicit
   [GET] api.example.com/v1/public           # path-restricted (enforced in strict only)
   ```
3. **Apply** — the allowlist is baked into the image, so a restart changes nothing. Two options:
   - **Rebuild** `[host]`: VS Code → `Dev Containers: Rebuild Container`. Required for `domains.d/` and `domains.txt`; for `domains.local.txt` only if `FIREWALL_ALLOW_LOCAL_AT_REBUILD=1` is set in `.env`.
   - **Hot-reload the local layer** (basic mode only, `domains.local.txt` + `policy.local.d/`, lasts until the next container start):
     ```bash
     reload-firewall --dry-run               # [container] unprivileged preview of the diff
     wtf firewall reload                     # [host] from .devcontainer/ — docker exec -u 0 … /usr/local/bin/reload-firewall
     ```
     `reload-firewall` refuses in strict (the L7 policy would need reloading too) and is root-only by design — no NOPASSWD sudo for it, ever.
4. **Verify** `[container]`:
   ```bash
   getent hosts docs.example.com              # → an address
   curl -sI https://docs.example.com/ | head -1   # → HTTP/2 200
   boot-summary                               # Firewall row: count went up, "baked in" (not STAGED)
   ```

If the curl still fails:
- Check the mode in force: `cat /etc/devcontainer-firewall/default-mode` (baked) vs `cat .devcontainer/firewall/default-mode` (next rebuild)
- Check the host is in the compiled output: `grep example.com /var/run/devcontainer-firewall/policy.compiled.yaml`
- *(strict)* `firewall-blocks` for L7 refusals; `sudo tail -50 /var/log/mitmproxy.log`

Reference: `/opt/devcontainer/base/docs/how-to/allow-a-domain.md`.

---

## 2. Add a POST on a third-party API

The threat model (see [SECURITY § POST surface](docs/SECURITY.md#post-surface-as-declared-enforced-only-in-strict)) declares the POST allowlist — and in `basic`, the mode this tree runs, methods and paths are **not enforced at all**: an allowlisted host accepts every method on every path. So the question is not "how do I allow POST" but "do I accept this host at all, with what it can receive".

1. **Decide the scope.** A host that will receive data from the container is a host the whole team's audit surface includes. Prefer a `domains.d/<name>.txt` entry with a comment stating what is sent, reviewed in PR. Not `domains.local.txt`: a POST host nobody else sees is the exfiltration path the firewall exists to prevent.
2. **Declare it** `[host]` — `.devcontainer/firewall/domains.d/<name>.txt`:
   ```
   # payments integration — POST /v1/charges from scripts/billing.mjs
   [GET,POST] api.example.com
   ```
3. *(strict)* **Scope it** — `.devcontainer/firewall/policy.d/api.example.com.yaml`, so the L7 filter only lets the paths through that the integration needs:
   ```yaml
   endpoints:
     - path: "^/v1/charges$"
       methods: [POST]
       max_body_kb: 64
   ```
   Mind the parity rule for Anthropic-shaped targets (`/opt/devcontainer/base/knowledge/INDEX.md` § Policy parity).
4. **Rebuild** `[host]`: VS Code → `Dev Containers: Rebuild Container`.
5. **Verify** `[container]`: the request succeeds; *(strict)* `firewall-blocks` shows no refusal for the host, `sudo tail /var/log/mitmproxy-writes.log` shows the POST. Record the new host in [SECURITY § What this tree adds](docs/SECURITY.md#what-this-tree-adds-to-the-egress-surface).

---

## 3. Troubleshoot a blocked curl

A request fails with timeout, REJECT, 503, or NXDOMAIN.

1. **Identify the mode** `[container]`:
   ```bash
   cat /etc/devcontainer-firewall/default-mode
   ```
   - `off` → no filter active, something else is wrong (typo, network down)
   - `basic` (this tree) → only DNS allowlist active, no path/method filtering
   - `strict` → full L1-L6 stack
2. **Check L1 (DNS allowlist)** `[container]`:
   ```bash
   getent hosts example.com
   dig +short example.com @127.0.0.53
   # → empty = not in allowlist
   ```
   If empty: the host isn't allowed. Add it (procedure 1).
3. **Check the compiled policy** `[container]`:
   ```bash
   grep example.com /var/run/devcontainer-firewall/policy.compiled.yaml
   ```
   If empty: same as above, not in policy. If present but you edited `firewall/` since the last rebuild: the boot panel says `STAGED` — rebuild (procedure 1 step 3).
4. **Resolves but times out** → the service is not HTTP and needs a `host:port` entry in `firewall/ports.txt` (then Rebuild). `sudo /usr/local/bin/test-firewall.sh` probes every `ports.txt` entry.
5. *(strict)* **Check L2-L6 (mitmproxy + addons)** `[container]`:
   ```bash
   firewall-blocks                               # recent refusals: reason, host, path
   sudo tail -50 /var/log/mitmproxy.log          # CONNECT events + errors
   sudo tail -50 /var/log/mitmproxy-writes.log   # POST/PUT/PATCH/DELETE audit
   curl -v https://example.com/ 2>&1 | head -40  # mitmproxy should appear in the TLS chain
   ```
   Look for `403`, `503`, `path not allowed`, `method not allowed`, `body size`. A 403 from a host that resolves is a *path* decision: add a `policy.d/<host>.yaml`, not another hostname.
6. **Test the same URL with a known-good baseline**:
   ```bash
   curl -sI https://api.anthropic.com/ | head -3   # should 200/401
   ```
   If even this fails, the firewall itself is down — `sudo /usr/local/bin/init-firewall.sh` re-applies it (what `20-firewall-reinit` does at every start); *(strict)* see procedure 6.

---

## 4. Switch firewall mode

The mode is baked: `firewall/default-mode` is COPYed into the image as `/etc/devcontainer-firewall/default-mode` and read at boot.

1. **Set the flag** `[host]`:
   ```bash
   npx @meitogi/devcontainer-cli firewall-mode strict   # DNS + mitmproxy L7
   npx @meitogi/devcontainer-cli firewall-mode basic    # DNS-only, no L7 (this tree)
   npx @meitogi/devcontainer-cli firewall-mode off      # kill-switch (debug only)
   npx @meitogi/devcontainer-cli firewall-mode          # no argument: report
   ```
   This writes `firewall/default-mode` AND aligns the proxy/CA variables in `.env` (`HTTPS_PROXY`, CA env vars) with it. A bare `echo strict > firewall/default-mode` leaves `.env` saying the opposite — running the command for the mode already set is the repair for exactly that. `--dry-run` says what would change.
2. **Rebuild the container** `[host]`:
   - VS Code → `Dev Containers: Rebuild Container`
3. **Verify** `[container]` after reopen:
   ```bash
   cat /etc/devcontainer-firewall/default-mode
   grep -i 'firewall' .devcontainer/tmp/boot-summary.txt
   ```

Deprecated aliases still accepted with a stderr warn: `okeish` → `basic`, `paranoid` → `strict`.

---

## 5. Routine cleanup

Most of it is automatic now: `tmp/logs/` is rotated after 7 days (`05-log-rotation`), `tmp/pending/` after 60 min (`80-watch-log-cleanup`), and `rm -rf .devcontainer/tmp/` is always safe (it costs one patcher refetch). What is left:

```bash
# PR drafts older than 7 days (authored documents — never purged automatically)
find .devcontainer/pr-drafts/ -mtime +7 -name "*.md" -delete
find .devcontainer/pr-drafts/ -mtime +7 -name "*.yaml" -delete

# State snapshots written by claude/backup-state.sh (two retention rules apply on write;
# list what is there)
bash .devcontainer/claude/backup-state.sh --list

# [host] Docker: what is reclaimable, then reclaim (never touches named volumes)
wtf docker usage
wtf docker reclaim                      # bare = report only; read it, then pass the flag it suggests
bash .devcontainer/host-helpers/docker-test-images.sh   # tagged leftovers of this repo's test runs
```

---

## 6. Regenerate mitmproxy CA

*(strict)* The CA cert lives in volume `mitmproxy-${DC_PROJECT}`. If the volume is corrupted (cert expired, file permissions broken) or you want to start fresh:

1. **Stop the container** `[host]`: VS Code → close the window or `docker compose -f .devcontainer/docker-compose.yml down` (**without** `-v` — the other volumes hold Claude transcripts).
2. **Delete the volume** `[host]`:
   ```bash
   docker volume ls | grep mitmproxy
   docker volume rm mitmproxy-<project>
   ```
3. **Reopen in Container** `[host]`: VS Code → `Dev Containers: Reopen in Container`.
4. **CA regenerates** at first strict boot. The baked `init-firewall.sh` calls `mitm-init.sh` which runs `mitmdump` once to create the certs if the volume is empty.
5. **Verify** `[container]`:
   ```bash
   curl -sI https://api.anthropic.com/ | head -1   # 401 or 200 → CA OK
   ```

The mitmproxy binary itself is baked into the image, so this reset only affects the cert — no re-download. In `basic` the volume is mounted but empty; nothing to regenerate.

---

## 7. Run the test suites

Before any commit that touches `.devcontainer/`:

```bash
# [container] firewall smoke test — DNS allowlist, ports.txt probes, ollama.internal
sudo /usr/local/bin/test-firewall.sh

# [container] this tree's suites (integration/, runs as node)
bash .devcontainer/tests/run.sh
bash .devcontainer/tests/run.sh tests/integration/test-claude-switch.sh   # one file

# [container] after each claude-switch + rebuild — multi-rebuild orchestrator
bash .devcontainer/tests/validate-claude-switch.sh

# the base image's own suites, against the dind sidecar (both sides: container, then host)
wtf image test
```

The v2 firewall / bake / host-tier suites left with the local base image: the image repository runs them in its own `test/` as a strict superset. What stays here asserts this tree's own behaviour — the `claude-switch` pair.

---

## 8. Reset the Claude mode

```bash
# Reset Claude mode (dev vs reviewer)
rm .devcontainer/tmp/configured/claude-mode
# → Reopen / Rebuild → devc initialize re-prompts; 10-symlink-claude-mode resymlinks /workspace/CLAUDE.md

# Replay just the symlink without a rebuild
devc-hook post-create
```

While `claude-switch` is in `local` mode, `CLAUDE.md` points at `CLAUDE-local-dev.md` whatever the flag says — switch back to `cloud` first (README § Local backends). GitHub auth has no flag any more: `gh auth login` / `gh auth logout` directly.

---

## 9. Reinstall VS Code extensions

The `90-install-extensions-safety` fragment normally handles extensions that failed to download at first start, but if you need to force:

```bash
install-extensions
```

Idempotent — extensions already installed are skipped. Reads the list from `devcontainer.json` `customizations.vscode.extensions`. Useful after `vscode-server` corruption or after editing the extensions list.

To force a re-install (skip the skip-if-installed check), uninstall first:

```bash
code --list-extensions
code --uninstall-extension <publisher.name>
install-extensions
```

Do **not** add `anthropic.claude-code` to the list: the image bakes the (patched) extension, and a Marketplace pin is the one way an unpatched copy arrives. If the Claude extension itself is missing or duplicated, the `42-claude-ext-pin-warn` banner says so — the fix is a rebuild from a published tag (procedure 13).

---

## 10. Inspect / rotate Claude OAuth credentials

The `claude-creds` volume is shared across projects (`external: true`). Token is in `/home/node/.claude-creds/.credentials.json` (shared) and `/home/node/.claude/.credentials.json` (local copy). Sync logic: `/opt/devcontainer/base/knowledge/INDEX.md` § Claude OAuth sync flow.

```bash
# Inspect token expiry
jq -r '.claudeAiOauth.expiresAt / 1000 | todate' /home/node/.claude-creds/.credentials.json
jq -r '.claudeAiOauth.expiresAt / 1000 | todate' /home/node/.claude/.credentials.json

# Manual sync — decision details on stderr
DEBUG=1 sync-creds
VERBOSE=1 sync-creds

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

## 11. Inspect the audit trail

*(strict)* What did the container POST today?

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
# → should be empty; non-empty = investigate

# What was refused, and why
firewall-blocks 50
firewall-blocks --follow
```

In `basic` there is no mitmproxy, hence no write log and no blocks log: the only audit is the DNS allowlist itself (`/var/run/devcontainer-firewall/policy.compiled.yaml`) and `CLAUDE_CODE_FIREWALL_DEBUG=true` in `.env` for verbose iptables logging. This is the accepted trade-off described in [SECURITY](docs/SECURITY.md).

---

## 12. Quick commands reference

```bash
# Where am I? Which mode? Any pending overrides?
cat /etc/devcontainer-firewall/default-mode          # in force
cat .devcontainer/firewall/default-mode              # next rebuild
cat .devcontainer/tmp/configured/claude-mode
ls -la .devcontainer/firewall/domains.local.txt 2>/dev/null
ls -la .devcontainer/firewall/policy.local.d/ 2>/dev/null
cat .devcontainer/tmp/boot-summary.txt               # the cached boot panel (or: boot-summary)

# Re-apply the firewall without restarting (kernel-state guard skips if already up)
sudo /usr/local/bin/init-firewall.sh
sudo /usr/local/bin/test-firewall.sh

# Hot-reload the local layer (basic mode)
reload-firewall --dry-run                            # [container] preview
wtf firewall reload                                  # [host] from .devcontainer/

# Show all allowed hosts after merge+overrides
sudo cat /var/run/devcontainer-firewall/policy.compiled.yaml | yq '.domains | keys'

# Show what overrides are active (machine-readable)
sudo yq '.runtime._overrides_applied' /var/run/devcontainer-firewall/policy.compiled.yaml

# (strict) live tail mitmproxy / recent refusals
sudo tail -F /var/log/mitmproxy.log
firewall-blocks

# Lifecycle: what would run, replay a phase, read its log
devc-hook post-start --dry-run
devc-hook post-start
cat "$(ls -t .devcontainer/tmp/logs/post-start-*.log | head -1)"

# Claude Code binary, patchers, skills
claude --version && cat /etc/claude-source
ext-patches-sync --status
ls ~/.claude/commands/

# [host] mode, routing, images
npx @meitogi/devcontainer-cli firewall-mode
bash .devcontainer/host-helpers/claude-switch status
wtf docker usage
```

---

## 13. Bump Claude Code or the base image

The Claude Code version is part of the base image tag: `ghcr.io/meitogi/devcontainer-sandbox:<base-version>-cc<claude-code-version>`. Bumping Claude Code, bumping the base, and rolling either back are the same one-line edit. The pairs published together are listed in the image repository's `cc-versions.json`; the `45-claude-update-probe` banner ("Claude Code X available") is informational until a `-ccX` tag exists.

1. **Edit the pin** `[host]` — `.devcontainer/.env`:
   ```bash
   BASE_IMAGE=ghcr.io/meitogi/devcontainer-sandbox:1.9.1-cc2.1.280     # ← the tag you want
   ```
   Commented out = the default in `Dockerfile` / `docker-compose.yml`.
2. **Rebuild Container** `[host]`: VS Code → `Dev Containers: Rebuild Container`. Compose pulls the tag; only this project's thin layer (anim tools, docker CLI, rust, zig, xwin) is built, and its cache is invalidated by the new `FROM`.
3. **Verify** `[container]`:
   ```bash
   claude --version                                # → the cc<version> of the tag
   cat /etc/claude-source                          # → extension:<path> ideally
   ls /etc/claude-fallback-warn 2>/dev/null && echo "FALLBACK" || echo "OK"
   boot-summary                                    # Claude / Firewall / Patchers rows
   ext-patches-sync --status                       # patchers resolved for the new cc line? (procedure 14)
   ```
4. **Rollback** `[host]`: put the previous tag back in `.env`, Rebuild.

A yellow "npm fallback active" banner after the bump means the published image itself fell back at its build (the extension's embedded binary was not usable) — nothing in this tree fixes it; pick another published tag and report it to the image repository. There is no local base to rebuild, no cache to bypass: `docker pull` of the tag is the whole story.

---

## 14. Update the extension patchers

Patchers are applied to the baked VS Code extension at container create and re-checked at every start (`45-ext-patches`), from the tarball of `EXT_PATCHES_REPO` at `EXT_PATCHES_REF` (`.env`), merged with this tree's `claude/vscode-ext-patchs/`. `auto` (unset) resolves to the newest `cc<version>-r<n>` tag for the installed Claude Code; a boot never moves the pin on its own.

```bash
# What is configured, cached, and live in the extension on disk — reads only
ext-patches-sync --status          # = wtf ext-patch status

# What is installed vs available, then stop
wtf ext-patch update --check

# Move to the newest -r for this Claude Code line; rewrites EXT_PATCHES_REF in .env
wtf ext-patch update

# Pin a specific tag or SHA / replay the current ref from cache / trial run without moving the pin
wtf ext-patch update --ref cc2.1.280-r4
wtf ext-patch update --reapply
wtf ext-patch update --no-write-env
```

Then **Reload Window** in VS Code (a window reload, not a rebuild, is what reaches the extension host). A "Patchers — nothing cached for this line" banner after a Claude Code bump (procedure 13) means no tag exists yet for the new `cc` version: either wait for one, pin a tag with `--ref`, or set `EXT_PATCHES_ALLOW_UNTESTED=1` in `.env` to take the repository's HEAD. `EXT_PATCHES_TOKEN` must be a real read-only PAT — with the `<change-me>` placeholder the hook applies nothing and says so. How-to: `/opt/devcontainer/base/docs/how-to/patch-the-extension.md`.

# 20260703-1040 — Add `SCAN_DEPS_HOOK_DISABLED` env opt-out for scan-deps nudge

**Affects** : v2.1 devcontainers where the SessionStart / shell-init
scan-deps "stale allowlist" nudge (loud yellow banner in post-start,
quieter one on every new shell) is noisy and no opt-out exists.

**Symptom** : project isn't tracking npm/composer manifests for
firewall extraction yet, but the nudge fires every container boot +
every new terminal. No env var to silence it.

**Fix** : gate all three call sites (post-start.sh, shell-init.sh, the
scan-deps-suggest-stale.sh script itself) on
`SCAN_DEPS_HOOK_DISABLED` — any non-empty value silences the nudge for
the whole session. `/scan-deps` remains invokable on demand. Document
the opt-out in `.env.example` next to other opt-outs.

## Manual how-to

Four files per side (dogfood + template) → 8 edits total. All edits
are single-line guards or an `.env.example` insertion.

### File 1 — `.devcontainer/.env.example`

Find the section that ends with :

```
#NOTIFY_CLEANUP_MAX_AGE_HOURS=24
```

After the two blank lines that follow, and **before** the next section
header (`# === Firewall ===...`), insert :

```
# === scan-deps ===============================================================
# SessionStart hook (.devcontainer/skills/scan-deps/scan-deps-suggest-stale.sh)
# injects a banner when project manifests are newer than firewall/domains.d/<eco>.txt,
# nudging Claude to propose /scan-deps. Uncomment to silence the nudge for the
# whole session — the /scan-deps skill remains invokable on demand.
#SCAN_DEPS_HOOK_DISABLED=1


```

(Keep the two trailing blank lines so the next `=== Firewall ===` block
stays visually separated.)

### File 2 — `.devcontainer/post-start.sh`

Around line 168 (right after the `check_claude_update` invocation, in
the "stale allowlist" comment block), find :

```bash
if command -v python3 >/dev/null 2>&1; then
  python3 - <<'PY' || true
```

Change the `if` line to :

```bash
if [ -z "${SCAN_DEPS_HOOK_DISABLED:-}" ] && command -v python3 >/dev/null 2>&1; then
```

### File 3 — `.devcontainer/shell-init.sh`

Around line 154 (inside the `if [[ $- == *i* ]]; then` interactive
guard, in the "quieter" stale-check block), find :

```bash
  if command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY' 2>/dev/null || true
```

Change the `if` line to :

```bash
  if [ -z "${SCAN_DEPS_HOOK_DISABLED:-}" ] && command -v python3 >/dev/null 2>&1; then
```

### File 4 — `.devcontainer/skills/scan-deps/scan-deps-suggest-stale.sh`

After the `set -uo pipefail` line (~line 25) and **before** the
`command -v python3 …` line, insert :

```bash

# Opt-out via .devcontainer/.env (docker-compose loads it as env_file) — any
# non-empty value silences the nudge ; /scan-deps stays invokable on demand.
[ -n "${SCAN_DEPS_HOOK_DISABLED:-}" ] && exit 0
```

### Files 5-8 — `templates/v2/` mirror

Apply the **exact same 4 edits** to :

- `templates/v2/.env.example` (same section insertion)
- `templates/v2/post-start.sh` (same one-liner guard change)
- `templates/v2/shell-init.sh` (same one-liner guard change)
- `templates/v2/skills/scan-deps/scan-deps-suggest-stale.sh` (same insertion after `set -uo pipefail`)

### Commit

`````bash
git add .devcontainer/.env.example .devcontainer/post-start.sh \
  .devcontainer/shell-init.sh \
  .devcontainer/skills/scan-deps/scan-deps-suggest-stale.sh \
  templates/v2/.env.example templates/v2/post-start.sh \
  templates/v2/shell-init.sh \
  templates/v2/skills/scan-deps/scan-deps-suggest-stale.sh

git commit -m "feat(scan-deps): SCAN_DEPS_HOOK_DISABLED env opt-out for stale nudge"
`````

## Verify

- [ ] `grep -c SCAN_DEPS_HOOK_DISABLED .devcontainer/.env.example templates/v2/.env.example`
      → 1 each.
- [ ] `grep -c 'SCAN_DEPS_HOOK_DISABLED:-' .devcontainer/post-start.sh .devcontainer/shell-init.sh templates/v2/post-start.sh templates/v2/shell-init.sh`
      → 1 each.
- [ ] `grep -A1 'set -uo pipefail' .devcontainer/skills/scan-deps/scan-deps-suggest-stale.sh | grep -c SCAN_DEPS_HOOK_DISABLED`
      → 1.
- [ ] Live check : set `SCAN_DEPS_HOOK_DISABLED=1` in `.devcontainer/.env`,
      rebuild the container. On next boot no stale-allowlist banner in
      post-start ; new terminals also silent. Comment the line back →
      banner returns.

## Rollback

Revert the 4 guard changes + `.env.example` insertion on both sides.

`````bash
git revert <commit-hash>
`````

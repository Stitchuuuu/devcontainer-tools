# Upgrading a v2.x devcontainer to a newer v2.y

> Routine, minor-bump upgrade within the v2 line (e.g. 2.0.0 →
> 2.0.1 → 2.1.0). **Driven by Claude in human-in-the-loop mode**
> — no script.
>
> For the one-shot 1.x → 2.0 migration, use
> [MIGRATION-v1-to-v2.md](MIGRATION-v1-to-v2.md) instead.
>
> For **single targeted fixes between version bumps**, see the
> [Targeted updates](#targeted-updates-updatesname) section below
> — much lighter, no rebuild, just `git apply` + `git commit`.

---

## Targeted updates (`updates/<name>/`)

Between full version bumps, individual fixes ship as **per-update
patches** under `updates/<YYYYMMDD-HHMM>-<title>/`. Each entry is a
self-contained folder with exactly two files :

- `update.patch` — a `git format-patch`-style diff that applies
  cleanly against the previously-shipped state of the affected
  files.
- `update.md` — context (`Affects` / `Symptom` / `Cause` /
  `Upstream commits`), an `Apply` block, a `Verify` checklist, and
  a `Rollback` block.

The `Apply` and `Rollback` blocks contain **flat-pasteable**
`git apply` + `git commit` commands : bare commands separated by
blank lines, paths inlined, no `set -euo pipefail`, no `# …` step
comments, no leading `VAR=…` assignments — so each line can be
pasted directly into an interactive zsh prompt without wrapping
it in `bash <<EOF`. The rule lives in
[.devcontainer/LESSONS.md](.devcontainer/LESSONS.md) ("updates/*/update.md
bash blocks must be flat-pasteable"). Canonical reference :
[updates/20260613-0929-plus-button-chrome/update.md](updates/20260613-0929-plus-button-chrome/update.md).

**Bootstrap** (one-time per project — sparse-checkout, only ~1 MB
vs ~30 MB for the full release clone) :

```bash
cd /path/to/v2x-project
git clone --depth 1 --filter=blob:none --sparse \
  git@github.com:<user>/devcontainer-tools.git .tmp/devcontainer-updates
git -C .tmp/devcontainer-updates sparse-checkout set updates
grep -qxF '.tmp/' .gitignore 2>/dev/null || echo ".tmp/" >> .gitignore
```

**Apply** one update :

```bash
$EDITOR .tmp/devcontainer-updates/updates/<update-name>/update.md
```

…read the `Apply` block, paste it into your shell. The block
already knows where to find its `update.patch` — every recipe
references `.tmp/devcontainer-updates/updates/<name>/update.patch`.

**Refresh** between fixes :

```bash
git -C .tmp/devcontainer-updates pull --ff-only
```

**Cleanup** when done :

```bash
rm -rf .tmp/devcontainer-updates
```

The full release upgrade (Steps 0–11 below) folds in every patch
shipped to date, so per-update is for when you want a fix **now**
without waiting for the next version bump — or when you only want
that one specific fix and not the rest of the release delta.

---

## Bootstrap (humain, 2 commandes)

From inside the project to upgrade :

```bash
cd /path/to/v2x-project

# 1. Get the target v2 release alongside the project
git clone --depth 1 --branch v2.Y.Z \
  git@github.com:<user>/devcontainer-tools.git .tmp/upgrade-v2

# Variante locale (si déjà cloné sur l'host) :
# mkdir -p .tmp && cp -r /path/to/devcontainer-tools .tmp/upgrade-v2

# 2. Add the staging dir to .gitignore (idempotent)
grep -qxF '.tmp/' .gitignore 2>/dev/null || echo ".tmp/" >> .gitignore

# 3. Open Claude Code in this project and paste the prompt below.
```

**Prompt à coller dans Claude** :

> Upgrade ce projet vers la version `v2.Y.Z` de devcontainer-tools
> en suivant `.tmp/upgrade-v2/UPGRADE-v2.md`. Source de vérité = ce
> dossier `.tmp/upgrade-v2/`. Procède en écrasant uniquement les
> fichiers shipped, ne touche jamais aux compagnons `*.local`, et
> lance le firewall en mode `basic` AVANT toute autre modif.

Tout ce qui suit est lu par Claude.

---

## Instructions pour Claude

You are driving a minor-bump upgrade within the v2 line. The
delta is small : v2's `.local` convention isolates user-specific
config so the upgrade is mechanically simple — overwrite shipped
files, leave `.local` companions alone.

**Source of truth** : `.tmp/upgrade-v2/` is the cloned target
release. Baseline at `.tmp/upgrade-v2/templates/v2/`. Authoritative
file list at `.tmp/upgrade-v2/install.sh::install_files()`.

**Operating mode** : same as the migration playbook — per-class,
human-in-the-loop, confirm before destruction. But this upgrade
is much lighter : no drops, no relocations, mostly just
overwrites.

---

## Step 0 — Detect current version and target

Read the current marker :

```bash
cat .devcontainer/.configured-setup
```

- `VERSION="1.x"` → **abort**. Use
  [MIGRATION-v1-to-v2.md](MIGRATION-v1-to-v2.md) instead.
- `VERSION="2.x"` → proceed. Capture the current version.

Read the target version :

```bash
grep '^TEMPLATE_VERSION=' .tmp/upgrade-v2/install.sh | head -1
```

Confirm with the user :

> Upgrading from `2.x` to `2.y`. Proceed?

---

## Step 1 — Read the CHANGELOG between source and target

```bash
sed -n '/^## 2\.y\.z/,/^## /p' .tmp/upgrade-v2/CHANGELOG.md
```

(Adjust the regex to span every release between the user's current
version and the target.)

Surface to the user :

- **Breaking changes** : any "Breaking" or "Drops" section in the
  CHANGELOG → call out explicitly, ask for confirmation that the
  affected files / behaviours are not being relied upon.
- **New shipped files** : add them to the overwrite list in
  Step 3.
- **Removed shipped files** : add them to a drop list.
- **Renames** : handle as a delete-old + add-new pair, preserving
  any user customisations the user wants to port (rare in minor
  bumps, but possible).

If the CHANGELOG between the two versions has no breaking
changes, say so and continue.

---

## Step 2 — Switch firewall to `basic` mode

Same justification as the migration : `strict` mode may block
operations during the transition (especially if new
`domains.txt` / `policy.d/` shipped data changed and your
`.local` overlays haven't caught up).

Inside the container :

```bash
./firewall-mode.sh basic
```

If the container is stopped, the flip waits until after the
rebuild in Step 6. Either is fine, just be explicit.

---

## Step 3 — Overwrite shipped files

The shipped classes — identical to the migration playbook
Step 3, modulo any additions / removals surfaced in Step 1.

Run the overwrites, showing the user each `cp -v` line. For
directories, the **safest** pattern is `rm -rf` then `cp -r` to
avoid stale files left behind :

```bash
SRC=.tmp/upgrade-v2/templates/v2
DST=.devcontainer

# Lifecycle scripts (6)
for f in initialize on-create post-create post-start shell-init install-extensions; do
  cp -v "$SRC/${f}.sh" "$DST/${f}.sh"
done

# Firewall scripts (3)
for f in init-firewall firewall-mode test-firewall; do
  cp -v "$SRC/${f}.sh" "$DST/${f}.sh"
done

# Firewall shipped data (8 files)
for f in dnsmasq.conf compile-policy.py mitm-init.sh domains.txt \
         domains.local.txt.example firewall-blocks \
         default-mode direct-tcp-allow.txt; do
  cp -v "$SRC/firewall/$f" "$DST/firewall/$f"
done

# Firewall shipped dirs (3 — NOT policy.local.d/!)
for d in addons policy.d policy.local.d.example tests; do
  rm -rf "$DST/firewall/$d" && cp -rv "$SRC/firewall/$d" "$DST/firewall/$d"
done

# Claude config
rm -rf "$DST/claude" && cp -rv "$SRC/claude" "$DST/claude"

# Knowledge
rm -rf "$DST/knowledge" && cp -rv "$SRC/knowledge" "$DST/knowledge"

# Docs (4)
for f in README RUNBOOK SECURITY RESEARCH; do
  cp -v "$SRC/${f}.md" "$DST/${f}.md"
done

# Sidecar
rm -rf "$DST/claude-bridge" && cp -rv "$SRC/claude-bridge" "$DST/claude-bridge"
rm -rf "$DST/host-helpers" && cp -rv "$SRC/host-helpers" "$DST/host-helpers"
cp -v  "$SRC/diag-ollama-local.sh" "$DST/diag-ollama-local.sh"

# Generic skills (5) + sync helper
cp -v "$SRC/skills/sync-skills.sh" "$DST/skills/sync-skills.sh"
for s in prepare-pr watch-log prepare-research scan-deps prepare-plan; do
  rm -rf "$DST/skills/$s" && cp -rv "$SRC/skills/$s" "$DST/skills/$s"
done

# Tests
rm -rf "$DST/tests" && cp -rv "$SRC/tests" "$DST/tests"

# VS Code shipped settings
cp -v "$SRC/vscode-settings.json" "$DST/vscode-settings.json"

# Shipped .devcontainer/.gitignore (internal-scope rules)
cp -v "$SRC/.gitignore" "$DST/.gitignore"
```

**Confirm and continue.**

---

## Step 4 — Dockerfiles (diff + confirm)

`Dockerfile`, `Dockerfile.base`, `Dockerfile.php` are shipped, but
this is the one class users sometimes customise. Diff before
overwriting :

```bash
SRC=.tmp/upgrade-v2/templates/v2
DST=.devcontainer

for f in Dockerfile Dockerfile.base Dockerfile.php; do
  if [ -f "$DST/$f" ]; then
    echo "=== diff $f ==="
    diff "$SRC/$f" "$DST/$f" || true
  fi
done
```

For each non-empty diff :

- If the lines after `FROM <base>` in `Dockerfile` /
  `Dockerfile.php` contain user customisations (extra `RUN`,
  custom `COPY`, etc.), **stop and ask the user** before
  overwriting. Then either :
  - Overwrite and re-apply the customisations, or
  - Keep the current file and manually apply only the shipped
    delta.
- `Dockerfile.base` is rarely customised — usually safe to
  overwrite. Still confirm.

---

## Step 5 — Local files — DO NOT TOUCH

Verify these were not modified by Step 3 / 4 :

```bash
# Each of these must show the same mtime / content as before
ls -la .devcontainer/.env
ls -la .devcontainer/firewall/domains.local.txt 2>/dev/null
ls -la .devcontainer/firewall/policy.local.d/ 2>/dev/null
ls -d  .devcontainer/skills/*.local 2>/dev/null
find .devcontainer/skills -name '*.local.skill.md' 2>/dev/null
ls -la .devcontainer/LESSONS.local.md 2>/dev/null
```

**Local files protected from overwrite** :

| File / Dir | Why protected |
|---|---|
| `.devcontainer/.env` | User-edited values (`DC_PROJECT`, `CLAUDE_CREDS_VOLUME`, secrets). |
| `.devcontainer/.configured-setup` | Bumped, not overwritten (Step 7). |
| `.devcontainer/firewall/domains.local.txt` | User allowlist, gitignored. |
| `.devcontainer/firewall/policy.local.d/*.toml` | User L7 policies, gitignored. |
| `.devcontainer/skills/*.local*` | User per-machine skill variants. |
| `.devcontainer/LESSONS.local.md` | User notes, gitignored. |

If any of these were inadvertently touched, restore from
`git` before proceeding.

---

## Step 6 — Root `.gitignore` fragment (idempotent append)

The root-scope fragment may have been extended in the target
version. Re-run the idempotent append :

```bash
cd ..  # at project root
SENTINEL="$(head -n1 .tmp/upgrade-v2/templates/v2/.gitignore-root)"
if ! grep -qxF "$SENTINEL" .gitignore 2>/dev/null; then
  [ -s .gitignore ] && echo "" >> .gitignore
  cat .tmp/upgrade-v2/templates/v2/.gitignore-root >> .gitignore
fi
cd - >/dev/null
```

If the sentinel matches but new entries were added in the target
release, the user may need to merge by hand — show them the diff
between the existing applied fragment and the target one.

---

## Step 7 — Bump the marker

Edit only the `VERSION` line :

```bash
TARGET=$(grep '^TEMPLATE_VERSION=' .tmp/upgrade-v2/install.sh \
           | head -1 | cut -d= -f2 | tr -d '"')
sed -i.bak -E "s|^VERSION=.*|VERSION=\"$TARGET\"|" \
  .devcontainer/.configured-setup
rm .devcontainer/.configured-setup.bak
```

Other fields (`PROJECT_ID`, `PROJECT_TYPE`, `CLAUDE_CREDS_VOLUME`)
stay untouched.

---

## Step 8 — Restore exec permissions

```bash
cd .devcontainer
chmod +x initialize.sh on-create.sh post-create.sh post-start.sh \
         shell-init.sh install-extensions.sh \
         init-firewall.sh firewall-mode.sh test-firewall.sh \
         diag-ollama-local.sh \
         firewall/mitm-init.sh firewall/firewall-blocks \
         firewall/compile-policy.py
chmod +x claude/sync-creds.sh 2>/dev/null
chmod +x host-helpers/*.sh 2>/dev/null
chmod +x skills/sync-skills.sh 2>/dev/null
cd ..
```

---

## Step 9 — Rebuild and smoke test

1. *Dev Containers : Rebuild Container*.
2. After build :

   ```bash
   tail -100 /tmp/post-start.log
   ```

   Look for : firewall active, creds sync OK, skills sync OK.

3. Run your project's normal smoke (dev server, tests, etc.).

If smoke fails in `basic`, investigate (likely env / new
shipped behaviour). Don't proceed to strict until basic is green.

---

## Step 10 — Switch firewall back to `strict`

```bash
./firewall-mode.sh strict
```

If `strict` newly blocks something that `basic` allowed, the
target release may ship updated `domains.txt` / `policy.d/`
defaults — extend `domains.local.txt` / `policy.local.d/` as
needed. Iterate until smoke is green in strict.

---

## Step 11 — Cleanup

```bash
rm -rf .tmp/upgrade-v2
```

**Upgrade complete.** Commit :

```
Upgrade devcontainer from v2.x to v2.y.z

(Summarise the breaking-light notes from the CHANGELOG entries
between source and target.)
```

---

## Authoritative reference

The exact list of files v2 ships is encoded in
`.tmp/upgrade-v2/install.sh::install_files()` — if any class in
this playbook diverges from that function, **trust the function,
not the playbook**, and please report the divergence.

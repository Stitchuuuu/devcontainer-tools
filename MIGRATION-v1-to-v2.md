# Migrating a v1.x devcontainer to v2.0

> One-shot, breaking migration from `devcontainer-tools` v1.x
> (typically 1.3) to v2.0.0. **Driven by Claude in human-in-the-loop
> mode** — no script.
>
> For routine v2.x → v2.y bumps, use [UPGRADE-v2.md](UPGRADE-v2.md)
> instead.

---

## Bootstrap (humain, 2 commandes)

From inside the project to migrate :

```bash
cd /path/to/v1x-project

# 1. Get v2.0.0 alongside the project (private SSH clone)
git clone --depth 1 --branch v2.0.0 \
  git@github.com:<user>/devcontainer-tools.git .tmp/migrate-v2

# Variante locale, si le repo est déjà cloné sur l'host :
# mkdir -p .tmp && cp -r /path/to/devcontainer-tools .tmp/migrate-v2

# 2. Add the staging dir to .gitignore so it never gets committed
echo ".tmp/" >> .gitignore

# 3. Open Claude Code in this project and paste the prompt below.
```

**Prompt à coller dans Claude** :

> Migre ce projet d'une devcontainer v1.x vers v2.0 en suivant
> `.tmp/migrate-v2/MIGRATION-v1-to-v2.md`. Source de vérité = ce
> dossier `.tmp/migrate-v2/`. Procède par classes de fichiers,
> demande confirmation avant chaque opération destructive, et lance
> le firewall en mode `basic` AVANT toute autre modif.

Tout ce qui suit est lu par Claude.

---

## Instructions pour Claude

You are driving a one-shot migration of an existing
`.devcontainer/` (v1.x, typically 1.3) to the v2.0.0 baseline.

**Source of truth** : `.tmp/migrate-v2/` is the cloned v2.0.0 repo.
The canonical baseline is `.tmp/migrate-v2/templates/v2/`. The
installer that produces a fresh v2 install is
`.tmp/migrate-v2/install.sh` — **do not run it** (it would refuse
on a v1.x marker, and even past that refusal it is built for fresh
installs, not migrations).

**Operating mode** :

- Work per-class, in the order below. Each class section ends with
  a "Confirm and continue" pause.
- Before any destructive op (`rm`, `git rm`, overwrite of a file
  the user may have customised), **state what you will do and
  wait for the user to confirm**.
- If the user has obvious customisations in a shipped file
  (e.g. extra `RUN` steps in `Dockerfile`, extra entries in
  `domains.txt`), call them out and propose options (keep, port
  into `.local` companion, drop). Never silently absorb.
- After each step, verify locally with `ls` / `cat` / `diff` —
  show evidence, not assertions.

---

## Step 0 — Detect the current version

Read `.devcontainer/.configured-setup` :

```bash
cat .devcontainer/.configured-setup 2>/dev/null
```

- `VERSION="2.x"` → **abort**. This project is already on v2.
  Use [UPGRADE-v2.md](UPGRADE-v2.md).
- `VERSION="1.x"` (1.0 / 1.1 / 1.2 / 1.3) → proceed.
- File missing → legacy pre-1.0. The migration still works but
  more fields will be missing (no `PROJECT_ID` recorded, no
  `SHARED_CREDS_VOLUME`, etc.). Ask the user for the values that
  would have been in the marker before continuing (PROJECT_ID,
  DISPLAY_NAME, PROJECT_TYPE, TIMEZONE).

Capture the values to a scratchpad (you'll need them in Step 8
when writing the v2 marker).

---

## Step 1 — Switch firewall to `basic` mode (mandatory, FIRST)

Rationale : the v2 `strict` firewall (mitmproxy + L7 policies)
would block legitimate operations during the migration —
`npm install`, base image pull, fetches from Ollama registry,
etc. — while `domains.local.txt` and `policy.local.d/` are not
yet calibrated. Basic mode keeps the DNS allowlist active (not
open-bar) but drops mitmproxy enforcement.

```bash
# Bring in the v2 firewall-mode.sh helper (v1.x didn't have it)
cp .tmp/migrate-v2/templates/v2/firewall-mode.sh \
   .devcontainer/firewall-mode.sh
chmod +x .devcontainer/firewall-mode.sh

# Verify it parses (do not exec from host — it expects to run in-container)
bash -n .devcontainer/firewall-mode.sh
```

Tell the user :

> The firewall is being switched to `basic` mode for the duration
> of the migration. After everything is verified, the last step
> will flip it back to `strict`.

The actual mode flip happens **inside the container**, so :

- If the v1.x container is still running, ask the user to open a
  shell in it and run `./firewall-mode.sh basic` from
  `/workspaces/<project>/.devcontainer/`.
- If the container is stopped, the firewall is inactive anyway —
  the mode flip can wait until the first rebuild after Step 9.
  Either is fine, just note it explicitly.

**Confirm and continue.**

---

## Step 2 — Files to drop

These v1.x artefacts are removed in v2.0 (superseded or unused) :

| Path | Reason |
|---|---|
| `.devcontainer/gh-secure/` | Superseded by the `/prepare-pr` skill. |
| `.devcontainer/Dockerfile.node` | Generic `Dockerfile` covers Node now. |
| `.devcontainer/skills/master-review/` | Superseded by `/prepare-pr`. |
| `.devcontainer/KNOWLEDGE.md` *(single file)* | Replaced by `.devcontainer/knowledge/` directory (6 files). |
| `.devcontainer/test-db.php` | Unused in any v1.x project we know of. |
| `templates/gitignore-entries.txt` | install.sh embeds gitignore inline now (split between shipped `.devcontainer/.gitignore` and root-scope inline). |
| `.devcontainer/Dockerfile.custom` *(if present as a separate file)* | Renamed → `Dockerfile` (default project layer). Keep customisations, see Step 4. |

**Before removing** : check each path's presence and ask the user :

```bash
for p in .devcontainer/gh-secure .devcontainer/Dockerfile.node \
         .devcontainer/skills/master-review .devcontainer/KNOWLEDGE.md \
         .devcontainer/test-db.php; do
  [ -e "$p" ] && echo "PRESENT : $p" || echo "absent  : $p"
done
```

For each `PRESENT`, prompt the user :

> About to `git rm -r <path>` — this artefact has no replacement
> in v2. Proceed? [y/N]

If the user customised any of these (rare but possible — e.g. a
custom `gh-secure/` script), pause and discuss before deleting.

**Confirm and continue.**

---

## Step 3 — Files to overwrite verbatim (shipped, no customisation expected)

Authoritative shipped list — taken from `install_files()` in
`.tmp/migrate-v2/install.sh` :

**Lifecycle (6)** :

```
.devcontainer/initialize.sh
.devcontainer/on-create.sh
.devcontainer/post-create.sh
.devcontainer/post-start.sh
.devcontainer/shell-init.sh
.devcontainer/install-extensions.sh
```

**Firewall core (3 scripts + 8 data files)** :

```
.devcontainer/init-firewall.sh
.devcontainer/firewall-mode.sh   (already copied in Step 1)
.devcontainer/test-firewall.sh

.devcontainer/firewall/dnsmasq.conf
.devcontainer/firewall/compile-policy.py
.devcontainer/firewall/mitm-init.sh
.devcontainer/firewall/domains.txt
.devcontainer/firewall/domains.local.txt.example
.devcontainer/firewall/firewall-blocks
.devcontainer/firewall/default-mode
.devcontainer/firewall/direct-tcp-allow.txt
```

**Firewall trees (5 dirs)** :

```
.devcontainer/firewall/addons/
.devcontainer/firewall/policy.d/
.devcontainer/firewall/policy.local.d.example/
.devcontainer/firewall/policy.local.d/   (only the .keep — see Step 5)
.devcontainer/firewall/tests/
```

**Claude config (1 dir)** :

```
.devcontainer/claude/
```

**Knowledge (1 dir, replaces single KNOWLEDGE.md)** :

```
.devcontainer/knowledge/
```

**Docs (4)** :

```
.devcontainer/README.md
.devcontainer/RUNBOOK.md
.devcontainer/SECURITY.md
.devcontainer/RESEARCH.md
```

**Local-backend sidecar** :

```
.devcontainer/claude-bridge/
.devcontainer/host-helpers/
.devcontainer/diag-ollama-local.sh
```

**Skills (sync helper + 5 generic)** :

```
.devcontainer/skills/sync-skills.sh
.devcontainer/skills/prepare-pr/
.devcontainer/skills/watch-log/
.devcontainer/skills/prepare-research/
.devcontainer/skills/scan-deps/
.devcontainer/skills/prepare-plan/
```

**Tests** :

```
.devcontainer/tests/
```

**VS Code settings** :

```
.devcontainer/vscode-settings.json
```

For each entry above :

```bash
SRC=.tmp/migrate-v2/templates/v2
DST=.devcontainer

# Files
cp -v "$SRC/<file>" "$DST/<file>"

# Dirs (preserve structure)
rm -rf "$DST/<dir>" && cp -rv "$SRC/<dir>" "$DST/<dir>"
```

**Important nuance for `firewall/policy.local.d/`** : the shipped
version is essentially empty (just a `.keep`). If the user has
custom policies in their existing `firewall/policy.local.d/`, see
Step 5 — do **not** overwrite this dir blindly.

**Confirm and continue.**

---

## Step 4 — Dockerfiles (regenerate + re-apply user customisations)

v2 ships three Dockerfile files :

```
.devcontainer/Dockerfile.base   (verbatim, never customised)
.devcontainer/Dockerfile        (project layer, default for node/custom)
.devcontainer/Dockerfile.php    (project layer, for PROJECT_TYPE=php)
```

In v1.x, the user's `.devcontainer/Dockerfile` was produced by
concatenating `Dockerfile.base` + `Dockerfile.<type>`. In v2, the
two-file split is canonical and `install.sh` chooses one project
layer based on `PROJECT_TYPE`.

**Procedure** :

1. **Diff the current Dockerfile against v1.3's concat baseline**
   to identify user customisations :

   ```bash
   # Reconstruct what v1.3 would have produced
   cat .tmp/migrate-v2/devcontainer-tools-v1.3.0/templates/Dockerfile.base \
       .tmp/migrate-v2/devcontainer-tools-v1.3.0/templates/Dockerfile.<type> \
     > /tmp/v1.3-dockerfile-baseline

   # Note: the v1.3.0 sub-tree is NOT in the v2 repo. If you don't
   # have it locally, ask the user whether their Dockerfile is
   # customised; if no, skip to step 2.

   diff /tmp/v1.3-dockerfile-baseline .devcontainer/Dockerfile
   ```

   The non-baseline lines are the customisations to preserve
   (extra `RUN apt-get install …`, project-specific COPY, etc.).

2. **Overwrite with v2 shipped files** :

   ```bash
   cp .tmp/migrate-v2/templates/v2/Dockerfile.base .devcontainer/Dockerfile.base
   # Choose ONE based on PROJECT_TYPE:
   cp .tmp/migrate-v2/templates/v2/Dockerfile      .devcontainer/Dockerfile
   # OR
   cp .tmp/migrate-v2/templates/v2/Dockerfile.php  .devcontainer/Dockerfile
   ```

3. **Re-apply customisations** to the project-layer Dockerfile,
   AFTER the `FROM <base>` line. Show the diff to the user and ask
   them to confirm each customisation is still relevant in v2.

   Some customisations are no longer needed in v2 :

   - GitHub CLI install : already in v2 base.
   - `gh-secure/` COPY + sudoers : dropped (skill replaces it).
   - Manual firewall data COPY : already in v2 Dockerfile (split S3).

**Confirm and continue.**

---

## Step 5 — Local configs to preserve (NEVER overwrite)

These files belong to the user — they must survive the migration
intact. Audit each one and report status :

| File | What to do |
|---|---|
| `.devcontainer/.env` | Preserve verbatim. Existing values stay. |
| `.devcontainer/.configured-setup` | Will be rewritten in Step 8 with `VERSION="2.0.0"`. Read it first to capture the existing values. |
| `.devcontainer/firewall/domains.local.txt` | Preserve. If absent, create from `.example` (Step 7). |
| `.devcontainer/firewall/policy.local.d/*.toml` | Preserve all user files. The `.keep` sentinel may or may not exist — add it if missing (`touch .devcontainer/firewall/policy.local.d/.keep`). |
| `.devcontainer/skills/*.local/` | Preserve all `.local` skills (hours.local, master-review.local, etc.). v2 does NOT ship them — they stay per-user. |
| `.devcontainer/skills/**/*.local.skill.md` | Preserve. |
| `LESSONS.local.md` (root) | Preserve content, but relocate (Step 6). |

Show the user a listing :

```bash
ls -la .devcontainer/.env .devcontainer/.configured-setup 2>/dev/null
ls -la .devcontainer/firewall/domains.local.txt 2>/dev/null
ls -la .devcontainer/firewall/policy.local.d/ 2>/dev/null
ls -d .devcontainer/skills/*.local 2>/dev/null
find .devcontainer/skills -name '*.local.skill.md' 2>/dev/null
ls -la LESSONS.local.md 2>/dev/null
```

**Confirm none of these are overwritten before continuing.**

---

## Step 6 — Relocations

### 6a. `LESSONS.md` and `LESSONS.local.md` : root → `.devcontainer/` + root symlink

v2 keeps team-shared `LESSONS.md` in `.devcontainer/` and exposes
it at the project root via a symlink (mode 120000, same pattern as
`CLAUDE.md`).

```bash
# If root LESSONS.md exists and is a regular file (not a symlink)
if [ -f LESSONS.md ] && [ ! -L LESSONS.md ]; then
  mv LESSONS.md .devcontainer/LESSONS.md
fi

# If root LESSONS.md doesn't exist at all, seed from template
if [ ! -e .devcontainer/LESSONS.md ]; then
  cp .tmp/migrate-v2/templates/v2/LESSONS.md .devcontainer/LESSONS.md
fi

# Local sibling (gitignored)
if [ -f LESSONS.local.md ] && [ ! -L LESSONS.local.md ]; then
  mv LESSONS.local.md .devcontainer/LESSONS.local.md
fi

# Root symlink
[ -L LESSONS.md ] || ln -sf .devcontainer/LESSONS.md LESSONS.md
ls -la LESSONS.md   # must show -> .devcontainer/LESSONS.md, mode lrwxr-xr-x
```

### 6b. `KNOWLEDGE.md` (single file) → `.devcontainer/knowledge/` (6 files)

The mono `KNOWLEDGE.md` is dropped (Step 2). The knowledge tree
was already copied in Step 3 (`copy_dir knowledge`). Nothing to
do here beyond verifying the new layout :

```bash
ls .devcontainer/knowledge/
# Expected: INDEX.md, firewall.md, wtf.md, extension-points.md,
#           docker-base-image.md, ollama-local.md (or similar — 6 files)
```

### 6c. Legacy firewall config relocations

v2 baked two former mutable surfaces into firewall data files.
If they exist in the project, port them :

```bash
# Legacy firewall mode (was at .devcontainer/.configured-firewall-mode)
if [ -f .devcontainer/.configured-firewall-mode ]; then
  cp .devcontainer/.configured-firewall-mode .devcontainer/firewall/default-mode
  rm .devcontainer/.configured-firewall-mode
fi

# Legacy direct-TCP allowlist (was in .env as CLAUDE_CODE_FIREWALL_ALLOWED=)
if grep -q '^CLAUDE_CODE_FIREWALL_ALLOWED=' .devcontainer/.env 2>/dev/null; then
  value=$(grep '^CLAUDE_CODE_FIREWALL_ALLOWED=' .devcontainer/.env \
            | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'")
  if [ -n "$value" ]; then
    {
      echo ""
      echo "# Migrated from .env CLAUDE_CODE_FIREWALL_ALLOWED on $(date +%Y-%m-%d)"
      echo "$value" | tr ',' '\n' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
    } >> .devcontainer/firewall/direct-tcp-allow.txt
  fi
  # Strip the line from .env
  sed -i.bak '/^CLAUDE_CODE_FIREWALL_ALLOWED=/d' .devcontainer/.env
  rm -f .devcontainer/.env.bak
fi
```

These are the exact relocations `install.sh::migrate_legacy_firewall()`
would perform on a fresh re-install. Mirror its behaviour here.

**Confirm and continue.**

---

## Step 7 — Templated files (sed substitution)

Only two files need placeholder substitution in v2 :

```
.devcontainer/devcontainer.json   — {{PROJECT_ID}}, {{PROJECT_DISPLAY_NAME}}
.devcontainer/.env.example        — {{PROJECT_ID}}, {{PROJECT_DISPLAY_NAME}}
```

Read existing values from the legacy `.configured-setup` (or ask
the user if pre-1.0). Then :

```bash
PROJECT_ID="<from .configured-setup>"
DISPLAY="<from .configured-setup>"
SRC=.tmp/migrate-v2/templates/v2
DST=.devcontainer

for f in devcontainer.json .env.example; do
  cp "$SRC/$f" "$DST/$f"
  sed -i.bak \
    -e "s|{{PROJECT_ID}}|$PROJECT_ID|g" \
    -e "s|{{PROJECT_DISPLAY_NAME}}|$DISPLAY|g" \
    "$DST/$f"
  rm "$DST/$f.bak"
done
```

The existing `.env` is **not** overwritten (Step 5). If
`.env` is missing entirely, generate it from the freshly-templated
`.env.example` :

```bash
[ -f .devcontainer/.env ] || cp .devcontainer/.env.example .devcontainer/.env
```

Also bring in `domains.local.txt` from the example if the user
doesn't already have one :

```bash
[ -f .devcontainer/firewall/domains.local.txt ] \
  || cp .devcontainer/firewall/domains.local.txt.example \
        .devcontainer/firewall/domains.local.txt
```

**Confirm and continue.**

---

## Step 8 — Gitignore (split architecture)

v2 ships two gitignore surfaces :

1. `.devcontainer/.gitignore` — internal-scope rules, shipped
   verbatim. Already copied as part of Step 3 (`copy_verbatim
   .gitignore` in install.sh).
2. Root `<project>/.gitignore` — root-scope fragment appended
   to whatever the project already has.

For (1), verify presence :

```bash
test -f .devcontainer/.gitignore || cp .tmp/migrate-v2/templates/v2/.gitignore .devcontainer/.gitignore
```

For (2), append the root fragment — but only if not already
applied. The fragment's first line is the sentinel :

```bash
SENTINEL="$(head -n1 .tmp/migrate-v2/templates/v2/.gitignore-root)"
if ! grep -qxF "$SENTINEL" .gitignore 2>/dev/null; then
  [ -s .gitignore ] && echo "" >> .gitignore
  cat .tmp/migrate-v2/templates/v2/.gitignore-root >> .gitignore
fi
```

Also remove any v1.x-era gitignore entries that the new fragment
supersedes. Common offenders to look for and clean up :

- `.devcontainer/.env` (now covered by shipped `.devcontainer/.gitignore`)
- `.devcontainer/.configured-*` (same)
- `.devcontainer/skills/**/*.local*` (same)
- `.devcontainer/firewall/domains.local.txt` (same)

Show the user the resulting `.gitignore` and confirm no entries are
duplicated.

**Confirm and continue.**

---

## Step 9 — Write the v2 marker

Rewrite `.configured-setup` with `VERSION="2.0.0"` and the values
captured in Step 0 :

```bash
cat > .devcontainer/.configured-setup <<EOF
# Auto-generated by migration v1.x → v2.0 on $(date +%Y-%m-%d) — do not edit.
VERSION="2.0.0"
PROJECT_ID="<from-step-0>"
PROJECT_DISPLAY_NAME="<from-step-0>"
PROJECT_TYPE="<node|php|custom>"
CLAUDE_CREDS_VOLUME="<from-step-0-or-empty>"
EOF
```

The exact field set matches `install.sh::write_v2_marker()`. If
some v1.x fields aren't relevant in v2 (e.g.
`INCLUDE_CLAUDE_SETTINGS`, `HAS_DB`), drop them.

**Confirm and continue.**

---

## Step 10 — Set exec permissions on shipped scripts

Mirrors `install.sh::set_exec_perms()` :

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

## Step 11 — Rebuild and smoke test

1. **Rebuild Container** (VS Code command palette : *Dev
   Containers : Rebuild Container*).
2. After build, in a container shell :

   ```bash
   tail -100 /tmp/post-start.log
   ```

   Expected lines :
   - Firewall status (basic mode active — flipped back to strict
     in Step 12).
   - Credentials sync status.
   - Skills sync status (5 generic skills synced to
     `~/.claude/commands/`).
   - Session banner with GitHub / Claude mode.

3. **Application smoke** : whatever the project's normal startup
   smoke is (run dev server, run tests, open the app). The app
   must function in basic mode before flipping to strict.

If anything fails, **stop**. Do not flip to strict. Investigate
with the user — likely a missing entry in `domains.local.txt` or
`policy.local.d/`.

---

## Step 12 — Switch firewall to `strict` mode

Once the smoke passes, inside the container :

```bash
./firewall-mode.sh strict
```

This re-enables mitmproxy + L7 enforcement. The first run after
flipping may surface domains that `basic` mode tolerated but
`strict` blocks — add them to `firewall/domains.local.txt` and/or
write `firewall/policy.local.d/*.toml` policies as needed. Iterate
until the smoke passes in `strict`.

---

## Step 13 — Cleanup

```bash
# Remove the migration staging dir
rm -rf .tmp/migrate-v2

# If you don't want .tmp tracked at all, the line already in .gitignore
# from the bootstrap handles it.
```

**Migration complete.** Commit the resulting `.devcontainer/`
changes with a message like :

```
Migrate devcontainer from v1.x to v2.0.0

- Drops: gh-secure/, Dockerfile.node, master-review/ skill, KNOWLEDGE.md mono
- Adds: knowledge/, claude-bridge/, host-helpers/, 5 generic skills
- Relocations: LESSONS.{md,local.md} → .devcontainer/ + root symlink
- Marker bumped to VERSION="2.0.0"
```

---

## Troubleshooting

**`install.sh` refuses with "Detected legacy v1 devcontainer"** :
that's expected. Don't run `install.sh` — the playbook above
replaces it.

**Container fails to build after rebuild** : `Dockerfile.base` may
have changed in a way incompatible with a v1.x cached base image.
Force a fresh base build :

```bash
docker rmi claude-devcontainer-base:$(grep CLAUDE_CODE_VERSION \
  .devcontainer/.env | cut -d= -f2) 2>/dev/null
```

then *Rebuild Container* again.

**Skills don't appear in `/<slash>` autocomplete** : check that
`post-start.sh` ran `skills/sync-skills.sh` — look for it in
`/tmp/post-start.log`. If absent, re-run manually inside the
container :

```bash
bash .devcontainer/skills/sync-skills.sh
```

**`firewall-mode strict` blocks something that worked in basic** :
expected on first strict run. Add domains/policies (see Step 12)
or flip back to `basic` temporarily while you investigate.

---

## Authoritative reference

The exact list of files v2 ships is encoded in
`.tmp/migrate-v2/install.sh::install_files()` — if any class in
this playbook diverges from that function, **trust the function,
not the playbook**, and please report the divergence.

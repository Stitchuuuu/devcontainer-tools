# Migration to 1.1.0

Guide for upgrading an existing `.devcontainer/` to version 1.1.0.
Works for both legacy (pre-1.0.0, no `.configured-setup`) and 1.0.0 installs.

## Quick path

If the project was installed with the template creator:

```bash
bash /path/to/.devcontainer-tpl-creator/update.sh /path/to/project
```

The update script auto-detects legacy/1.0.0 and handles migration.
Below is the manual checklist for verification or manual migration.

---

## 1. `.configured-setup`

**If missing (legacy):** create it with project values.

```bash
cat > .devcontainer/.configured-setup << 'EOF'
VERSION="1.1.0"
PROJECT_ID="<slug>"
DISPLAY_NAME="<VS Code title>"
PROJECT_TYPE="<node|php|custom>"
TIMEZONE="<tz>"
NETWORK_NAME="<external-network-or-empty>"
HAS_DB="<true|false>"
DB_HOST="<host>"
DB_PORT="<port>"
DB_NAME="<name>"
DB_USER="<user>"
SHARED_CREDS_VOLUME="<volume-name-or-empty>"
INCLUDE_CLAUDE_SETTINGS="true"
EOF
```

**If exists (1.0.0):** update the VERSION line to `1.1.0`.

---

## 2. Raw files — copy from templates

These files have no project-specific placeholders. Safe to overwrite:

```
post-create.sh
post-start.sh          # now includes sync-skills.sh call
shell-init.sh          # now includes session banner
init-firewall.sh       # now includes "already active" guard
vscode-settings.json
README.md              # skip if project-customized
gh-secure/*.sh         # 6 files (new directory for legacy)
firewall/ranges.d/*.sh
skills/sync-skills.sh
skills/hours.local/hours.local.skill.md
skills/hours.local/hours-calibrate.local.skill.md
skills/hours.local/hooks.json
skills/hours.local/hours-log.sh
skills/hours.local/hours.config.md
```

`hours-calibration.json` — copy only if template file is < 30 days old.

---

## 3. Dockerfile — regenerate from templates

The Dockerfile is templated (base + project type), so `update.sh` does NOT touch it.
The simplest approach is to regenerate it by concatenating the template files:

```bash
TPL="/path/to/.devcontainer-tpl-creator/templates"
cat "$TPL/Dockerfile.base" "$TPL/Dockerfile.<type>" > .devcontainer/Dockerfile
```

Where `<type>` is `php`, `node`, or `custom` (matches `PROJECT_TYPE` in `.configured-setup`).

This gives you an identical Dockerfile with all 1.1.0 changes:
- GitHub CLI from official repo (CVE-2024-53858 fix)
- Firewall config baked into image
- gh-secure scripts + sudoers
- Shell init sourced from workspace (no rebuild needed on changes)

> **⚠️ ATTENTION — Do NOT blindly overwrite the Dockerfile.**
>
> Projects installed before 1.1.0 may already have some of these changes applied manually
> (e.g., GH CLI from official repo, gh-secure COPY/sudoers). The only **new** change in
> 1.1.0 is the firewall config baked into the image:
>
> ```dockerfile
> # In the "Firewall script + sudoers" section, add:
> COPY firewall/domains.txt /etc/devcontainer-firewall/domains.txt
> COPY firewall/ranges.d/ /etc/devcontainer-firewall/ranges.d/
> ```
> and the corresponding `chown`/`chmod` in the same `RUN` block.
>
> **Always `diff` your current Dockerfile against the concatenated template before replacing.**
> If the project has custom additions (extra packages, custom RUN steps), re-apply them
> after the stage 2 `FROM devcontainer-claude-base` line.

**Note:** if the project has custom additions in the Dockerfile (extra packages, custom RUN steps),
diff before overwriting and re-apply them after the stage 2 `FROM devcontainer-claude-base` line.

---

## 4. docker-compose.yml changes

### 4a. Add firewall config mount (read-only)

Under `services.app.volumes`, add:

```yaml
      - ./firewall:/etc/devcontainer-firewall:ro
```

This mounts the workspace `firewall/` directory over `/etc/devcontainer-firewall/` (read-only).
Without this mount, only the baked files from the Dockerfile are used and `domains.local.txt`
(user-specific, gitignored) is never loaded by `init-firewall.sh`.

The `:ro` flag prevents any modification from inside the container (even via sudo).
The `touch` in the Dockerfile creates an empty fallback if the mount is absent.

### 4b. Add gh-secrets volume mount

Under `services.app.volumes`, add:

```yaml
      - gh-secrets:/mnt/gh-secrets:ro
```

### 4c. Add gh-secrets volume definition

Under `volumes`, add:

```yaml
  gh-secrets:
    name: gh-secrets-${DC_PROJECT:-<PROJECT_ID>}
    external: true
```

**Important:** the volume must exist before `docker compose up`.
The `initialize.sh` script (via `initializeCommand`) creates it automatically.

---

## 5. devcontainer.json changes

Update the lifecycle commands:

```json
"initializeCommand": "bash .devcontainer/initialize.sh",
"postCreateCommand": "bash .devcontainer/post-create.sh",
```

If `initialize.sh` doesn't exist yet, create it from template with `{{PROJECT_ID}}` replaced.

---

## 6. Firewall — `domains.local.txt`

Calibration domains are now in `firewall/domains.local.txt` (gitignored, user-specific).
Create the file if missing:

```bash
cp templates/firewall/domains.local.txt .devcontainer/firewall/domains.local.txt
```

If the project already has Hours calibration domains in `domains.txt`, **remove them** — they belong in `domains.local.txt` now.

`init-firewall.sh` now reads both `domains.txt` (committed) and `domains.local.txt` (local) automatically.

---

## 7. New directories

Ensure these exist:

```bash
mkdir -p .devcontainer/{gh-secure,skills/hours.local}
```

---

## 8. .gitignore

Add if missing (under a `# DevContainer` section):

```
# DevContainer
.env.dev
.devcontainer/.env
.devcontainer/.configured-*
.vscode/
# Skills — local (personal) & runtime files
.devcontainer/skills/**/*.local.skill.md
.devcontainer/skills/**/*.local/
.devcontainer/firewall/domains.local.txt
```

See `templates/gitignore-entries.txt` for the canonical list.

---

## 9. Optional: .claude/settings.local.json

Copy the read-only permissions template to the project root:

```bash
mkdir -p .claude
cp templates/claude/settings.local.json .claude/settings.local.json
```

This pre-approves safe operations (Read, Grep, git read-only, ls, etc.)
so Claude doesn't ask permission for every file read.

---

## 10. Rebuild

After all changes, rebuild the container:

```
VS Code → Rebuild Container
```

On first rebuild with `initialize.sh`, you'll be prompted to choose auth mode (Standard / Advanced).

---

## Verification

After rebuild, check the post-start log:

```bash
cat /tmp/post-start.log
```

Expected output includes:
- Firewall status (active or initialized)
- Credentials sync status
- Claude CLI pre-configuration
- GitHub CLI auth status
- Skills sync status
- Session banner with GitHub/Claude mode

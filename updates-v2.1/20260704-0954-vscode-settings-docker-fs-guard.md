# 20260704-0954 — Docker FS overflow guard in `vscode-settings.jsonc`

**Affects** : v2.1 devcontainers where the workspace VS Code settings
file is plain `vscode-settings.json` (no JSONC comments allowed), and
where dense workspaces make Docker Desktop's virtual FS
(VirtioFS/gRPCFS) spin or crash.

**Symptom** : VS Code feels sluggish on save/search, or the container
CPU stays high at idle, or the devcontainer boot crashes at
`.git/objects/` walks. No documented tiered opt-in in the workspace
settings.

**Fix** : rename `vscode-settings.json` → `vscode-settings.jsonc` and
seed the file with a **3-tier commented Docker FS guard** :

- **TIER 1** — file watcher / search excludes for dense dirs.
- **TIER 2** — kill git auto-fetch/refresh, gutter, GitLens live probes.
- **TIER 3** — nuke `git.enabled` (fatal for VS Code Git UI ; terminal
  `git` unaffected).

Tiers are cumulative. All disabled by default — user uncomments as
needed. Update the docker-compose bind path and `install.sh` copy step
accordingly.

## Manual how-to

Four steps per side, four on the dogfood side, three on the template
side (no `install.sh` mirror there).

### Step 1 — Rename bind-mounted file

Rename the settings file :

`````bash
git mv .devcontainer/vscode-settings.json .devcontainer/vscode-settings.jsonc

git mv templates/v2/vscode-settings.json templates/v2/vscode-settings.jsonc
`````

### Step 2 — Replace content of both files

Open `.devcontainer/vscode-settings.jsonc` and replace **all its content**
with :

`````jsonc
{
	// ─────────────────────────────────────────────────────────────
	// Docker FS overflow guard — DISABLED BY DEFAULT
	// ─────────────────────────────────────────────────────────────
	// Uncomment ONE tier below based on how much Docker Desktop's
	// virtual FS (VirtioFS/gRPCFS) is suffering. Tiers are cumulative:
	// TIER 2 assumes TIER 1 is on, TIER 3 assumes TIER 1 + 2.
	//
	// TIER 1 — LOWER
	//   Excludes the noisy dirs from file watcher + search. GitLens,
	//   git auto-refresh, SCM panel stay fully active.
	//   Use when: workspace is dense but Docker FS only occasionally
	//   spins; VSCode feels sluggish on save/search.
	//
	// TIER 2 — MEDIUM
	//   Adds: kills git auto-fetch/refresh, auto-repo detection, gutter
	//   decorations, GitLens auto-blame. SCM panel still visible but
	//   no longer polls / walks the tree. `git.untrackedChanges: hidden`
	//   reduces `git status -uall` walks — NOT fully (see microsoft/
	//   vscode#131020) but enough in practice.
	//   Use when: opening a file or the devcontainer boot makes Docker
	//   FS choke; container CPU stays high at idle.
	//
	// TIER 3 — EXTREME (fatal for VSCode Git UI)
	//   Adds: `git.enabled: false`. Kills the entire VSCode Git
	//   extension. No SCM panel, no gutter diff, no GitLens (depends
	//   on git.enabled). Manual `git` in a terminal is 100 % unaffected.
	//   Use when: Docker FS still crashes at devcontainer boot even
	//   with TIER 2 applied.
	//
	// If you have ONE identifiable dense directory, also uncomment the
	// "YOUR_DENSE_DIR" line in TIER 1 (benefits all tiers).
	//
	// After editing: Reload Window (Cmd/Ctrl+Shift+P → "Reload Window").
	// Bind-mount → no rebuild needed.
	// ─────────────────────────────────────────────────────────────

	// ─── TIER 1 (lower) ──────────────────────────────────────────
	//
	// "files.watcherExclude": {
	// 	"**/.git/objects/**": true,
	// 	"**/.git/subtree-cache/**": true,
	// 	"**/node_modules/**": true,
	// 	"**/vendor/**": true,
	// 	"**/dist/**": true,
	// 	"**/build/**": true,
	// 	"**/.next/**": true,
	// 	"**/.cache/**": true,
	// 	"**/coverage/**": true,
	// 	"**/.turbo/**": true,
	// 	"**/target/**": true,
	// 	"**/__pycache__/**": true,
	// 	"**/.venv/**": true
	// 	// ,"**/YOUR_DENSE_DIR/**": true
	// },
	// "search.exclude": {
	// 	"**/node_modules": true,
	// 	"**/vendor": true,
	// 	"**/dist": true,
	// 	"**/build": true,
	// 	"**/.next": true,
	// 	"**/coverage": true,
	// 	"**/target": true,
	// 	"**/__pycache__": true,
	// 	"**/.venv": true,
	// 	"**/*.tar.gz": true,
	// 	"**/*.tar": true,
	// 	"**/*.zip": true
	// 	// ,"**/YOUR_DENSE_DIR": true
	// },

	// ─── TIER 2 (medium) — needs TIER 1 above ────────────────────
	//
	// "git.autofetch": false,
	// "git.autorefresh": false,
	// "git.decorations.enabled": false,
	// "git.untrackedChanges": "hidden",
	// "git.autoRepositoryDetection": false,
	// "git.repositoryScanMaxDepth": 1,
	// "gitlens.currentLine.enabled": false,
	// "gitlens.codeLens.enabled": false,
	// "gitlens.statusBar.enabled": false,

	// ─── TIER 3 (extreme / fatal) — needs TIER 1 + 2 above ───────
	//
	// "git.enabled": false,
	//
	// ─────────────────────────────────────────────────────────────

	"claudeCode.preferredLocation": "primary",
	"revealInOS.debug": true,
	"revealInOS.implementation": "spawn-only"
}
`````

For `templates/v2/vscode-settings.jsonc`, use the **same content** but
**drop the last two settings** (`revealInOS.debug` and
`revealInOS.implementation`) — the template ships only
`claudeCode.preferredLocation`. Final block :

```jsonc
	"claudeCode.preferredLocation": "primary"
}
```

### Step 3 — Update docker-compose bind paths (both sides)

Open `.devcontainer/docker-compose.yml`. Around line 47, find :

```yaml
      - ./vscode-settings.json:/workspace/.vscode/settings.json:bind
```

Change to :

```yaml
      - ./vscode-settings.jsonc:/workspace/.vscode/settings.json:bind
```

Do the exact same edit in `templates/v2/docker-compose.yml`.

### Step 4 — Update `install.sh` (dogfood side only)

Open `install.sh`. Around line 268 (in `install_files`), find :

```bash
    copy_verbatim vscode-settings.json
```

Change to :

```bash
    copy_verbatim vscode-settings.jsonc
```

Only `install.sh` — not `templates/v2/install.sh` (there isn't one ;
the shipped installer is the same file).

### Commit

`````bash
git add .devcontainer/vscode-settings.jsonc .devcontainer/docker-compose.yml \
  templates/v2/vscode-settings.jsonc templates/v2/docker-compose.yml \
  install.sh

git commit -m "feat(vscode): jsonc settings with tiered Docker FS overflow guard"
`````

### Rebuild the container

Palette → `Dev Containers: Rebuild Container`. The bind mount now
points at the `.jsonc` file. Post-rebuild, activate a tier by
uncommenting the corresponding block, then `Developer: Reload Window`
(no rebuild needed for tier toggles thanks to bind-mount).

## Verify

- [ ] `test -f .devcontainer/vscode-settings.jsonc && ! test -f .devcontainer/vscode-settings.json`
      → both true (rename applied dogfood).
- [ ] `test -f templates/v2/vscode-settings.jsonc && ! test -f templates/v2/vscode-settings.json`
      → both true (rename applied template).
- [ ] `grep -c vscode-settings.jsonc .devcontainer/docker-compose.yml templates/v2/docker-compose.yml install.sh`
      → 1 each.
- [ ] `grep -c '"claudeCode.preferredLocation": "primary"' .devcontainer/vscode-settings.jsonc templates/v2/vscode-settings.jsonc`
      → 1 each.
- [ ] `grep -c '// TIER' .devcontainer/vscode-settings.jsonc`
      → ≥ 3 (three tier headers present).
- [ ] After rebuild + `Reload Window`, uncomment TIER 1's
      `files.watcherExclude` block, save, `Reload Window`. Verify VS
      Code no longer spins on `git status` under a `node_modules`-heavy
      workspace.

## Rollback

`````bash
git revert <commit-hash>
`````

Or manually : rename `.jsonc` back to `.json`, revert content to the
1-line `{"claudeCode.preferredLocation": "primary"}`, revert
`docker-compose.yml` bind path and `install.sh` copy step. Rebuild.

# 20260612-1857 — Navigator `PendingMigration` activation fix

> **Idempotency note** — if
> `.devcontainer/claude/vscode-ext-patchs/navigator-pending-migration-fix.py`
> already exists in your downstream with identical content (some forks
> picked it up out-of-band before the July batch shipped), `git apply`
> will fail with *"already exists in working directory"*. Skip this
> patch — the target state is already reached. Verify with
> `git apply --reverse --check <patch>` (exit 0 → already applied).

**Affects** : v2.1 devcontainers where the Claude Code VS Code
extension crashes on activation with
`PendingMigrationError` / `TypeError: Cannot read properties of undefined (reading 'PendingMigration')`.
Symptom appears intermittently on VS Code 1.108+ when Zod (bundled
inside the extension) probes `globalThis.navigator` at module load.

**Cause** : VS Code redefines `globalThis.navigator` with
`configurable: true` and a getter that throws
`PendingMigrationError` on any access. Zod's env-detection touches
that getter during extension bootstrap → uncaught throw → extension
never finishes activation → the Claude side panel stays blank.

**Resolution** : new patcher `navigator-pending-migration-fix.py`
prepends an IIFE to `extension.js` that redefines
`globalThis.navigator` as `undefined` **before** Zod loads,
neutralising the getter. Idempotent via a sentinel marker so
`run-all.sh` re-runs stay clean.

Also ships a standalone `.md` (why-doc) alongside the patcher so the
next contributor bumping `CLAUDE_CODE_VERSION` can decide whether the
hack is still needed (root cause + repro instructions).

**Upstream commits** :

- `cc7cf11` — `feat(vscode-ext-patchs): navigator activation fix + 6-step icon-fix`
- `8f40ddf` — `docs(vscode-ext-patch): standalone why-doc for navigator-pending-migration-fix`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260612-1857-vscode-ext-navigator-pending-migration-fix.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260612-1857-vscode-ext-navigator-pending-migration-fix.patch

git add .devcontainer/claude/vscode-ext-patchs/navigator-pending-migration-fix.py \
        .devcontainer/claude/vscode-ext-patchs/navigator-pending-migration-fix.md
git commit -m "feat(vscode-ext-patchs): navigator PendingMigration activation fix"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] `test -f .devcontainer/claude/vscode-ext-patchs/navigator-pending-migration-fix.py`
      → present.
- [ ] `bash .devcontainer/claude/vscode-ext-patchs/run-all.sh <extension-dir>` runs
      without errors and prints the navigator patch step.
- [ ] Reload window → Claude side panel activates without
      `PendingMigration` errors in the extension host log (open with
      `Developer: Show Running Extensions` → `Claude Code` → check
      output).
- [ ] Optional : `grep -c "PENDING_MIGRATION_SENTINEL" <extension-dir>/dist/extension.js`
      → 1 (marker injected, idempotent).

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

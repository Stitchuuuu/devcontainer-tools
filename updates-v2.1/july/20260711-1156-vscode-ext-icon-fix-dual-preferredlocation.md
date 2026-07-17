# 20260711-1156 — `icon-fix` dual-value `preferredLocation` with ownership detection

**Affects** : v2.1 devcontainers with an existing
`icon-fix-open-in-current-panel.py` patcher. Downstream extension
version 2.1.145 → 2.1.207.

**Symptom** : the previous `resolve_enum_name` helper picked a single
`preferredLocation` enum value based on a hard version cutoff
(`primary` before 2.1.202, `active-panel` after). Forced migration on
version bumps caused settings churn and would break entirely if
Anthropic ever declared `"primary"` natively.

**Cause** : single-value ownership model, no collision handling.

**Resolution** : replace `resolve_enum_name` with `resolve_owned_names`
returning a **set** of owned enum values :

- `"--active-panel"` — vendor-prefixed alias (WebKit-style). Always
  ours, always injected. Collision-safe by construction (`--` prefix
  reserved).
- `"primary"` — claimed **only** if Anthropic hasn't declared it or if
  our description signature matches. Detected via `OWN_DESCS` lookup
  in `package.json`.
- `"active-panel"` (bare, no prefix) — legacy value from earlier
  installs, preserved in ownership if found.

All downstream JS checks become
`[...owned].includes(getConfig("preferredLocation"))` instead of
single-string compare, so existing configs on `"primary"`,
`"active-panel"` or `"--active-panel"` all keep working — zero
migration required.

**Upstream commit** : `42a92ea` — `refactor(vscode-ext-patchs): dual-value preferredLocation with ownership detection`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260711-1156-vscode-ext-icon-fix-dual-preferredlocation.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260711-1156-vscode-ext-icon-fix-dual-preferredlocation.patch

git add .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py
git commit -m "refactor(vscode-ext-patchs): dual-value preferredLocation + ownership detection"

bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

echo "Done — Cmd+Shift+P → Developer: Reload Window."
`````

## Verify

- [ ] `grep -c "OWN_DESCS\|resolve_owned_names" .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py`
      → ≥ 4 hits (helper + call sites).
- [ ] `grep -c "resolve_enum_name" .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py`
      → 0 (old single-value helper fully removed).
- [ ] Existing setting `claudeCode.preferredLocation = "primary"` still
      works after reload — the `+` button and Claude icon both open in
      the active column with full chrome (session-tabs + right menu).
- [ ] Flip to `claudeCode.preferredLocation = "--active-panel"` in
      workspace settings → same behavior (both values are claimed).
- [ ] Optional : `python3 .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py <extension-dir>`
      run twice → second run is idempotent (no re-injection log lines).

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*
`````

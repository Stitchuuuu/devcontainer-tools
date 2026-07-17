# July batch — downstream-ready, self-contained

Targeted updates bundling the July fixes on top of `v2.1.0`. Each recipe
carries an `Affects` / `Symptom` / `Cause` / `Resolution` / `Apply` /
`Verify` block and points to the upstream commit(s) that landed the
change in dogfood + `templates/v2/`.

## Design rules

- **`.devcontainer/` scope only.** Every `.patch` file is strictly
  scoped to `.devcontainer/*` — no hunks in `templates/v2/`, `docs/`,
  `.gitignore`, or `updates-v2.1/`. Those paths only exist in the
  `devcontainer-tools` upstream repo, not in downstream forks.
- **Independent patches.** No `.md` in this folder points at another
  patch as a prerequisite. Apply order is alphabetical ; the
  `YYYYMMDD-HHMM` filename prefix encodes it.
- **Idempotent apply.** `apply-batch.sh` runs `git apply --reverse
  --check` first ; already-applied recipes are skipped, not errored.

## Recipes shipped (15)

Apply order = alphabetical filename order.

| # | Filename prefix | Scope |
|---|---|---|
| 1 | `20260612-1857-vscode-ext-navigator-pending-migration-fix` | new patcher `navigator-pending-migration-fix.py` |
| 2 | `20260613-0929-plus-button-chrome` | `icon-fix-open-in-current-panel.py` — `+` button chrome regression fix |
| 3 | `20260613-1934-notify-accents-state` | notify accent decode + `payload` persistence in state (+ hook + tests) |
| 4 | `20260707-0811-vscode-ext-outbound-action-injector` | creates `outbound-tester.js`, `outbound-action-injector.py`, `webview-simulated-click.py` v1, refactors `icon-fix-open-in-current-panel.py` |
| 5 | `20260711-0701-vscode-ext-disable-webview-auth-redirect-create` | creates `disable-webview-auth-redirect.py` (upstream commit `47b5365`) |
| 6 | `20260711-0711-vscode-ext-webview-simulated-click` | `webview-simulated-click.py` v2 — reqId back-ref for 2.1.205+ |
| 7 | `20260711-1057-vscode-ext-rename-session-retry-poll` | `rename-session-restore-empty-file-fallthrough.py` retry-poll |
| 8 | `20260711-1156-vscode-ext-icon-fix-dual-preferredlocation` | `icon-fix-open-in-current-panel.py` — dual-value ownership |
| 9 | `20260712-0531-vscode-ext-disable-webview-auth-redirect-oauth-gate` | `disable-webview-auth-redirect.py` — `isAuthenticated` getter gate (delta on #5) |
| 10 | `20260715-0951-vscode-ext-handle-uri-workspace` | `handle-uri-workspace.py` — `/open?workspace=&prompt=` support |
| 11 | `20260715-0954-notify-features-rollup` | Full notify rollup — 20 files, includes `claude-code.icns` base85 blob |
| 12 | `20260716-0800-notify-daemon-nvm-autosource` | `initialize.sh` — `spawn_notify_daemon()` nvm auto-source + node-missing diag |
| 13 | `20260716-0812-initialize-split-into-helpers` | `initialize.sh` shrinks ~ 400 lines ; new `initialize/notify-daemon.sh` + `initialize/rebuild-debug.sh` |
| 14 | `20260717-1131-vscode-ext-outbound-action-injector-track-session-id` | outbound-action-injector v1→v2 — track SDK sessionId, rename `channelId`, unblocks Allow button |
| 15 | `20260717-1336-notify-consumers-title-and-input-polish` | `notify-app` title `projectName` seed + `ExitPlanMode` H1 branch + skip Allow on `AskUserQuestion` |

## Apply

```sh
cd <your-downstream-repo-root>          # the one containing .devcontainer/
bash updates-v2.1/july/apply-batch.sh --dry-run    # preview
bash updates-v2.1/july/apply-batch.sh              # apply
# --continue : keep going past a failing patch
```

Idempotent — already-applied recipes are skipped (reverse-check).

Naive `git apply *.patch` also works if your downstream matches the
pre-batch baseline for every recipe, but stops on the first
"already applied" collision.

## Verification per recipe

Each `.md` documents its own verify steps. Post-batch sanity check :

```sh
ls .devcontainer/claude/vscode-ext-patchs/*.py
bash .devcontainer/claude/vscode-ext-patchs/run-all.sh \
    /home/node/.vscode-server/extensions/anthropic.claude-code-*

test -f .devcontainer/claude/vscode-ext-patchs/disable-webview-auth-redirect.py

test -f .devcontainer/initialize/notify-daemon.sh
test -f .devcontainer/initialize/rebuild-debug.sh
grep -c '^spawn_notify_daemon()' .devcontainer/initialize.sh                 # → 0
grep -c '^spawn_notify_daemon()' .devcontainer/initialize/notify-daemon.sh   # → 1

test -f .devcontainer/notify/vendor/senders/claude-code.icns
test -f .devcontainer/notify/lib/consumers/notify-app.js
test -f .devcontainer/notify/tools/session-title.js
```

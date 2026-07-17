# 20260715-0954 — Notify features rollup (`notif` daemon integration + focus-aware + Allow inbox + session-title + sibling logs)

**Affects** : v2.1 devcontainers with the notify pipeline in the
"payload-persisted" state (`state.js` documents `payload`, `watcher.js`
propagates it, `README.md` describes it).

**Symptom / features being added** :

- **`notif` daemon dispatch (opt-in)** — plug the standalone
  `notif` binary (macOS UN backend) as a notify consumer. Adds
  `Allow` / `Focus` action buttons + persistent audit trail. Off by
  default ; enable via `NOTIFY_APP=notif` in project env.
- **Focus-aware banner delay** — no popup while the VS Code window
  is already focused (fires after a debounce window if the user
  navigates away). Reads focus state from the reciprocal outbound
  control channel (extension → notify).
- **Focus:open DSL** — click-to-focus payload
  `focus:open?workspace=<path>&session_id=<sid>` that routes back to
  the target project window (paired with the
  `handle-uri-workspace` ext patch).
- **Allow action button + inbox tail** — the notify banner exposes an
  `Allow` action for tool-permission prompts. Clicks read the pending
  request UUID from `~/.claude/pending-perms.jsonl` and dispatch the
  reciprocal `allow` message back to the extension (paired with the
  outbound-action-injector ext patch).
- **`getNotifPath` PATH scan fallback** — resolves the `notif` binary
  via a full `$PATH` scan (`/usr/local/bin`, `/opt/homebrew/bin`,
  Cargo target dirs), so downstream projects don't need to hardcode
  the path.
- **Session-title tool** — `notify/tools/session-title.js` maps
  Claude session UUIDs to their current VS Code tab title (indexed
  from the vscode-ext inbound jsonl log). Supports prefix lookups
  (≥ 4 chars).
- **1-banner-per-sid invariant** — new watcher-level dedupe so a
  single notify event never surfaces two banners for the same
  session.
- **Post-fire cancellation** — the watcher emits
  `cancelled:notification` even for events cancelled AFTER the
  banner was already dispatched (relevant for the `Allow` inbox
  reconciliation).
- **Non-macOS host unblock** — the `notify-app.js` consumer
  gracefully no-ops on Linux hosts (VM-smoke scenario documented in
  `tests/README-vm-smoke.md`).
- **Session-replay tool** — `tests/replay-session.js` replays
  captured session fixtures against a live notify pipeline for local
  debugging.
- **Sibling logs docs + outbound channel** — `notify/README.md`
  documents the reciprocal outbound control channel, sibling log
  layout, and the `Allow` inbox flow.
- **body-click via authority-writer** — notify-queue hook resolves
  the vscode-remote authority so click-to-focus works from
  containers.

**Upstream commits** :

- `5af37e6` — `feat(notifier): opt-in daemon dispatch via notif + remove subcommand + Claude Code identity`
- `42432b8` — `fix(notifier): v0.2.5 daemon boot flow — long-timeout register, bundle-exists short-circuit, Sequoia icon`
- `b986f51` — `fix(notifier): v0.2.7 emit cancelled:notification for post-fire cancels too`
- `db70d41` — `fix(notify-daemon): enforce 1-banner-per-sid invariant`
- `8857acd` — `feat(notify): focus-aware banner delay based on VS Code window state`
- `afe6fc0` — `feat(notif): add focus:open DSL for click-to-focus routing`
- `7247010` — `feat(notif): add Allow action button + inbox tail for permission requests`
- `3b5a18e` — `fix(notify): source Allow requestId from pending-perms.jsonl`
- `5a8dd5b` — `feat(notify-app): add /usr/local/bin + /opt/homebrew/bin to getNotifPath`
- `80a58be` — `feat(notify-app): PATH scan fallback in getNotifPath`
- `e4f1d7e` — `feat(notify): add session-title tool mapping Claude UUIDs to tab titles`
- `9a2f010` — `feat(notify-queue): resolve vscode-remote authority in hook.js`
- `8e4cd9d` — `feat(notify): fix body-click via authority-writer ext patch`
- `0253865` — `chore(notify): unblock notify-app.js for non-macOS hosts + VM-smoke doc`
- `1bde8ca` — `feat(notify): add replay-session tool + 3 captured session fixtures`
- `f0ec42e` — `docs(notify): teach README-vm-smoke about replay-session.js full-session replay`
- `f879f12` — `docs(notify): document sibling logs + outbound control channel`

## Runtime dependency

The `notif` daemon dispatch requires the standalone `notif` binary
(new macOS UN backend, ships from its own repo). Without `notif`
installed, the notify consumer falls back to the terminal-notifier
transport (default behavior — nothing breaks).

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260715-0954-notify-features-rollup.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260715-0954-notify-features-rollup.patch

git add .devcontainer/notify .devcontainer/skills/notify-queue
git commit -m "feat(notify): daemon dispatch + focus-aware + Allow inbox + session-title (rollup)"
`````

No daemon to restart, no rebuild — the notify pipeline re-reads its
modules on the next hook fire.

## Verify

- [ ] `test -f .devcontainer/notify/tools/session-title.js` → present.
- [ ] `test -f .devcontainer/notify/lib/focus-debounce.js` → present.
- [ ] `test -f .devcontainer/notify/lib/consumers/notifier.md` →
      present (dispatch docs).
- [ ] `grep -c "focus:open" .devcontainer/notify/lib/consumers/notify-app.js`
      → ≥ 1 (DSL parser present).
- [ ] `grep -c "getNotifPath" .devcontainer/notify/lib/consumers/notify-app.js`
      → ≥ 2 (PATH scan resolver present).
- [ ] Fire a test hook (`bash .devcontainer/notify/tests/fake-hook.sh`)
      → a banner surfaces once. Fire twice for the same sid → still
      one banner (1-banner-per-sid invariant).
- [ ] Focus a VS Code window → hooks fired during focus produce
      **no** banner (silence window). Blur → subsequent hooks fire
      normally after debounce.
- [ ] If `notif` binary installed and `NOTIFY_APP=notif` : the banner
      renders through UN center with `Allow` action button.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
`````

No state to migrate. Pipeline reverts to the pre-rollup behavior on
the next hook fire.

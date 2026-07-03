# 20260613-1934 — Notify accents + state payload

**Affects** : v2.x devcontainers from before commits `701240a`
(accents fix) and `6d550ac` (state payload) on `devcontainer-tools`
`main`.

**Symptom** :
- macOS notifications display ` u00e9` in place of `é` when the
  assistant's `**Recap** — …` line carries JSON-escaped accents
  instead of raw UTF-8.
- `.devcontainer/notify/queue/state/pending.json` and
  `.devcontainer/notify/queue/state/actions.jsonl` only carry
  `{sid, eventType, armed_at, fire_at, delay_ms}` — external
  consumers (status bars, dashboards, alternative renderers) can't
  render or route from the state alone, they have to re-read the
  source queue JSONL.

**Cause** : compound, fixed in two upstream commits.

- *Accents.* §14 of `CLAUDE-dev.md` said "plain text" — ambiguous
  about raw UTF-8 vs ASCII escapes. The notifier's `safe()`
  AppleScript-injection guard strips backslashes, turning `é`
  into ` u00e9`. Fix : §14 now mandates raw UTF-8 ; the
  notify-queue hook defensively decodes any literal `\uXXXX`
  survivors before they hit the AppleScript pipeline.
- *State.* The in-process notify daemon only persisted timer
  metadata, not the payload. Fix : `armed` / `replaced` mutations
  now accept the raw parsed JSONL `line` as `payload` and mirror
  it into `pending.json` + `actions.jsonl`. Additive change, no
  schema-version bump. `cancelled` / `fired` / `unmapped` stay
  payload-less by design (matching `armed` is searchable by
  `sid + ts`).

**Upstream commits** :
- `701240a` — `fix(template): decode \uXXXX escapes in notify-queue excerpt`
- `6d550ac` — `feat(template): persist notification payload in notify state`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname)
populates `.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates/20260613-1934-notify-accents-state/update.patch
git apply        .tmp/devcontainer-updates/updates/20260613-1934-notify-accents-state/update.patch

git add .devcontainer/claude/CLAUDE-dev.md \
        .devcontainer/skills/notify-queue/hook.js \
        .devcontainer/skills/notify-queue/test-excerpt.js \
        .devcontainer/notify/lib/state.js \
        .devcontainer/notify/lib/watcher.js \
        .devcontainer/notify/README.md \
        .devcontainer/notify/tests/state-test.js
git commit -m "fix(notify): accents decode + state payload"

if [ -f .devcontainer/notify/queue/.daemon.pid ]; then kill "$(cat .devcontainer/notify/queue/.daemon.pid)" 2>/dev/null || true; fi
nohup node .devcontainer/notify/index.js \
    >> .devcontainer/notify/queue/daemon.log 2>&1 &
disown

echo "Done — next notification will carry the payload field."
`````

## Verify

- [ ] `node .devcontainer/skills/notify-queue/test-excerpt.js`
      → `test-excerpt.js — all assertions passed` (6 assertions).
- [ ] `node .devcontainer/notify/tests/state-test.js`
      → `state-test.js — all assertions passed` (5 assertion blocks).
- [ ] `ps -p $(cat .devcontainer/notify/queue/.daemon.pid)` → daemon
      running with a fresh PID (post-restart).
- [ ] After the next `Stop` event (any finished Claude turn), inspect
      `.devcontainer/notify/queue/state/pending.json` — armed-timer
      entries now carry a `payload` object with the parsed JSONL
      line.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>

if [ -f .devcontainer/notify/queue/.daemon.pid ]; then kill "$(cat .devcontainer/notify/queue/.daemon.pid)" 2>/dev/null || true; fi
nohup node .devcontainer/notify/index.js \
    >> .devcontainer/notify/queue/daemon.log 2>&1 &
disown
`````

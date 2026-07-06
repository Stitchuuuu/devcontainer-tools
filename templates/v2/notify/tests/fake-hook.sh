#!/usr/bin/env bash
# fake-hook.sh — simulate what hook.js would write to the queue when
# Claude Code fires a real event, so the notify daemon runs its full
# pipeline (watcher → notify-app → notif send / remove) without needing
# an actual Claude Code session in the loop.
#
# Runs on the Mac host (or in the container — both see the same queue
# via bind-mount). Requires jq for the JSON build ; every mac has it.
#
# Usage :
#   ./fake-hook.sh notify           # fake `notification` event
#                                   # (idle_prompt sub-type — 0s delay,
#                                   # fires INSTANTLY)
#   ./fake-hook.sh perm             # fake `permission_request` event
#                                   # (30s delay before dispatch)
#   ./fake-hook.sh cancel <SID>     # fake `user_replied` for a
#                                   # previously-fired SID → should
#                                   # trigger `notif remove` on the
#                                   # dispatched banner
#   ./fake-hook.sh stop             # fake `stop` event (30s delay,
#                                   # renders a "Recap" body)
#
# The queue dir defaults to `.devcontainer/notify/queue/` relative to
# the current working directory. Override with $QUEUE_DIR if the daemon
# writes elsewhere on your host.

set -euo pipefail

QUEUE_DIR="${QUEUE_DIR:-.devcontainer/notify/queue}"
mkdir -p "$QUEUE_DIR"

now_iso() { date -u +"%Y-%m-%dT%H:%M:%S.000Z" ; }

# Deterministic per-run SID so cancel-remove can target it. Overridable
# via `SID=xxx ./fake-hook.sh …` for repeated runs against the same
# fake session.
: "${SID:=$(uuidgen 2>/dev/null | tr '[:upper:]' '[:lower:]' || echo deadbeef-1111-2222-3333-444444444444)}"
SID8="${SID:0:8}"

fake_notify() {
    local NOTIF_ID="notification-${SID8}-$(date +%s%3N 2>/dev/null || date +%s000)"
    cat >> "${QUEUE_DIR}/${SID}.jsonl" <<EOF
{"ts":"$(now_iso)","sid":"${SID}","event":"notification","sender":"default","notif_id":"${NOTIF_ID}","notification_type":"idle_prompt","message":"fake-hook notification test"}
EOF
    echo "queued notification (idle_prompt) for sid=${SID} — fires instantly"
    echo "  → banner subject to your NOTIFY_CHANNELS ; daemon.log will show DISPATCH within 500 ms"
}

fake_perm() {
    local NOTIF_ID="permission_request-${SID8}-$(date +%s%3N 2>/dev/null || date +%s000)"
    cat >> "${QUEUE_DIR}/${SID}.jsonl" <<EOF
{"ts":"$(now_iso)","sid":"${SID}","event":"permission_request","sender":"default","notif_id":"${NOTIF_ID}","tool_name":"Bash","tool_input":{"command":"fake test"}}
EOF
    echo "queued permission_request for sid=${SID} — fires in ~30 s (EVENT_DELAYS_MS)"
    echo "  → cancel it with : $0 cancel ${SID}"
}

fake_stop() {
    local NOTIF_ID="stop-${SID8}-$(date +%s%3N 2>/dev/null || date +%s000)"
    cat >> "${QUEUE_DIR}/${SID}.jsonl" <<EOF
{"ts":"$(now_iso)","sid":"${SID}","event":"stop","sender":"default","notif_id":"${NOTIF_ID}","last_message_excerpt":"fake-hook stop test — recap goes here"}
EOF
    echo "queued stop for sid=${SID} — fires in ~30 s"
}

fake_cancel() {
    local target_sid="${1:-${SID}}"
    local NOTIF_ID="user_replied-${target_sid:0:8}-$(date +%s%3N 2>/dev/null || date +%s000)"
    cat >> "${QUEUE_DIR}/${target_sid}.jsonl" <<EOF
{"ts":"$(now_iso)","sid":"${target_sid}","event":"user_replied","sender":"default","notif_id":"${NOTIF_ID}"}
EOF
    echo "queued user_replied for sid=${target_sid}"
    echo "  → pre-fire pending timer cleared ; post-fire banner dismissed via notif remove"
}

case "${1:-help}" in
    notify) fake_notify ;;
    perm)   fake_perm   ;;
    stop)   fake_stop   ;;
    cancel) shift ; fake_cancel "${1:-}" ;;
    *)
        cat <<EOF
usage: $0 <notify|perm|stop|cancel [SID]>

Environment overrides :
  QUEUE_DIR   default: .devcontainer/notify/queue
  SID         default: fresh UUID per invocation

Examples :
  $0 notify                                 # instant fire
  $0 perm                                   # 30 s delay
  SID=my-test-sid $0 perm                   # fixed SID …
  SID=my-test-sid $0 cancel                 # … then cancel it
  $0 cancel deadbeef-1111-2222-3333-444…    # cancel an existing sid
EOF
        exit 1
        ;;
esac

#!/bin/bash
# Snapshot the Claude state that lives in named volumes onto the bind mount.
#
# Why it exists: ~/.claude (session transcripts, memory, plans) and
# /commandhistory are Docker *volumes*. `docker compose down -v` — the reflex
# fix for a container that will not start — deletes them, and the transcripts
# are the one thing in this stack that cannot be rebuilt. /workspace is a bind
# mount to the host, so anything written here survives every volume operation.
#
# Shape: one standalone .tar.gz per snapshot (no chain to replay at restore
# time), skipped when nothing changed, with two independent retention rules.
# Written under a .partial name and renamed once complete, so an interrupted
# run never leaves a truncated archive under a restorable name.
#
# Usage:
#   backup-state.sh              silent, detached — used by Claude Code hooks (Stop / SessionEnd)
#   backup-state.sh --verbose    print what it did
#   backup-state.sh --list       list the snapshots on disk
#   backup-state.sh --restore [archive]   restore latest (or a named archive)
#
# Always exits 0 in hook mode so a backup failure never blocks Claude.

set -uo pipefail

DEST="${CLAUDE_BACKUP_DIR:-/workspace/.devcontainer/.state-backup}"
# Retention, unioned: a snapshot is kept if EITHER rule wants it.
KEEP_LAST="${CLAUDE_BACKUP_KEEP_LAST:-10}"   # always keep the N most recent
KEEP_DAYS="${CLAUDE_BACKUP_KEEP_DAYS:-14}"   # plus the newest of each of the last X days

CLAUDE_DIR="/home/node/.claude"
STAMP="$DEST/.last-state"

# What goes in. Secrets stay out on purpose: .credentials.json and .claude.json
# carry OAuth tokens and account data, and this destination is inside a git
# working tree. They are already replicated to the claude-creds volume.
SOURCES=(
  "$CLAUDE_DIR/projects"      # session transcripts + auto-memory
  "$CLAUDE_DIR/plans"
  "$CLAUDE_DIR/commands"
  "$CLAUDE_DIR/settings.json"
  "/commandhistory"
)

VERBOSE=0
case "${1:-}" in
  --verbose) VERBOSE=1 ;;
  --list)
    ls -lh "$DEST"/claude-state-*.tar.gz 2>/dev/null || echo "No snapshots in $DEST"
    exit 0 ;;
  --restore)
    ARCHIVE="${2:-$(ls -1 "$DEST"/claude-state-*.tar.gz 2>/dev/null | tail -1)}"
    [ -n "$ARCHIVE" ] && [ -f "$ARCHIVE" ] || { echo "No snapshot to restore in $DEST" >&2; exit 1; }
    echo "Restoring $ARCHIVE"
    # Archived with paths relative to /, so / is the right extraction root.
    tar -xzf "$ARCHIVE" -C / && echo "✓ Restored. Reopen Claude to see recovered sessions."
    exit $? ;;
esac
[ "${VERBOSE:-0}" = "1" ] || [ "${CLAUDE_BACKUP_VERBOSE:-0}" != "1" ] || VERBOSE=1
log() { [ "$VERBOSE" = "1" ] && echo "$*"; return 0; }

# Hook mode returns at once: a snapshot takes seconds, and the turn stays
# "loading" until every Stop hook has exited.
if [ -z "${1:-}" ] && [ "${BACKUP_STATE_DETACHED:-0}" != "1" ]; then
  BACKUP_STATE_DETACHED=1 setsid bash "$0" </dev/null >/dev/null 2>&1 &
  exit 0
fi

mkdir -p "$DEST" 2>/dev/null || { log "⚠️  cannot create $DEST"; exit 0; }

# One snapshot at a time. A run that finds the lock taken leaves: the
# fingerprint has moved, so the next Stop catches up.
exec 9>"$DEST/.lock"
flock -n 9 || { log "a snapshot is already running"; exit 0; }
# Holding the lock, any .partial left is from an interrupted run.
rm -f "$DEST"/.claude-state-*.tar.gz.partial

# Only keep sources that exist — a fresh container has no plans/ yet.
PRESENT=()
for s in "${SOURCES[@]}"; do [ -e "$s" ] && PRESENT+=("$s"); done
[ "${#PRESENT[@]}" -gt 0 ] || { log "nothing to back up yet"; exit 0; }

# "Incremental" without a chain: fingerprint path+size+mtime of every file and
# skip the snapshot entirely when it matches the previous run. Cheap, and every
# archive on disk stays independently restorable.
FINGERPRINT="$(find "${PRESENT[@]}" -type f -printf '%p %s %T@\n' 2>/dev/null | sort | md5sum | cut -d' ' -f1)"
if [ -f "$STAMP" ] && [ "$(cat "$STAMP" 2>/dev/null)" = "$FINGERPRINT" ]; then
  log "✓ Claude state unchanged — no new snapshot"
  exit 0
fi

TS="$(date +%Y%m%d-%H%M%S)"
OUT="$DEST/claude-state-${TS}.tar.gz"
PART="$DEST/.claude-state-${TS}.tar.gz.partial"
# tar strips the leading / on create; --restore extracts with -C / to put it back.
if tar -czf "$PART" --warning=no-file-changed "${PRESENT[@]}" 2>/dev/null && mv -f "$PART" "$OUT"; then
  printf '%s\n' "$FINGERPRINT" > "$STAMP"
  log "✓ Claude state backed up → ${OUT##*/} ($(du -h "$OUT" | cut -f1))"
else
  rm -f "$PART"
  log "⚠️  snapshot failed"
  exit 0
fi

# Retention. Keep the newest $KEEP_LAST unconditionally, plus the newest
# snapshot of each of the last $KEEP_DAYS days (day read from the file name);
# delete only what neither rule wants. Newest first, so the first file seen
# for a day is that day's newest.
mapfile -t ALL < <(ls -1 "$DEST"/claude-state-*.tar.gz 2>/dev/null | sort -r)
CUTOFF="$(date -d "-$KEEP_DAYS days" +%Y%m%d)"
RANK=0
PREV_DAY=""
for f in "${ALL[@]}"; do
  DAY="${f##*/claude-state-}"
  DAY="${DAY%%-*}"
  KEEP=0
  [ "$RANK" -lt "$KEEP_LAST" ] && KEEP=1
  [ "$DAY" != "$PREV_DAY" ] && [ "$DAY" -gt "$CUTOFF" ] && KEEP=1
  PREV_DAY="$DAY"
  RANK=$((RANK + 1))
  [ "$KEEP" = "1" ] || { rm -f "$f" && log "  pruned ${f##*/}"; }
done

exit 0

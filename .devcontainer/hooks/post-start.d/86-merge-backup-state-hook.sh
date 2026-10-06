#!/usr/bin/env bash
# @name merge-backup-state-hook
# @phase post-start
# @required false
# @description Merge the backup-state hook (Stop + SessionEnd → claude/backup-state.sh) into ~/.claude/settings.json. The baked 85-merge-creds-hooks.sh merges creds-sync only; this project also snapshots transcripts/history onto the bind mount, which survives the `down -v` that volumes do not. Idempotent — dedup by command.

set -eE

SETTINGS="${DEVC_CLAUDE_HOME:-/home/node/.claude}/settings.json"
BACKUP_STATE="${DEVC_CONFIG_DIR:-/workspace/.devcontainer}/claude/backup-state.sh"

# bash, not sh: backup-state.sh uses pipefail, arrays and mapfile, and `sh …`
# ignores the shebang. Under dash it dies on line 1 of real work — silently,
# because a Stop hook's stderr goes nowhere anyone reads.
[ -x "$BACKUP_STATE" ] || { echo "- no executable $BACKUP_STATE — nothing to merge"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "⚠ python3 missing — backup-state hook not merged"; exit 0; }

mkdir -p "$(dirname "$SETTINGS")"
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
python3 -c "
import json, sys
path, cmds = sys.argv[1], sys.argv[2:]
with open(path) as f:
    s = json.load(f)
hooks = s.setdefault('hooks', {})
added = []
for event in ('Stop', 'SessionEnd'):
    entries = hooks.setdefault(event, [])
    seen = set()
    for entry in entries:
        for h in entry.get('hooks', []):
            if 'command' in h:
                seen.add(h['command'])
    for cmd in cmds:
        if cmd not in seen:
            entries.append({'matcher': '', 'hooks': [{'type': 'command', 'command': cmd}]})
            added.append(cmd.rsplit('/', 1)[-1])
if added:
    with open(path, 'w') as f:
        json.dump(s, f, indent=2)
    print('✓ state hooks merged into settings.json (%s)' % ', '.join(sorted(set(added))))
else:
    print('✓ backup-state hook already registered')
" "$SETTINGS" "bash $BACKUP_STATE"

#!/bin/bash
# Sync skills from .devcontainer/skills/ to ~/.claude/commands/
# and merge hooks from hooks.json into settings.json
# Called from post-start.sh: bash /workspace/.devcontainer/skills/sync-skills.sh

SKILLS_DIR="/workspace/.devcontainer/skills"
LOCAL_CMD="/home/node/.claude/commands"
SETTINGS="/home/node/.claude/settings.json"

echo "=== sync-skills $(date) ==="

# --- 1. Install skills: find all *.skill.md, copy to ~/.claude/commands/ ---
mkdir -p "$LOCAL_CMD"
find "$SKILLS_DIR" -name "*.skill.md" 2>/dev/null | while read -r file; do
  base=$(basename "$file")
  # Remove .skill and .local from name: hours.local.skill.md → hours.md
  dest_name=$(echo "$base" | sed 's/\.local\.skill\.md$/.md/' | sed 's/\.skill\.md$/.md/')
  cp "$file" "$LOCAL_CMD/$dest_name"
  echo "✓ skill $dest_name installed"
done

# --- 2. Merge hooks from all hooks.json files ---
# Create empty settings.json if missing (Claude CLI works without it, but we need it to merge hooks)
if [ ! -f "$SETTINGS" ]; then
  mkdir -p "$(dirname "$SETTINGS")"
  echo '{}' > "$SETTINGS"
  echo "✓ created empty $SETTINGS"
fi

if command -v python3 >/dev/null 2>&1; then
  python3 -c "
import json, glob, os

settings_path = '$SETTINGS'
skills_dir = '$SKILLS_DIR'

with open(settings_path) as f:
    settings = json.load(f)

merged_hooks = settings.get('hooks', {})
changed = False

for hf in glob.glob(os.path.join(skills_dir, '**/hooks.json'), recursive=True):
    with open(hf) as f:
        skill_hooks = json.load(f)
    for event, handlers in skill_hooks.items():
        if event not in merged_hooks:
            merged_hooks[event] = handlers
            changed = True
        else:
            existing_cmds = set()
            for entry in merged_hooks[event]:
                for h in entry.get('hooks', []):
                    if 'command' in h:
                        existing_cmds.add(h['command'])
            for handler in handlers:
                hooks_list = handler.get('hooks', [])
                if hooks_list:
                    cmd = hooks_list[0].get('command', '')
                    if cmd and cmd not in existing_cmds:
                        merged_hooks[event].append(handler)
                        changed = True

if changed:
    settings['hooks'] = merged_hooks
    with open(settings_path, 'w') as f:
        json.dump(settings, f, indent=2)
    print('✓ hooks merged into settings.json')
else:
    print('✓ hooks already up to date')
"
fi

echo "=== sync-skills done ==="

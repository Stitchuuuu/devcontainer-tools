#!/usr/bin/env bash
# @name skills-sync
# @phase post-start
# @required false
# @description Skills — sync skill commands and hooks via sync-skills.sh.

set -eE

if [ -f /workspace/.devcontainer/skills/sync-skills.sh ]; then
  bash /workspace/.devcontainer/skills/sync-skills.sh
fi

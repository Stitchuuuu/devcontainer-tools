#!/bin/bash
# tokens skill — Stop hook entry point.
# Reads stdin JSON payload from Claude Code, extracts session_id + transcript_path,
# resolves project identity, invokes lib/capture.py.
# Must never crash a Claude session: swallow all errors.

set -u

SKILL_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck disable=SC1091
. "$SKILL_DIR/lib/project-id.sh"

INPUT=$(cat)
SID=$(echo "$INPUT" | grep -o '"session_id":"[^"]*"' | head -1 | cut -d'"' -f4)
TRANSCRIPT=$(echo "$INPUT" | grep -o '"transcript_path":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$SID" ] || [ -z "$TRANSCRIPT" ]; then
  exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
  exit 0
fi

python3 "$SKILL_DIR/lib/capture.py" \
  --session "$SID" \
  --transcript "$TRANSCRIPT" \
  --project-root "$PROJECT_ROOT" \
  --project-id "$PROJECT_ID" \
  --host-workspace "${HOST_WORKSPACE_PATH:-}" 2>/dev/null || true

exit 0

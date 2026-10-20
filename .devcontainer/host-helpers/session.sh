#!/usr/bin/env bash
#
# .devcontainer/host-helpers/session.sh — `wtf session …`: the archived Claude
# conversations of THIS project (the directory you run it from), as devc-sweep
# archives them: one archive per session, out of /resume until restored.
#
#   session.sh archived [--match T]         list: date, size, title
#   session.sh show <id>                    read one without restoring it
#   session.sh restore                      fzf picker: type to search, preview,
#                                           Tab to pick several, Ctrl-A all, Enter
#   session.sh restore <id>…                these ones
#   session.sh restore --all [--match T] [--yes]
#
# Devcontainer only: the conversations live in its config volume.
set -u

if [ ! -f /.dockerenv ]; then
    echo "wtf session: run it inside the devcontainer — the conversations live in its" >&2
    echo "  config volume, not on the Mac." >&2
    exit 1
fi
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
if command -v devc-sweep >/dev/null 2>&1; then
    SWEEP="devc-sweep"
elif [ -x "$ROOT/packages/devcontainer-sandbox/bin/devc-sweep" ]; then
    SWEEP="$ROOT/packages/devcontainer-sandbox/bin/devc-sweep"
else
    echo "wtf session: no devc-sweep in this container (image older than 1.12) — rebuild on 1.12+" >&2
    exit 1
fi
exec "$SWEEP" "$@"

#!/usr/bin/env bash
# scan-deps-suggest-stale.sh — Claude Code SessionStart hook.
#
# Detects when project npm manifests are newer than the firewall
# allowlist file (firewall/domains.d/npm.txt) and injects a
# SessionStart additionalContext telling Claude to propose
# /scan-deps before any dependency-touching work.
#
# Mirrors the staleness check from post-start.sh:166-207 — kept as
# a standalone script per the v1.3.0 hook pattern (each hook lives
# self-contained in its skill dir, no shared library).
#
# Exit code is always 0 — a non-zero exit on a SessionStart hook
# would block session start, which is unacceptable for a soft nudge.
#
# Stdin contract (Claude Code SessionStart hook):
#   {"session_id": "...", "transcript_path": "...", "cwd": "...",
#    "hook_event_name": "SessionStart", "source": "startup|resume|clear"}
# We don't consume stdin — the check is purely filesystem-based.

set -uo pipefail

command -v python3 >/dev/null 2>&1 || exit 0

stale=$(python3 << 'PY'
import os, subprocess

DOMAINS_D = "/workspace/.devcontainer/firewall/domains.d"
MANIFEST = "package.json"
ECO_FILE = "npm.txt"

def find_manifests(depths=(3, 5, 8, 10)):
    for d in depths:
        res = subprocess.run(
            ['find', '/workspace', '-maxdepth', str(d), '-type', 'f',
             '-name', MANIFEST,
             '-not', '-path', '*/node_modules/*',
             '-not', '-path', '*/vendor/*',
             '-not', '-path', '*/.git/*',
             '-not', '-path', '*/__pycache__/*',
             '-not', '-path', '*/research-bundles/*'],
            capture_output=True, text=True)
        ms = [p for p in res.stdout.splitlines() if p]
        if ms:
            return ms
    return []

manifests = find_manifests()
if not manifests:
    print("no")
    raise SystemExit

target = os.path.join(DOMAINS_D, ECO_FILE)
verdict = "no"
if not os.path.exists(target):
    verdict = "yes"
else:
    try:
        tmt = os.path.getmtime(target)
    except OSError:
        verdict = "yes"
    else:
        for m in manifests:
            try:
                if os.path.getmtime(m) > tmt:
                    verdict = "yes"
                    break
            except OSError:
                continue
print(verdict)
PY
)

[ "$stale" = "yes" ] || exit 0

python3 << 'PY'
import json
msg = (
    "scan-deps signal: project npm manifests changed since the last "
    "firewall extract (.devcontainer/firewall/domains.d/npm.txt is "
    "stale or missing). Before any dependency-touching work in this "
    "session, propose /scan-deps to the user. Do not run it "
    "autonomously. If the user declines or postpones, drop the topic "
    "and don't re-raise it the same session."
)
print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": msg,
    }
}))
PY

exit 0

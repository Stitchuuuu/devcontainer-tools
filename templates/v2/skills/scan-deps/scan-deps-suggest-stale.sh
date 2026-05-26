#!/usr/bin/env bash
# scan-deps-suggest-stale.sh — Claude Code SessionStart hook.
#
# Detects when project manifests are newer than the corresponding firewall
# allowlist file (firewall/domains.d/<eco>.txt) and injects a SessionStart
# additionalContext telling Claude to propose /scan-deps before any
# dependency-touching work.
#
# Multi-ecosystem : npm (package.json → npm.txt) + composer
# (composer.lock → composer.txt). Each ecosystem is checked independently
# and the resulting banner lists the stale ones.
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
ECOSYSTEMS = [
    # (eco_name, manifest_filename, output_filename)
    ("npm",      "package.json",  "npm.txt"),
    ("composer", "composer.lock", "composer.txt"),
]

def find_manifests(name, depths=(3, 5, 8, 10)):
    for d in depths:
        res = subprocess.run(
            ['find', '/workspace', '-maxdepth', str(d), '-type', 'f',
             '-name', name,
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

stale_ecos = []
for eco, manifest_name, out_name in ECOSYSTEMS:
    manifests = find_manifests(manifest_name)
    if not manifests:
        continue
    target = os.path.join(DOMAINS_D, out_name)
    if not os.path.exists(target):
        stale_ecos.append(eco)
        continue
    try:
        tmt = os.path.getmtime(target)
    except OSError:
        stale_ecos.append(eco)
        continue
    for m in manifests:
        try:
            if os.path.getmtime(m) > tmt:
                stale_ecos.append(eco)
                break
        except OSError:
            continue

print(",".join(stale_ecos))
PY
)

[ -n "$stale" ] || exit 0

STALE_ECOS="$stale" python3 << 'PY'
import json, os

ecos = os.environ["STALE_ECOS"].split(",")
ecos_str = " + ".join(ecos)
files_str = ", ".join(f".devcontainer/firewall/domains.d/{e}.txt" for e in ecos)
msg = (
    f"scan-deps signal: project manifests changed since the last firewall "
    f"extract ({ecos_str} — {files_str} stale or missing). Before any "
    f"dependency-touching work in this session, propose /scan-deps to the "
    f"user. Do not run it autonomously. If the user declines or postpones, "
    f"drop the topic and don't re-raise it the same session."
)
print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": msg,
    }
}))
PY

exit 0

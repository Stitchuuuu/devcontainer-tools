#!/usr/bin/env python3
"""
Patch VS Code core to make FocusMode.Force the default in BaseWindow.focus().

Why
---
VS Code's `handleProtocolUrl` (and several other focus callsites) uses
`FocusMode.Transfer` by default. On macOS, only `FocusMode.Force`
triggers `electron.app.focus({ steal: true })` — which under the hood
calls `[[AtomApplication sharedApplication] activateIgnoringOtherApps:YES]`
(confirmed via electron/electron shell/browser/browser_mac.mm).

That SELF-activate via NSApp is the ONLY reliable way to switch cross-Space
fullscreen VS Code windows — we validated empirically that `open -a
"Visual Studio Code" <folder>` works precisely because LaunchServices
sends the AppleEvent to VS Code main, which invokes its own SELF-activate.
Any EXTERNAL activate (NSRunningApplication from another process) is
subject to macOS foreground rights restrictions and doesn't switch Spaces.

Impact
------
- `vscode-remote://` protocol URLs now bring the correct devcontainer
  window forward from any Space (including other fullscreen Spaces).
- `code --reuse-window` and similar entry paths get the aggressive
  activation on macOS.

Trade-offs
----------
- VS Code becomes slightly more aggressive at stealing focus. On macOS
  the OS still ultimately decides, so the impact is limited to focus
  requests that are already user-initiated.

Idempotency + fallback
----------------------
- MARKER `// devcontainer-focus-patch-v1` embedded on patch success.
- Multiple regex patterns tried (unminified TS-derived, minified with
  enum inlining). First match wins.
- On regex miss : RED banner, exit 1. Auto-update likely bumped
  the compiled output — anchor needs a revisit.

Post-mutation
-------------
- `codesign --sign - --deep --force` re-adhoc-signs the whole bundle.
  Required else macOS refuses to launch modified app.

Persistence caveat
------------------
VS Code auto-update overwrites patched files. Re-run this script after
each VS Code version bump on host.

Usage
-----
    python3 focus-mode-force-default.py
    # or with explicit bundle path :
    python3 focus-mode-force-default.py "/Applications/Visual Studio Code.app"

Exit codes
----------
0 — patched or already-patched (idempotent)
1 — regex miss / file missing / codesign failed
"""

import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _common import (
    RED, YELLOW, GREEN, BOLD, RESET,
    banner, resolve_app_bundle, app_resources, resolve_target_file,
    ensure_writable, codesign_bundle, snapshot_bundle_if_env,
)


MARKER = "// devcontainer-focus-patch-v1"

# Patterns tried in order. Each = (name, regex, replacement).
# First successful match wins. The replacement must not include the
# marker — we append MARKER separately to keep the substitution readable.
#
# Anchor rationale : the switch statement's default expression
# `options?.mode ?? <default>` is the surface we change. We look for
# both the pretty form (FocusMode enum names preserved) and the minified
# form (enum inlined as numeric literal).
#
# Minified anchor was validated empirically as unique in main.js on
# VS Code 1.126.0 (single match in the whole ~1.7MB bundle).
PATTERNS = [
    # Pretty form — enum names preserved (dev / debug builds, forks
    # publishing unminified vs/platform/ modules).
    (
        "pretty",
        re.compile(r'(\?\?\s*)FocusMode\.Transfer(\s*\))'),
        r'\1FocusMode.Force\2',
    ),
    # Minified form — TypeScript compiler inlines enum values
    # (Transfer=0, Notify=1, Force=2). Tight anchor on the full
    # `focus(e){switch(e?.mode??0){` prefix keeps it unique bundle-wide.
    (
        "minified-0-to-2",
        re.compile(r'(focus\(e\)\{switch\(e\?\.mode\?\?)0(\)\{)'),
        r'\g<1>2\g<2>',
    ),
]

# Anchor substrings used both to pick the right file and to guard against
# false positives (the file must look like it hosts BaseWindow.focus()).
SANITY_MINI = 'focus(e){switch(e?.mode??'
SANITY_PRETTY = 'FocusMode.Transfer'


def find_target(app_res):
    # Modern VS Code stable (≥ ~1.100) bundles the whole electron-main
    # layer into out/main.js. The old per-module compiled paths are kept
    # as fallback for pre-bundle builds and non-bundled forks.
    candidates = [
        "out/main.js",
        "out/vs/platform/windows/electron-main/windowImpl.js",
        "out/vs/platform/window/electron-main/window.js",
    ]
    for rel in candidates:
        p = app_res / rel
        if not p.is_file():
            continue
        try:
            content = p.read_text(errors="ignore")
        except Exception:
            continue
        if SANITY_MINI in content or SANITY_PRETTY in content:
            return p
    # Last-resort scan : any JS under out/ that carries a BaseWindow
    # focus() signature. Kept narrow to avoid touching workbench code.
    for c in (app_res / "out").rglob("*.js"):
        try:
            content = c.read_text(errors="ignore")
        except Exception:
            continue
        if SANITY_MINI in content or (
            SANITY_PRETTY in content and "class BaseWindow" in content
        ):
            return c
    return None


def main():
    app = resolve_app_bundle(sys.argv)
    app_res = app_resources(app)

    # Opt-in full-bundle snapshot before any mutation. Cheap safety net
    # when iterating on tests — no App Store re-download if codesign
    # botches something.
    snapshot_bundle_if_env(app)

    target = find_target(app_res)
    if not target:
        banner(
            "TARGET FILE NOT FOUND",
            "No main.js / windowImpl.js hosting BaseWindow.focus() found",
            [
                f"Searched under : {app_res / 'out'}",
                "VS Code layout changed — anchor needs update.",
            ],
        )
        sys.exit(1)

    print(f"→ Target : {target}")

    text = target.read_text()
    if MARKER in text:
        print(f"{YELLOW}⚠  Already patched (marker present) — no-op.{RESET}")
        sys.exit(0)

    ensure_writable(target)

    matched_pattern = None
    for name, regex, replacement in PATTERNS:
        new_text, n = regex.subn(replacement, text, count=1)
        if n > 0:
            matched_pattern = name
            text = new_text
            break

    if not matched_pattern:
        banner(
            "REGEX MISS",
            f"None of {[p[0] for p in PATTERNS]} matched in {target.name}",
            [
                "VS Code has bumped its compiled focus() output.",
                "Manual inspection required — search for 'BaseWindow' + 'FocusMode' + 'switch'",
                "and craft a new pattern.",
            ],
        )
        sys.exit(1)

    # Insert MARKER right after the modified line for easy grep detection.
    # Find the injected replacement location and append a comment marker.
    # Simplest : append at the end of the target file. Idempotency check
    # at the top already catches this.
    text = text.rstrip() + f"\n{MARKER}\n"

    # Backup once (first-time patch only)
    backup = target.with_suffix(target.suffix + ".bak-devcontainer-focus")
    if not backup.exists():
        backup.write_text(target.read_text())
        print(f"→ Backup written : {backup.name}")

    target.write_text(text)
    print(f"{GREEN}✓ Patched (pattern={matched_pattern}) : {target}{RESET}")

    if not codesign_bundle(app):
        sys.exit(1)

    print(f"\n{GREEN}{BOLD}✓ Done. Restart VS Code to pick up the patch.{RESET}")


if __name__ == "__main__":
    main()

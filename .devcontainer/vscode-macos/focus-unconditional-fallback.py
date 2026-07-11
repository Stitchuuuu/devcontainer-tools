#!/usr/bin/env python3
"""
Fallback patch : unconditional SELF-activate at the top of BaseWindow.focus().

When to use
-----------
Only if `focus-mode-force-default.py` has been applied AND the empirical
test (protocol URL from another Space) still fails to switch cross-Space
fullscreen. Meaning : some callers pass an explicit `FocusMode.Transfer`
so flipping the default is not enough.

What it does
------------
Prepends `B && vn.app.focus({ steal: true })` at the very start of the
`focus(e)` method body — before the switch. Every entry into focus() now
triggers the SELF-activate on macOS regardless of the mode passed.

Why we reuse local aliases (B, vn)
----------------------------------
In VS Code 1.126.0's bundled `out/main.js`, esbuild wraps modules in IIFE
closures. `vn` (electron namespace) and `B` (macOS platform check) are
in scope inside the same method — they appear in `case 2:` a few chars
downstream. Reusing them is stable ; trying to inject the same call into
`handleProtocolUrl` fails because `vn` is out of scope there. Same net
effect (any focus() call SELF-activates on macOS), zero alias-resolution
risk.

Stackability
------------
Distinct MARKER (`// devcontainer-focus-fallback-v1`) so it coexists with
the primary patch's marker. The primary patch's `??2` swap is left intact.

Usage
-----
    sudo python3 focus-unconditional-fallback.py
    # or with explicit bundle path :
    sudo python3 focus-unconditional-fallback.py "/Applications/Visual Studio Code.app"

Exit codes
----------
0 — patched or already-patched (idempotent)
1 — target file missing / regex miss / codesign failed
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _common import (
    RED, YELLOW, GREEN, BOLD, RESET,
    banner, resolve_app_bundle, app_resources,
    ensure_writable, codesign_bundle, snapshot_bundle_if_env,
)


MARKER = "// devcontainer-focus-fallback-v1"

# Sanity substrings — same as the primary patch. The target file must
# host BaseWindow.focus() ; we look for both mini and pretty forms.
SANITY_MINI = 'focus(e){switch(e?.mode??'
SANITY_PRETTY = 'FocusMode.Transfer'

# Regex : match the very start of the focus() method body, then prepend
# the SELF-activate. We keep two variants — the primary patch may already
# have flipped `??0` to `??2`, so both must be accepted.
PATTERNS = [
    # After-primary form (??2) — most common when this fallback runs.
    (
        "post-primary-mini",
        re.compile(r'(focus\(e\)\{)(switch\(e\?\.mode\?\?2\)\{)'),
        r'\1B&&vn.app.focus({steal:!0}),\2',
    ),
    # Fresh mini form (??0) — allows fallback-only application (skip
    # primary, or primary regex miss and manual override).
    (
        "fresh-mini",
        re.compile(r'(focus\(e\)\{)(switch\(e\?\.mode\?\?0\)\{)'),
        r'\1B&&vn.app.focus({steal:!0}),\2',
    ),
    # Pretty form — unminified builds. Uses the enum name for the guard
    # expression ; we assume the same `[[NSApp]] app.focus({steal:true})`
    # call already exists in the file so the alias reference stays valid
    # in this narrow reuse. In pretty builds `electron.app.focus` is the
    # spelling, so we inject that directly rather than an alias.
    (
        "pretty",
        re.compile(
            r'(focus\(options\?\s*:\s*\{[^}]*\}\)\s*\{[\s\n]*)(switch\s*\(options\?\.mode\s*\?\?)'
        ),
        r"\1if(isMacintosh)electron.app.focus({steal:true});\2",
    ),
]


def find_target(app_res):
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

    # Same opt-in snapshot as the primary. Idempotent per (bundle, version)
    # so running both scripts back-to-back doesn't duplicate the copy.
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
        print(f"{YELLOW}⚠  Fallback already applied (marker present) — no-op.{RESET}")
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
                "Check whether the primary patch changed the anchor shape,",
                "or VS Code bumped its compiled focus() output.",
            ],
        )
        sys.exit(1)

    text = text.rstrip() + f"\n{MARKER}\n"

    # Distinct backup suffix — do not clobber the primary patch's backup.
    backup = target.with_suffix(target.suffix + ".bak-devcontainer-focus-fb")
    if not backup.exists():
        backup.write_text(target.read_text())
        print(f"→ Backup written : {backup.name}")

    target.write_text(text)
    print(f"{GREEN}✓ Patched (pattern={matched_pattern}) : {target}{RESET}")

    if not codesign_bundle(app):
        sys.exit(1)

    print(f"\n{GREEN}{BOLD}✓ Done. Restart VS Code to pick up the fallback.{RESET}")


if __name__ == "__main__":
    main()

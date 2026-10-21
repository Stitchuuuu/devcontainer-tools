#!/usr/bin/env python3
"""
Patch VS Code core so ResourceContextKey.set() only skips its write when
the shared context keys still hold that resource.

Symptom
-------
Right-click a file in the explorer : the menu sometimes lacks
"Reveal in Finder", "Copy Relative Path" and other entries gated on
`resourceScheme` / `resourcePath`. Right-clicking the same file again
never fixes it — only right-clicking a *different* file does.

Root cause (verified in source 1.127.0 and minified 1.142.0)
------------------------------------------------------------
- ExplorerView and OpenEditorsView each create a ResourceContextKey on the
  SAME context key service (the view container's, not the ViewPane's
  scoped one) — explorerView.ts / openEditorsView.ts.
- On every active-tab change, Open Editors resets then re-sets those keys
  to the active tab's resource. A tab with no file (e.g. the Claude Code
  panel) leaves them empty.
- On right-click the explorer calls `resourceContext.set(uri)`, but
  `ResourceContextKey.set()` returns early when `uri` equals ITS OWN last
  value (contextkeys.ts). The keys keep Open Editors' value and the menu
  `when` clauses fail. A different file = different URI = rewrite.

Fix
---
Keep the early return, but compare against the truth as well :

    !isEqual(this._value, v)
  → !(isEqual(this._value, v) && this._resourceKey.get() == (v ? v.toString() : null))

`_resourceKey.get()` reads the shared service, so it sees what the other
view wrote. Cost : one string compare (URI.toString() is cached). `==`
on purpose : a removed key reads `undefined`, a cleared one `null`.
Fixes every ResourceContextKey sharing a scope, Open Editors included.

Earlier revision of this patch (marker `devcontainer-explorer-ctxkeys-v1`,
a reset() before the explorer's set()) is reverted in place if present.

Workaround without the patch : collapse the "Open Editors" section.

Post-mutation
-------------
- The `vs/workbench/workbench.desktop.main.js` checksum in product.json is
  recomputed, else VS Code shows the "installation appears to be corrupt"
  banner.
- `codesign --sign - --deep --force` re-adhoc-signs the whole bundle.

Persistence caveat
------------------
VS Code auto-update overwrites patched files. Re-run after each update.

Usage
-----
    python3 resource-context-key-stale-guard.py
    python3 resource-context-key-stale-guard.py "/Applications/Visual Studio Code.app"

Exit codes
----------
0 — patched or already-patched (idempotent)
1 — regex miss / file missing / codesign failed
"""

import base64
import hashlib
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _common import (
    YELLOW, GREEN, BOLD, RESET,
    banner, resolve_app_bundle, app_resources,
    ensure_writable, codesign_bundle, snapshot_bundle_if_env,
)


MARKER = "/*devcontainer-resource-ctxkey-guard-v1*/"
BACKUP_SUFFIX = ".bak-devcontainer-resource-ctxkey"
# Earlier revision : reset() before the explorer's set(). Stripped on sight.
LEGACY_SNIPPET = "/*devcontainer-explorer-ctxkeys-v1*/this.resourceContext.reset(),"

TARGET_REL = "out/vs/workbench/workbench.desktop.main.js"
CHECKSUM_KEY = "vs/workbench/workbench.desktop.main.js"

# ResourceContextKey.set() — anchor validated as unique in
# workbench.desktop.main.js on VS Code 1.127.0 and 1.142.0. The isEqual
# helper is minified (`He` / `ke`), hence captured ; property names are not.
PATTERN = re.compile(
    r'(set\((\w+)\)\{\2=\2\?\?void 0,!)'
    r'(\w+\(this\._value,\2\))'
    r'(&&\(this\._value=\2,this\._contextKeyService\.bufferChangeEvents\()'
)
REPLACEMENT = (
    r'\1(\3&&this._resourceKey.get()==(\2?\2.toString():null))'
    + MARKER + r'\4'
)


def checksum(path):
    """product.json format : base64 sha256, no `=` padding."""
    digest = hashlib.sha256(path.read_bytes()).digest()
    return base64.b64encode(digest).decode().rstrip("=")


def update_product_checksum(app_res, target):
    """Rewrite only the checksum value in product.json (no reformat).
    Returns True if the file changed."""
    product = app_res / "product.json"
    text = product.read_text(encoding="utf-8")
    entry = re.compile(r'("' + re.escape(CHECKSUM_KEY) + r'"\s*:\s*")([^"]*)(")')
    m = entry.search(text)
    if not m:
        print(f"{YELLOW}No checksum for {CHECKSUM_KEY} in product.json — skipped.{RESET}")
        return False
    new = checksum(target)
    if m.group(2) == new:
        return False
    backup = product.with_name(product.name + BACKUP_SUFFIX)
    if not backup.exists():
        backup.write_text(text, encoding="utf-8")
        print(f"→ Backup written : {backup.name}")
    ensure_writable(product)
    product.write_text(text[:m.start(2)] + new + text[m.end(2):], encoding="utf-8")
    print(f"{GREEN}✓ product.json checksum updated{RESET}")
    return True


def patch(app_res):
    """Patch the workbench bundle + its checksum. Returns True if the
    bundle changed (codesign needed)."""
    target = app_res / TARGET_REL
    if not target.is_file():
        banner(
            "TARGET FILE NOT FOUND",
            f"{target} missing",
            ["VS Code layout changed — target path needs update."],
        )
        sys.exit(1)

    print(f"→ Target : {target}")
    text = target.read_text(encoding="utf-8")
    legacy = LEGACY_SNIPPET in text
    if legacy:
        text = text.replace(LEGACY_SNIPPET, "", 1)
        print("→ Earlier revision found — reverted in place")

    if MARKER in text and not legacy:
        print(f"{YELLOW}Already patched (marker present) — no-op.{RESET}")
        # Still heal the checksum in case a previous run died in between.
        return update_product_checksum(app_res, target)

    new_text, n = PATTERN.subn(REPLACEMENT, text)
    if n != 1:
        banner(
            "REGEX MISS" if n == 0 else "AMBIGUOUS ANCHOR",
            f"{n} match(es) for ResourceContextKey.set() anchor in {target.name}",
            [
                "VS Code has changed its compiled ResourceContextKey.",
                "Search for 'this._value,' + '_contextKeyService.bufferChangeEvents(' and",
                "craft a new pattern.",
            ],
        )
        sys.exit(1)

    ensure_writable(target)
    backup = target.with_name(target.name + BACKUP_SUFFIX)
    if not backup.exists():
        backup.write_text(text, encoding="utf-8")
        print(f"→ Backup written : {backup.name}")

    target.write_text(new_text, encoding="utf-8")
    print(f"{GREEN}✓ Patched : {target}{RESET}")
    update_product_checksum(app_res, target)
    return True


def main():
    app = resolve_app_bundle(sys.argv)
    app_res = app_resources(app)
    snapshot_bundle_if_env(app)

    if patch(app_res) and not codesign_bundle(app):
        sys.exit(1)

    print(f"\n{GREEN}{BOLD}✓ Done. Restart VS Code to pick up the patch.{RESET}")


if __name__ == "__main__":
    main()

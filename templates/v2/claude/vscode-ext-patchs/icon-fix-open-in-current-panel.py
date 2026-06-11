#!/usr/bin/env python3
"""
Patches the Claude Code VS Code extension to add a "primary" value to the
`claudeCode.preferredLocation` setting. When set to "primary", clicking the
Claude icon (top-right of editor title bar or status bar item) — and the
"+ New session" button in the Claude UI, which also routes through
`editor.openLast` — opens each new Claude session as a tab in the currently
active editor column, instead of splitting into a new column.

Strategy
--------
Generic, version-resilient: search for stable anchors (Claude command names —
public API surface, unlikely to change across minor versions) and capture the
minified variable names dynamically via regex backreferences. This means the
patch survives bumps like 2.1.145 → 2.1.146 even when the minified bundle
renames `R0` → `Q7`, etc.

Idempotent: re-running is a no-op (markers detect prior application).

Exit codes
----------
- 0 : all steps applied or already patched
- 1 : regex miss, file missing, or JSON shape broken (red ANSI banner to
      stderr via _common.banner). The orchestrator (run-all.sh) absorbs
      this and keeps the container build green; the diagnostic is the
      per-script exit code visible in build logs.

Usage
-----
    icon-fix-open-in-current-panel.py [EXT_DIR]

If EXT_DIR is omitted, auto-discovers the latest
~/.vscode-server/extensions/anthropic.claude-code-*-{arch} directory.
"""

import json
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import RED, YELLOW, GREEN, BOLD, RESET, banner, resolve_ext_dir, check_files


IMPACT_LINES = [
    "→ The 'primary' value for claudeCode.preferredLocation will be",
    "  UNAVAILABLE. The Claude icon will use original Claude Code",
    "  behavior (split column / sidebar) instead of opening Claude",
    "  in the active editor column.",
    "Likely cause: CLAUDE_CODE_VERSION was bumped and the minified",
    "  patterns no longer match. Review and update the regexes in",
    "  .devcontainer/claude/vscode-ext-patchs/icon-fix-open-in-current-panel.py.",
]


def patch_package_json(pkg_path):
    p = json.loads(pkg_path.read_text())
    try:
        loc = p["contributes"]["configuration"]["properties"]["claudeCode.preferredLocation"]
    except KeyError as e:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               f"package.json: schema path missing ({e})",
               IMPACT_LINES)
        sys.exit(1)
    if "primary" in loc["enum"]:
        print(f"{YELLOW}[1/4]{RESET} package.json — already patched")
        return
    loc["enum"].append("primary")
    loc.setdefault("enumDescriptions", []).append("Primary Editor (Active Column)")
    pkg_path.write_text(json.dumps(p, indent=2))
    print(f"{GREEN}[1/4]{RESET} package.json — added 'primary' to preferredLocation enum")


def patch_editor_open_guard(content):
    """Prevent `editor.open` from silently overwriting the preferredLocation
    setting when it's "primary" (which would otherwise mutate it to "panel")."""
    marker = 'getConfiguration("claudeCode").get("preferredLocation")!=="primary"'
    if marker in content:
        print(f"{YELLOW}[2/4]{RESET} extension.js editor.open guard — already patched")
        return content
    pat = re.compile(
        r'registerCommand\("claude-vscode\.editor\.open",async\((\w+),(\w+),(\w+)\)=>\{'
        r'if\((\w+)!==(\w+)\.ViewColumn\.Active\)(\w+)\.setPreferredLocation\("panel"\);'
    )
    m = pat.search(content)
    if not m:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: editor.open guard pattern not found",
               IMPACT_LINES)
        sys.exit(1)
    a1, a2, a3, c1, vs, st = m.groups()
    replacement = (
        f'registerCommand("claude-vscode.editor.open",async({a1},{a2},{a3})=>{{'
        f'if({c1}!=={vs}.ViewColumn.Active'
        f'&&{vs}.workspace.getConfiguration("claudeCode").get("preferredLocation")!=="primary")'
        f'{st}.setPreferredLocation("panel");'
    )
    new_content = pat.sub(replacement, content, count=1)
    print(f"{GREEN}[2/4]{RESET} extension.js editor.open guard — applied (vscode={vs}, state={st})")
    return new_content


def patch_editor_openLast(content):
    """Redirect `editor.openLast` (= the icon and status bar handler) to
    `primaryEditor.open` when the setting is "primary"."""
    idx = content.find('"claude-vscode.editor.openLast"')
    marker = '"claude-vscode.primaryEditor.open");return}await'
    if idx != -1 and marker in content[idx:idx + 600]:
        print(f"{YELLOW}[3/4]{RESET} extension.js editor.openLast — already patched")
        return content
    pat = re.compile(
        r'registerCommand\("claude-vscode\.editor\.openLast",async\(\)=>\{'
        r'if\((\w+)\.getPreferredLocation\(\)==="sidebar"\)\{'
        r'await (\w+)\.commands\.executeCommand\("claude-vscode\.sidebar\.open"\);return\}'
        r'await \2\.commands\.executeCommand\("claude-vscode\.editor\.open"\)'
    )
    m = pat.search(content)
    if not m:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: editor.openLast pattern not found",
               IMPACT_LINES)
        sys.exit(1)
    st, vs = m.groups()
    replacement = (
        f'registerCommand("claude-vscode.editor.openLast",async()=>{{'
        f'if({st}.getPreferredLocation()==="sidebar"){{'
        f'await {vs}.commands.executeCommand("claude-vscode.sidebar.open");return}}'
        f'if({vs}.workspace.getConfiguration("claudeCode").get("preferredLocation")==="primary"){{'
        f'await {vs}.commands.executeCommand("claude-vscode.primaryEditor.open");return}}'
        f'await {vs}.commands.executeCommand("claude-vscode.editor.open")'
    )
    new_content = pat.sub(replacement, content, count=1)
    print(f"{GREEN}[3/4]{RESET} extension.js editor.openLast — applied (state={st}, vscode={vs})")
    return new_content


def patch_primaryEditor_use_active_column(content):
    """Force `primaryEditor.open` to resolve the active editor column itself
    (via `tabGroups.activeTabGroup.viewColumn`) and pass that integer to
    `createPanel`, instead of the symbolic `ViewColumn.Active`. Without this,
    VS Code splits into a new editor column on every click when a webview of
    the same viewType already exists in the active column."""
    anchor = '"claude-vscode.primaryEditor.open"'
    idx = content.find(anchor)
    new_marker = 'tabGroups.activeTabGroup'
    if idx != -1 and new_marker in content[idx:idx + 400]:
        print(f"{YELLOW}[4/4]{RESET} extension.js primaryEditor.open active column — already patched")
        return content
    # Tolerant regex: matches both the original body and the previous-iteration
    # dedup body, by accepting any content between the body open `{` and the
    # final `createPanel(A,R,vs.ViewColumn.Active)})`.
    pat = re.compile(
        r'registerCommand\("claude-vscode\.primaryEditor\.open",async\((\w+),(\w+)\)=>\{'
        r'[\s\S]*?'
        r'(\w+)\.createPanel\(\1,\2,(\w+)\.ViewColumn\.Active\)\}\)'
    )
    m = pat.search(content)
    if not m:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: primaryEditor.open pattern not found",
               IMPACT_LINES)
        sys.exit(1)
    a, r, pm, vs = m.groups()
    replacement = (
        f'registerCommand("claude-vscode.primaryEditor.open",async({a},{r})=>{{'
        f'let g={vs}.window.tabGroups.activeTabGroup;'
        f'{pm}.createPanel({a},{r},g&&g.viewColumn?g.viewColumn:{vs}.ViewColumn.Active)}})'
    )
    new_content = pat.sub(replacement, content, count=1)
    print(f"{GREEN}[4/4]{RESET} extension.js primaryEditor.open active column — applied (panelMgr={pm}, vscode={vs})")
    return new_content


def main():
    ext_dir = resolve_ext_dir(sys.argv)
    check_files(ext_dir, ["package.json", "extension.js"])
    pkg = ext_dir / "package.json"
    js = ext_dir / "extension.js"

    print(f"Patching Claude Code extension at: {ext_dir}")

    patch_package_json(pkg)

    content = js.read_text()
    original = content

    content = patch_editor_open_guard(content)
    content = patch_editor_openLast(content)
    content = patch_primaryEditor_use_active_column(content)

    if content != original:
        js.write_text(content)

    print(f"{GREEN}{BOLD}✓ icon-fix-open-in-current-panel patches complete{RESET}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Patches the Claude Code VS Code extension to route every Claude panel
entry-point through the active editor column, with the new tab landing at
the end of the active tab group.

Patches applied (6 steps)
-------------------------
[1/6] package.json — add "primary" value to `claudeCode.preferredLocation`
      enum (+ enumDescription).
[2/6] extension.js editor.open guard — skip `setPreferredLocation("panel")`
      when the setting is "primary", so opening a session doesn't mutate
      the user's choice.
[3/6] extension.js editor.openLast — when the setting is "primary", route
      to the new `primaryEditor.open` command (icon, status bar item,
      command palette all go through this).
[4/6] extension.js primaryEditor.open — resolve the active column from
      `tabGroups.activeTabGroup.viewColumn` (an integer) instead of the
      symbolic `ViewColumn.Active`, so re-clicking doesn't split.
[5/6] extension.js "+" button handler — add the 4th `viewColumn` arg to
      the `new_conversation_tab` → `editor.open` call, so the webview
      header button opens in the active column instead of splitting.
[6/6] extension.js editor.openLast — after `primaryEditor.open`, invoke
      `workbench.action.moveEditorToEnd` so the new tab lands at the end
      of the active tab group (right of all other tabs). Requires VS
      Code ≥ 1.109 (January 2026 release — PR microsoft/vscode#284999,
      milestone "December 2025", merged 2025-12-28). This wrapper is the
      only extension-callable path: `moveActiveEditor` is
      workbench-internal, `tabGroups.move()` doesn't exist (only
      `close()`). Not listed in the official commands reference page,
      but verified in the VS Code source.

Strategy
--------
Generic, version-resilient: search for stable anchors (Claude command names
and webview message-type strings — public API surface, unlikely to change
across minor versions) and capture the minified variable names dynamically
via regex backreferences. The patches survive bumps like 2.1.145 → 2.1.146
even when the minified bundle renames `R0` → `Q7`, etc.

Idempotent: re-running is a no-op for each step (markers / context inspection
detect prior application).

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
    "→ The '+' button in the Claude webview header will open a split",
    "  column instead of a tab in the active group.",
    "→ The Claude icon / status bar item / 'Open last' command will",
    "  open the new tab next to the current one instead of at the end",
    "  of the active tab group.",
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
        print(f"{YELLOW}[1/6]{RESET} package.json — already patched")
        return
    loc["enum"].append("primary")
    loc.setdefault("enumDescriptions", []).append("Primary Editor (Active Column)")
    pkg_path.write_text(json.dumps(p, indent=2))
    print(f"{GREEN}[1/6]{RESET} package.json — added 'primary' to preferredLocation enum")


def patch_editor_open_guard(content):
    """Prevent `editor.open` from silently overwriting the preferredLocation
    setting when it's "primary" (which would otherwise mutate it to "panel")."""
    marker = 'getConfiguration("claudeCode").get("preferredLocation")!=="primary"'
    if marker in content:
        print(f"{YELLOW}[2/6]{RESET} extension.js editor.open guard — already patched")
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
    print(f"{GREEN}[2/6]{RESET} extension.js editor.open guard — applied (vscode={vs}, state={st})")
    return new_content


def patch_editor_openLast(content):
    """Redirect `editor.openLast` (= the icon and status bar handler) to
    `primaryEditor.open` when the setting is "primary"."""
    idx = content.find('"claude-vscode.editor.openLast"')
    marker = 'getConfiguration("claudeCode").get("preferredLocation")==="primary"'
    if idx != -1 and marker in content[idx:idx + 600]:
        print(f"{YELLOW}[3/6]{RESET} extension.js editor.openLast — already patched")
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
    print(f"{GREEN}[3/6]{RESET} extension.js editor.openLast — applied (state={st}, vscode={vs})")
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
        print(f"{YELLOW}[4/6]{RESET} extension.js primaryEditor.open active column — already patched")
        return content
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
    print(f"{GREEN}[4/6]{RESET} extension.js primaryEditor.open active column — applied (panelMgr={pm}, vscode={vs})")
    return new_content


def patch_plus_button_active_column(content):
    """Fix the `+` button in the Claude webview header. The handler for the
    `new_conversation_tab` message calls `editor.open(sessionId, prompt)`
    without the 4th `viewColumn` arg, so it falls back to `ViewColumn.Beside`
    and splits the column. Append `vscode.ViewColumn.Active` as the 4th arg
    to match the `open_in_editor` handler's behavior."""
    anchor = '"new_conversation_tab"'
    idx = content.find(anchor)
    if idx != -1:
        end = content.find('"new_conversation_tab_response"', idx)
        window = content[idx:end] if end != -1 else content[idx:idx + 300]
        if "ViewColumn.Active" in window:
            print(f"{YELLOW}[5/6]{RESET} extension.js '+' button viewColumn — already patched")
            return content
    pat = re.compile(
        r'(\w+)\.request\.type==="new_conversation_tab"\)return await '
        r'(\w+)\.commands\.executeCommand\("claude-vscode\.editor\.open",'
        r'\1\.request\.sessionId,\1\.request\.initialPrompt\)'
    )
    m = pat.search(content)
    if not m:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: new_conversation_tab handler pattern not found",
               IMPACT_LINES)
        sys.exit(1)
    msg, vs = m.groups()
    replacement = (
        f'{msg}.request.type==="new_conversation_tab")return await '
        f'{vs}.commands.executeCommand("claude-vscode.editor.open",'
        f'{msg}.request.sessionId,{msg}.request.initialPrompt,'
        f'{vs}.ViewColumn.Active)'
    )
    new_content = pat.sub(replacement, content, count=1)
    print(f"{GREEN}[5/6]{RESET} extension.js '+' button viewColumn — applied (msg={msg}, vscode={vs})")
    return new_content


def patch_openlast_move_to_end(content):
    """For the primary branch of `editor.openLast` (icon top-right / status
    bar item / command palette), follow `primaryEditor.open` with a single
    `workbench.action.moveEditorToEnd` so the new tab lands at the end of
    the active tab group. Wrapped in `try/catch` ; failures log to the
    « Claude VSCode » output channel via the PanelManager singleton's
    `.output` field.

    Why this command : `vscode.window.tabGroups` has no `move()` method
    (only `close()`) ; the internal `moveActiveEditor` is workbench-internal
    and gated on text-editor focus ; `workbench.action.moveEditorToEnd`
    (VS Code ≥ 1.109) is the only extension-callable path that works on
    webview tabs."""
    anchor = 'getConfiguration("claudeCode").get("preferredLocation")==="primary"'
    idx = content.find(anchor)
    if idx != -1:
        marker = 'try{await '
        win = content[idx:idx + 600]
        if marker in win and '"workbench.action.moveEditorToEnd"' in win:
            print(f"{YELLOW}[6/6]{RESET} extension.js openLast move-to-end — already patched")
            return content

    pm_match = re.search(
        r'registerCommand\("claude-vscode\.primaryEditor\.open"[^}]*?(\w+)\.createPanel\(',
        content,
    )
    if not pm_match:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: panelMgr binding not found near "
               "primaryEditor.open (needed for the .output log target)",
               IMPACT_LINES)
        sys.exit(1)
    pm = pm_match.group(1)

    pat = re.compile(
        r'(getConfiguration\("claudeCode"\)\.get\("preferredLocation"\)'
        r'==="primary"\)\{await (\w+)\.commands\.executeCommand'
        r'\("claude-vscode\.primaryEditor\.open"\);)'
        r'[\s\S]*?'
        r'(return\})'
    )
    m = pat.search(content)
    if not m:
        banner("ICON-FIX-OPEN-IN-CURRENT-PANEL PATCH FAILED",
               "extension.js: openLast primary-branch pattern not found "
               "(expected from step [3/6])",
               IMPACT_LINES)
        sys.exit(1)
    vs = m.group(2)
    replacement = (
        rf'\1try{{await \2.commands.executeCommand'
        rf'("workbench.action.moveEditorToEnd")}}'
        rf'catch(e){{{pm}.output.error('
        rf'"[icon-fix] moveEditorToEnd failed: "+(e?.message??e))}};'
        rf'\3'
    )
    new_content = pat.sub(replacement, content, count=1)
    print(f"{GREEN}[6/6]{RESET} extension.js openLast move-to-end — applied (vscode={vs}, panelMgr={pm})")
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
    content = patch_plus_button_active_column(content)
    content = patch_openlast_move_to_end(content)

    if content != original:
        js.write_text(content)

    print(f"{GREEN}{BOLD}✓ icon-fix-open-in-current-panel patches complete{RESET}")


if __name__ == "__main__":
    main()

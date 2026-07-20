#!/usr/bin/env python3
"""
Patches the Claude Code VS Code extension's model picker (FXe on 2.1.201,
dt1 on 2.1.145, both in webview/index.js) to always surface Claude Opus
4.7 as a selectable option, regardless of whether the server-side tier
filter still lists it as "current".

Why
---
Opus 4.7 is Anthropic's flagship 1M-context legacy model. The current
session's env resolves it as `claude-opus-4-7[1m]`. It's the only
Opus-tier that remains classifier-free for cybersec / binary-RE
workloads (Fable 5 ships a refusal classifier that trips on offensive-
cyber content per docs/model-upgrade-eval-*.md — Opus 4.8 doesn't have
the classifier but is a full-price capability replacement, not a legacy
pin). Once Anthropic rotates the "current" tier to Opus 4.8+ the picker
silently drops 4.7 — losing the last classifier-free flagship for this
project's ~53 % share-of-spend (Ragexe RE).

Symptom before patch : `/model` picker in the VS Code chat shows only
the models tagged "current" for the user's tier (currently Opus 4.7,
Sonnet 4.6, Haiku 4.5 — this tier still surfaces 4.7). Once the tier
cycles, 4.7 disappears from the picker even though the binary still
knows `claude-opus-4-7[1m]` and the CLI accepts `--model claude-opus-
4-7[1m]` fine.

Strategy
--------
Wrap the `availableModels:X.claudeConfig.value?.models` prop passed to
the picker component with an IIFE that appends
`{value:"claude-opus-4-7[1m]",displayName:"Opus 4.7"}` if not already
present. Idempotent at runtime : if the CLI response already includes
4.7 (current tier behaviour), the IIFE returns the array unchanged.

Cross-version : regex captures the minified identifier before
`.claudeConfig.value?.models` (Z on 2.1.145, t on 2.1.201, …). Same
call-site anchor works on both because the picker component name
(`FXe` on 201, `dt1` on 145) is not in the anchor — only the stable
prop name + config path are.

Idempotency at file level : `MARKER = 'claude-opus-4-7[1m]'` — a
string that would never appear naturally in webview/index.js (verified
by grep on 2.1.145 and 2.1.201 : 0 hits at baseline). Presence signals
prior application, short-circuits with YELLOW.

Exit codes
----------
- 0 : applied or already patched
- 1 : regex miss / file missing (red banner via _common.banner)

Usage
-----
    opus-4-7-legacy-picker-fix.py [EXT_DIR]

If EXT_DIR is omitted, auto-discovers the latest
~/.vscode-server/extensions/anthropic.claude-code-*-{arch} directory.
"""

import os
import re
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import YELLOW, GREEN, BOLD, RESET, banner, resolve_ext_dir, check_files


MARKER = 'claude-opus-4-7[1m]'

IMPACT_LINES = [
    "→ The '/model' picker will not surface Claude Opus 4.7 if Anthropic's",
    "  server-side tier drops it from the 'current' set.",
    "→ Fallback still works :",
    "    • type `/model claude-opus-4-7[1m]` in the chat, or",
    "    • set \"model\": \"claude-opus-4-7[1m]\" in .claude/settings*.json.",
    "Likely cause : CLAUDE_CODE_VERSION was bumped and the picker call-site",
    "  anchor drifted (prop name or claudeConfig path). Review the regex in",
    "  .devcontainer/claude/vscode-ext-patchs/opus-4-7-legacy-picker-fix.py.",
]


def patch_webview_index_js(js_path):
    content = js_path.read_text()
    if MARKER in content:
        print(f"{YELLOW}[1/1]{RESET} webview/index.js — already patched (marker found)")
        return
    pat = re.compile(
        r'availableModels:(\w+)\.claudeConfig\.value\?\.models'
    )
    matches = list(pat.finditer(content))
    if not matches:
        banner("OPUS-4-7-LEGACY-PICKER-FIX PATCH FAILED",
               "webview/index.js: picker availableModels prop pattern not found",
               IMPACT_LINES)
        sys.exit(1)

    # Two matches are expected on 2.1.201 (call site + component definition
    # `function FXe({...availableModels:i...})`). Only the call site has the
    # `.claudeConfig.value?.models` path — the definition has a plain
    # parameter name. The regex requires `.claudeConfig.value?.models`, so
    # matches[] contains ONLY the call site by construction. On 2.1.145 there
    # is one match (the createElement call site).
    def make_replacement(m):
        ident = m.group(1)
        return (
            f'availableModels:((__m)=>__m?.some((__x)=>__x.value==="claude-opus-4-7[1m]")'
            f'?__m:[...(__m??[]),{{value:"claude-opus-4-7[1m]",displayName:"Opus 4.7"}}])'
            f'({ident}.claudeConfig.value?.models)'
        )

    new_content, n = pat.subn(make_replacement, content)
    js_path.write_text(new_content)
    captures = ', '.join(m.group(1) for m in matches)
    print(f"{GREEN}[1/1]{RESET} webview/index.js — Opus 4.7 injected at {n} site(s) (ident={captures})")


def main():
    ext_dir = resolve_ext_dir(sys.argv)
    check_files(ext_dir, ["webview/index.js"])

    print(f"Patching Claude Code extension at: {ext_dir}")
    patch_webview_index_js(ext_dir / "webview" / "index.js")
    print(f"{GREEN}{BOLD}✓ opus-4-7-legacy-picker-fix patch complete{RESET}")


if __name__ == "__main__":
    main()

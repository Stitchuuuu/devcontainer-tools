#!/usr/bin/env python3
"""
Injects a `simulated_click` interceptor into the Claude Code VS Code webview
bundle (`webview/index.js`). Companion to `outbound-action-injector.py`.

The extension-side watcher posts `{type:"from-extension", message:{type:
"simulated_click", requestId, result:{behavior, updatedInput,
updatedPermissions}}}` to `panel.webview.postMessage()` whenever an entry
lands in `.devcontainer/logs/claude-code-vscode-ext-outbound.jsonl`. This
patch intercepts that message on the WEBVIEW side (before the normal
message dispatcher runs) and calls `Q.accept(...)` / `Q.reject(...)` on the
matching pending `Gn` permission-request instance — which is the SAME
method the Allow/Deny button click handler invokes.

Why this is the right abstraction: `handleToolPermissionRequest` builds
a `Q = new Gn($, ...)` (where $ is the requestId, stored as `Q.channelId`
inside Gn — misleadingly named). `Q.accept(input, perms)` fires
`Q.resolved.emit({behavior:"allow", updatedInput, updatedPermissions})`.
That triggers the `onResolved` callback registered inside
handleToolPermissionRequest:
  ```
  Q.onResolved((result) => {
    X({type:"tool_permission_response", result});      // (a) postMessage to ext
    this.permissionRequests.value =
      this.permissionRequests.value.filter(z => z !== Q);  // (b) UI dismiss
  });
  ```
So (a) sends the canonical response through the standard webview→ext
channel — captured by `user-action-observer.py` into `inbound.jsonl` with
`source:"user-action"`, indistinguishable from a real click. And (b)
drops Q from the reactive `permissionRequests` signal — dismissing the
prompt UI atomically. Zero orphan.

Anchor: the top-level `window.addEventListener("message",(G)=>{...})` in
the connection provider class. The `this` in the arrow function is
already the `zn` instance (which owns both `fromHost` — the message queue
— and `permissionRequests`). No global exposure needed.

MARKER: `notify-queue-webview-sim-click-v1`.

Exit codes
----------
- 0 : applied or already patched
- 1 : regex miss (red banner + IMPACT_LINES)

Usage
-----
    webview-simulated-click.py [EXT_DIR]
"""

import os
import re
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import YELLOW, GREEN, BOLD, RESET, banner, resolve_ext_dir, check_files


MARKER = 'notify-queue-webview-sim-click-v1'

IMPACT_LINES = [
    "→ The webview will NOT intercept simulated_click messages from the",
    "  extension. External permission-response injection via outbound.jsonl",
    "  will still resolve ext-side promises (the tool runs), but the",
    "  webview UI prompt will not dismiss — orphaned Allow/Deny buttons.",
    "Likely cause: CLAUDE_CODE_VERSION was bumped and the",
    "  window.addEventListener('message', ...) shape drifted, OR the",
    "  connection provider class no longer stores permissionRequests on",
    "  `this` in the listener's scope.",
    "Inspect vendor/anthropic.claude-code/v<VER>/pretty/webview-index.js",
    "  around the addEventListener('message', ...) call and update the",
    "  regex in .devcontainer/claude/vscode-ext-patchs/webview-simulated-click.py.",
]


def patch_message_intercept(content):
    """Rewrite:
        window.addEventListener("message",(G)=>{
          if(G.data.type==="from-extension")this.fromHost.enqueue(G.data.message)
        })
    into:
        window.addEventListener("message",(G)=>{
          if(G.data.type==="from-extension"){
            /*marker*/
            const m = G.data.message;
            if (m?.type === "simulated_click") { ...resolve pending Gn...; return; }
            this.fromHost.enqueue(m);
          }
        })
    """
    if MARKER in content:
        n = content.count(MARKER)
        print(f"{YELLOW}[webview-sim-click]{RESET} webview/index.js — already patched ({n} site(s))")
        return content

    # Groups:
    #   1: full prefix ending at the closing paren of the from-extension cond
    #   2: event param var (G) — used in backrefs
    #   3: the closing `})` of the addEventListener call
    pat = re.compile(
        r'(window\.addEventListener\("message",\((\w+)\)=>\{'
        r'if\(\2\.data\.type==="from-extension"\))'
        r'this\.fromHost\.enqueue\(\2\.data\.message\)'
        r'(\}\))'
    )
    m = pat.search(content)
    if not m:
        banner("WEBVIEW SIMULATED-CLICK PATCH FAILED",
               'webview/index.js: window.addEventListener("message",...) shape not found',
               IMPACT_LINES)
        sys.exit(1)

    ev_var = m.group(2)

    replacement = (
        m.group(1) + '{'
        '/*' + MARKER + '*/'
        f'const _m={ev_var}.data.message;'
        'if(_m?.type==="simulated_click"){'
          'try{'
            'const _rid=_m.requestId;'
            'const _Q=this.permissionRequests?.value?.find(q=>q?.channelId===_rid);'
            'if(_Q){'
              'const _r=_m.result;'
              'if(_r?.behavior==="allow")_Q.accept(_r.updatedInput??{},_r.updatedPermissions??[]);'
              'else _Q.reject(_r?.message??"denied",false);'
            '}else{console.warn("[webview-sim-click] no pending Gn for requestId="+_rid);}'
          '}catch(e){console.warn("[webview-sim-click] failed:",e);}'
          'return;'
        '}'
        f'this.fromHost.enqueue(_m);'
        '}' + m.group(3)
    )

    new_content = content[:m.start()] + replacement + content[m.end():]
    print(f"{GREEN}[webview-sim-click]{RESET} webview/index.js — injected (event var: {ev_var})")
    return new_content


def main():
    ext_dir = resolve_ext_dir(sys.argv)
    check_files(ext_dir, ["webview/index.js"])
    js = ext_dir / "webview" / "index.js"

    print(f"Patching Claude Code webview at: {js}")

    content = js.read_text()
    original = content
    content = patch_message_intercept(content)

    if content != original:
        js.write_text(content)

    print(f"{GREEN}{BOLD}✓ webview-simulated-click patches complete{RESET}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
# @patch-category: fix
# @patch-files: webview/index.js
# @patch-sentinel: /*hasoo-v3*/
# @patch-step: adopt-guard webview/index.js /*hasoo-v3*/
# @patch-summary: Stops a fresh tab from guessing, as its own, the session id of an
#   older conversation — which ends in a black screen, a reload onto an empty
#   session and a lost title.
# @patch-bare-check: Keep one Claude conversation open. Open a NEW Claude tab
#   and, as soon as the input shows, paste a short prompt and press Enter
#   right away. COVERED if the tab keeps its conversation and gets its
#   generated title; NOT covered if it goes black, reloads onto the
#   "Introducing" home screen and the title stays "Claude Code". The output
#   channel tells it apart: the new tab's first update_session_state carries
#   an OLDER session's id, then an isFarewell on it.
"""
Patches the Claude Code VS Code extension's webview (webview/index.js) so the
session-id heuristic only adopts a session that is genuinely new, and never
one a panel already holds.

Symptom
-------
Prompt sent into a brand-new tab before its CLI has confirmed a session id
(paste + Enter within ~2 s of opening). The tab goes black a few seconds
later, reloads onto an empty session, and the generated title never lands.
The turn itself keeps running, unseen, on the first channel.

Measured on 2.1.280 (output channel):

    list_sessions_request              <- the tab's first read, at init
    launch_claude …                    resume: undefined
    io_message (the prompt)            ~1.6 s after the read was asked
    update_session_state <older id> running
    update_session_state <older id> idle isFarewell
    update_session_state <real id> running isPanelInitial idConfirmed
    init                               (webview reloaded, empty session)

The <older id> was the conversation open in the other tab (ec4980b7) on the
first runs; with that one excluded, it became the previous test session
(474f1dba), open nowhere.

Cause
-----
`doListSessions` walks the server's session list. For a session it does not
know locally, while the active session is busy and has no id yet, it adopts
that session's id — a guess that a transcript which appeared since the last
read is the active session's own:

    if(N&&!N.sessionId.value&&(!V||Boolean(H))&&(N.busy.value||N.isLoading.value)
       &&(!H||P.id===H&&!this.createdByUser.has(N))
       &&!this.isDroppedSessionId(P.id)&&!P.archived&&…)
      { N.adoptHeuristicSessionId(P.id) … }

When the tab's very FIRST read answers after the prompt was sent, there is no
previous read: every session of the workspace is "unknown", and the guess
takes whichever comes first. Nothing excludes a session another panel hosts
either. Upstream code — no other patcher touches this region.

Fix
---
One extra conjunct right after `!this.isDroppedSessionId(P.id)`. With no
explicit boot id (`H`), the guess is refused when:

- `first-read` — `storeAtLastRead` is still unset: no baseline, so "unknown"
  does not mean "new";
- `open-elsewhere` — the id is in `openSessionIds`, which the extension
  pushes on every `session_states_update` (its `sessionPanels` keys plus
  remembered tabs; the same class reads it in `replaceActiveSessionIfArchived`).

`H` keeps the stock path: when it is set the adoption is targeted
(`P.id===H`), not guessed. The other call site, `adoptHeuristicSessionId(H)`,
is the boot-id one and is left alone. The active session then simply waits
for the CLI to confirm its own id.

Log
---
A refused guess posts `{type:"hasoo-log", skip, candidate, total}` once per
list response (a flag on the response object; a first read refuses every
session of the workspace, one line each would be dozens); a granted guess
posts `{type:"hasoo-log", adopt, candidate}` every time. Both go to the
extension, which writes it to the output channel as
`Received message from webview: …` and otherwise ignores the unknown type.
Nothing leaves the machine (no `log_event`, which is telemetry). Needs
`window.__CC_vscodeApi__` (webview-login-retry-button); without it the log is
silent and the fix still applies.

Anchors: the conjunct chain from `isDroppedSessionId(P.id)` to
`adoptHeuristicSessionId(P.id)`, `q=this.storeAtLastRead` and
`for(let P of Y.sessions){if(!P.isCurrentWorkspace)continue;` in the same
function; minified names captured, exactly one match each. Earlier
insertions (v1, delimited v2+) are stripped before applying.

Exit codes
----------
- 0 : applied or already patched
- 1 : anchor miss / file missing (red banner)

Usage
-----
    heuristic-adopt-skip-open-sessions.py [EXT_DIR]
"""

import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import YELLOW, GREEN, RESET, banner, resolve_ext_dir, check_files


TAG = '/*hasoo-v3*/'
END = '/*hasoo-end*/'
STRIP_PAT = re.compile(r'/\*hasoo-v\d+\*/.*?/\*hasoo-end\*/', re.DOTALL)
V1_PAT = re.compile(
    r'/\*hasoo-v1\*/&&\(Boolean\([\w$]+\)\|\|!\(this\.comms\.openSessionIds\.value\?\?\[\]\)'
    r'\.includes\([\w$]+\.id\)\)'
)

IMPACT = ["A fresh tab can still adopt an older session's id when its prompt is",
          "sent early: black screen, reload onto an empty session, lost title.",
          "Re-derive the anchors in heuristic-adopt-skip-open-sessions.py."]

ADOPT_PAT = re.compile(
    r'&&!this\.isDroppedSessionId\(([\w$]+)\.id\)'
    r'(?=&&!\1\.archived&&\(!\1\.unreadable\|\|Boolean\(([\w$]+)\)\)\)\{'
    r'if\(([\w$]+)\.adoptHeuristicSessionId\(\1\.id\))'
)
LAST_READ_PAT = re.compile(r',([\w$]+)=this\.storeAtLastRead,')
LOOP_PAT = re.compile(r'for\(let ([\w$]+) of ([\w$]+)\.sessions\)\{if\(!\1\.isCurrentWorkspace\)continue;')
OPEN_IDS = 'this.comms.openSessionIds.value'


def guard_js(entry, boot, last_read, listing):
    reason = (f'Boolean({boot})?null:{last_read}===void 0?"first-read"'
              f':({OPEN_IDS}??[]).includes({entry}.id)?"open-elsewhere":null')
    return (TAG
            + '&&((__r,__c,__l)=>{try{var __a=window.__CC_vscodeApi__;'
            + 'if(!__r)__a?.postMessage({type:"hasoo-log",adopt:__c});'
            + 'else if(!__l.__hasooLogged){__l.__hasooLogged=1;'
            + '__a?.postMessage({type:"hasoo-log",skip:__r,candidate:__c,'
            + 'total:__l.sessions.length})}}catch(__e){}return!__r})'
            + f'({reason},{entry}.id,{listing})'
            + END)


def main():
    ext_dir = resolve_ext_dir(sys.argv)
    check_files(ext_dir, ["webview/index.js"])
    path = ext_dir / "webview/index.js"
    content = path.read_text()

    if TAG in content:
        print(f"{YELLOW}[heuristic-adopt-skip-open-sessions]{RESET} webview/index.js already patched (marker found)")
        return 0

    content, n_v1 = V1_PAT.subn('', content)
    content, n_old = STRIP_PAT.subn('', content)

    matches = list(ADOPT_PAT.finditer(content))
    if len(matches) != 1:
        banner("HEURISTIC-ADOPT-SKIP-OPEN-SESSIONS PATCH FAILED",
               f"webview/index.js: adoption guard matched {len(matches)}x (expected 1)", IMPACT)
        return 1
    m = matches[0]
    reads = [r for r in LAST_READ_PAT.finditer(content) if r.start() < m.start()]
    if len(list(LAST_READ_PAT.finditer(content))) != 1 or len(reads) != 1:
        banner("HEURISTIC-ADOPT-SKIP-OPEN-SESSIONS PATCH FAILED",
               "webview/index.js: storeAtLastRead read not found once before the guard", IMPACT)
        return 1
    loops = [l for l in LOOP_PAT.finditer(content) if l.start() < m.start() and l.group(1) == m.group(1)]
    if len(list(LOOP_PAT.finditer(content))) != 1 or len(loops) != 1:
        banner("HEURISTIC-ADOPT-SKIP-OPEN-SESSIONS PATCH FAILED",
               "webview/index.js: session-list loop not found once before the guard", IMPACT)
        return 1
    if OPEN_IDS not in content:
        banner("HEURISTIC-ADOPT-SKIP-OPEN-SESSIONS PATCH FAILED",
               f"webview/index.js: {OPEN_IDS} not found", IMPACT)
        return 1

    entry, boot, active = m.groups()
    last_read = reads[0].group(1)
    listing = loops[0].group(2)
    path.write_text(content[:m.end()] + guard_js(entry, boot, last_read, listing) + content[m.end():])
    print(f"{GREEN}[heuristic-adopt-skip-open-sessions]{RESET} webview/index.js — "
          f"heuristic adoption needs a prior read and skips open sessions "
          f"(entry {entry}, boot id {boot}, active {active}, last read {last_read}, list {listing})"
          + (" (earlier version stripped)" if n_v1 or n_old else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())

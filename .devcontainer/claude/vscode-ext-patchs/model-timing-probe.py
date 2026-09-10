#!/usr/bin/env python3
# @patch-category: ux
# @patch-files: extension.js
# @patch-files: webview/index.js
# @patch-sentinel: /*mt-v2*/
# @patch-sentinel: /*mtw-v2*/
# @patch-summary: Instruments the model pipeline with [model-timing] log lines so picker
#   fill latency and selection mismatches are measurable. Diagnostic: dogfood
#   only, never baked into the published image.
"""
Instruments the Claude Code VS Code extension's model pipeline — config
loading (extension.js) AND model selection (webview/index.js) — with
`[model-timing]` log lines, so the picker's variable fill latency and its
selection mismatches become measurable instead of anecdotal.

Why
---
Two symptoms, one blind spot.

1. The picker renders the hard-coded pins injected by
   opus-4-7-legacy-picker-fix.py immediately, then refreshes with the real
   server list — sometimes instantly, sometimes after 1-5 s, sometimes
   apparently never reusing the cache.
2. A model is NOT selected on the pins frame, then IS selected once the
   live list lands (observed with Opus 5). The picker compares with strict
   equality — `l.find((p) => p.value === o)` — so a pin `claude-opus-5[1m]`
   cannot match a `currentModel` of `opus[1m]`; the server's alias entry
   can. Proving that requires logging BOTH lists' actual values and the
   match outcome, not just entry counts.

Reading the code narrows symptom 1 to two branches but cannot say which
fires in practice, nor how often:

    invalidateConfigCache(){
      this.configEpoch++; …;
      this.config = void 0;
      this.cachedClaudeSettings = void 0;   // whole cache dropped
    }

    async pushStateUpdate(){
      let e;
      if(this.config) e = await this.config.catch(()=>{});              // HIT
      else this.loadConfig().then(()=>this.pushStateUpdate(), ()=>{});  // MISS
      this.send({…, request:{type:"update_state", config:e}});
    }

On a MISS the webview is handed `config: undefined` right away — the
pins-only frame — and a second push lands once loading completes. So
"instant" vs "never cached" are literally the two arms of that `if`.

Calibration, and why guessing was not good enough. Before these probes
existed, timing was read off the interleaved extension log and came out at
"~250 ms, deterministic" — the active session's own CLI output had been
mistaken for the probe's. Probe 4 measured the real thing on the first run:

    probeDone ms=8857

8.9 seconds. The spawn IS the latency, and the earlier estimate was wrong
by a factor of 35. Left as a standing reminder that a shared log file is
not a measurement.

Strategy
--------
extension.js probes write through `this.logger.log()`, which already
persists to
  ~/.vscode-server/data/logs/<ts>/exthost<N>/Anthropic.claude-code/Claude VSCode.log
(exthost<N> increments on every Reload Window — always glob `exthost*`, the
newest directory is the live one.)
with millisecond timestamps — a greppable file, zero new plumbing:

  1. invalidateConfigCache() — epoch transition + truncated caller stack.
     Answers "why is the cache never used": who drops it, how often.
  2. loadConfig() entry     — HIT / MISS, and stamps this.__mtT0.
  3. fallback setTimeout    — did the 500 ms timer spawn a probe, or had a
     real channel already claimed the resolver (wasted spawn).
  4. spawnConfigProbe()     — wall duration, plus the shape of the
     initialization result, whose payload is documented nowhere.
  5. pushStateUpdate()      — WARM / COLD, and the model VALUES shipped to
     the webview. Upper bound of the perceived latency.

webview/index.js probes go to the devtools console (no logger there) and
accumulate in `window.__modelTiming` so a whole session can be pasted at
once. They cover every site that decides which model is current:

  6. setModel      — manual picks.
  7. refusalFallback — the silent reassignment of modelSelection when a
     refusal triggers a fallback. No user action involved, so without a
     log it looks like the model changed by itself.

The picker-match and launch-seeding probes moved to model-selection-fix.py,
which rewrites those two statements. A probe must live with the fix it
observes: whichever script ran second would otherwise find its anchor
already mutated and go quietly dark.

The pins-vs-live render probe lives in opus-4-7-legacy-picker-fix.py
instead, grafted into the IIFE it already injects — same switch point, one
fewer anchor to maintain, and it cannot rot independently of the patch it
depends on.

Verbosity
---------
Both sides default to ON — the point is to catch a bug that only shows up
occasionally, and a probe that must be enabled before it can observe
anything is useless for that. The knob is `CLAUDE_EXT_VSCODE_LOG`; any of
0 / off / false / no silences it.

- extension.js : reads `process.env.CLAUDE_EXT_VSCODE_LOG`. Set it in
  `.devcontainer/.env`, which docker-compose loads via
  `env_file: [{path: .env}]`. Verified empirically rather than assumed:
  the extension host process carries the .env keys (CLAUDE_CREDS_VOLUME,
  NOTIFY_CHANNELS, FIGMA_FILE_KEY…), so `.env` genuinely reaches
  `process.env` here. Takes a container restart, which is the right cost
  for a "turn it off for good" switch. Note devcontainer.json's own
  comment: `containerEnv` applies AFTER env_file and would override it, so
  .env is the correct place — not containerEnv.
- webview      : reads `localStorage.CLAUDE_EXT_VSCODE_LOG`. A browser
  context cannot see process.env, and localStorage is better here anyway:
  it toggles live from the devtools console, persists across reloads, and
  needs no restart.

    localStorage.CLAUDE_EXT_VSCODE_LOG = "0"   // silence, live

Timestamp stamping (`__mtT0`, `__mtProbeT0`) stays OUTSIDE the gate, so
silencing the logs never changes control flow or leaves a later probe
computing a delta against an unset clock.

Cross-version: every minified identifier is CAPTURED by the anchor regex,
never hard-coded. Anchors key off stable method names
(invalidateConfigCache, loadConfig, pushStateUpdate, setModel,
applyRefusalFallback), the literal probe log string, and the "config
invalidated mid-probe" error text.

Log the VALUES, not the counts
------------------------------
Nine probes: five in extension.js, four in the webview (match / seed /
setModel / refusal-fallback). Each logs the model values themselves, which
is the whole point — entry counts cannot distinguish "the right list
arrived but nothing matched" from "the list was empty", and those two call
for opposite fixes.

Exit codes
----------
- 0 : applied
- 1 : regex miss / file missing (red banner via _common.banner)

Usage
-----
    model-timing-probe.py [EXT_DIR]

If EXT_DIR is omitted, auto-discovers the latest
~/.vscode-server/extensions/anthropic.claude-code-*-{arch} directory.
"""

import re
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _common import YELLOW, GREEN, BOLD, RESET, banner, resolve_ext_dir, check_files


EXT_TAG = '/*mt-v2*/'
WEB_TAG = '/*mtw-v2*/'
PREFIX = '[model-timing]'

# Node side: opt-out via env — verified that the extension host inherits
# .devcontainer/.env through docker-compose `env_file` (PID 3723 carries
# CLAUDE_CREDS_VOLUME & co). Browser side: opt-out via localStorage, since a
# webview has no process.env. Same knob name on both, "off" values aligned.
_OFF = '["0","off","false","no"]'
EXT_GATE = (
    f'!{_OFF}.includes('
    'String(process.env.CLAUDE_EXT_VSCODE_LOG??"1").toLowerCase())'
)
WEB_GATE = (
    f'!{_OFF}.includes('
    'String(localStorage.getItem("CLAUDE_EXT_VSCODE_LOG")??"1").toLowerCase())'
)

# Every injection has a fixed shape, so one pattern per file strips them all
# and restores the original bytes. Non-greedy up to a terminator no payload
# contains.
STRIP_EXT_PAT = re.compile(r'/\*mt-v\d+\*/try\{.*?\}catch\{\};', re.DOTALL)
STRIP_WEB_PAT = re.compile(r'/\*mtw-v\d+\*/try\{.*?\}catch\(__e\)\{\};', re.DOTALL)


def ext_probe(body, always=''):
    """extension.js probe: gated log, ungated stamping."""
    stamp = f'{always};' if always else ''
    return f'{EXT_TAG}try{{{stamp}if({EXT_GATE}){{{body}}}}}catch{{}};'


def web_probe(body):
    """webview probe: console + cumulative window.__modelTiming."""
    return f'{WEB_TAG}try{{if({WEB_GATE}){{{body}}}}}catch(__e){{}};'


def web_log(phase, expr_pairs, extra_fields=''):
    """Build a console.log + __modelTiming push for one webview probe.

    expr_pairs: list of (label, js_expr) rendered as `label=<expr>`.
    """
    text = '+" "+'.join([f'"{label}="+({expr})' for label, expr in expr_pairs])
    fields = ''.join(f',{label}:({expr})' for label, expr in expr_pairs)
    return (
        f'var __t=new Date().toISOString();'
        f'{extra_fields}'
        f'console.log("{PREFIX} "+__t+" {phase} "+{text});'
        f'(window.__modelTiming=window.__modelTiming||[])'
        f'.push({{ts:__t,phase:"{phase}"{fields}}});'
    )


IMPACT_LINES = [
    "→ '[model-timing]' lines will be ABSENT from the extension log and the",
    "  webview console, so the picker's cache behaviour and its model-",
    "  selection mismatches stay unmeasurable.",
    "→ No functional impact: these are pure observability probes.",
    "Likely cause : CLAUDE_CODE_VERSION was bumped and an anchor drifted",
    "  (method name, the probe log string, or the mid-probe error text).",
    "  Review the regexes in",
    "  .devcontainer/claude/vscode-ext-patchs/model-timing-probe.py.",
]


# =========================================================================
# extension.js — config loading
# =========================================================================

# --- 1 : cache invalidation. Answers "sometimes it NEVER uses the cache".
P1_PAT = re.compile(r'invalidateConfigCache\(\)\{')
P1_SUB = 'invalidateConfigCache(){' + ext_probe(
    'this.logger.log('
    f'`{PREFIX} ${{new Date().toISOString()}} invalidate '
    'epoch=${this.configEpoch}->${this.configEpoch+1} '
    'hadConfig=${this.config!==void 0} '
    'by=${(new Error().stack||"").split("\\n").slice(2,5).join(" | ")'
    '.replace(/\\s+/g," ").slice(0,240)}`)'
)

# --- 2 : loadConfig entry, HIT vs MISS. Stamp is ungated on purpose.
P2_PAT = re.compile(r'loadConfig\(\)\{if\(this\.config\)return this\.config;')
P2_SUB = 'loadConfig(){' + ext_probe(
    'this.logger.log('
    f'`{PREFIX} ${{new Date().toISOString()}} loadConfig '
    '${this.config!==void 0?"HIT":"MISS"} epoch=${this.configEpoch}`)',
    always='this.__mtT0=Date.now()',
) + 'if(this.config)return this.config;'


# --- 3 : the 500 ms fallback timer — how many spawns were wasted.
def _p3_sub(m):
    resolver_obj, cb_arg = m.group(1), m.group(2)
    return (
        f'{resolver_obj}.fallbackTimer=setTimeout(({cb_arg})=>{{'
        + ext_probe(
            'this.logger.log('
            f'`{PREFIX} ${{new Date().toISOString()}} fallbackTimer '
            f'superseded=${{this.configResolver!=={cb_arg}}} '
            'sinceLoadConfig=${Date.now()-(this.__mtT0||Date.now())}ms`)'
        )
        + f'if(this.configResolver!=={cb_arg})return;'
    )


P3_PAT = re.compile(
    r'([\w$]+)\.fallbackTimer=setTimeout\(\(([\w$]+)\)=>\{'
    r'if\(this\.configResolver!==\2\)return;'
)

# --- 4a : probe start stamp (ungated).
P4A_PAT = re.compile(
    r'this\.logger\.log\("Loading config cache by launching Claude '
    r'\(no channel\)\.\.\."\);'
)
P4A_SUB = (
    'this.logger.log("Loading config cache by launching Claude (no channel)...");'
    + ext_probe('', always='this.__mtProbeT0=Date.now()')
)


# --- 4b : probe completion — real cost of a spawn.
def _p4b_sub(m):
    done_obj, ret_obj, epoch_var, result_var = m.groups()
    return (
        f'if({done_obj}.done(),{ret_obj}.return(),this.configEpoch!=={epoch_var})'
        'throw Error("config invalidated mid-probe");'
        + ext_probe(
            'this.logger.log('
            f'`{PREFIX} ${{new Date().toISOString()}} probeDone '
            'ms=${Date.now()-(this.__mtProbeT0||Date.now())} '
            f'resultKeys=${{Object.keys({result_var}||{{}}).slice(0,10).join(",")}} '
            'settingsModel=${this.cachedClaudeSettings?.effective?.model??"?"}`)'
        )
        + f'return {result_var}}}'
    )


P4B_PAT = re.compile(
    r'if\(([\w$]+)\.done\(\),([\w$]+)\.return\(\),this\.configEpoch!==([\w$]+)\)'
    r'throw Error\("config invalidated mid-probe"\);return ([\w$]+)\}'
)


# --- 5 : what actually ships to the webview — values, not just counts.
def _p5_sub(m):
    msg_var, uuid_fn, config_var = m.groups()
    return (
        f'let {msg_var}={{type:"request",channelId:"",requestId:{uuid_fn}(),'
        'request:{type:"update_state",state:this.getCurrentState(),'
        f'config:{config_var}}}}};'
        + ext_probe(
            'this.logger.log('
            f'`{PREFIX} ${{new Date().toISOString()}} pushStateUpdate '
            f'${{{config_var}!==void 0?"WARM":"COLD"}} '
            f'models=${{{config_var}?.models?.length??-1}} '
            f'unavailable=${{{config_var}?.unavailable_models?.length??-1}} '
            'settingsModel=${this.getModelSetting?.()??"?"} '
            f'values=${{({config_var}?.models??[]).map((__x)=>__x.value+'
            '(__x.resolvedModel&&__x.resolvedModel!==__x.value'
            '?">"+__x.resolvedModel:"")).join(",")}`)'
        )
        + f'this.send({msg_var})'
    )


P5_PAT = re.compile(
    r'let ([\w$]+)=\{type:"request",channelId:"",requestId:([\w$]+)\(\),'
    r'request:\{type:"update_state",state:this\.getCurrentState\(\),'
    r'config:([\w$]+)\}\};this\.send\(\1\)'
)

# --- 5b : the per-channel push — the blind spot that hid the real bug.
# pushStateUpdate() is NOT the path that serves a webview during the config
# fetch; pushChannelStateUpdate() is, and it ships `config: undefined` while
# `state` still carries modelSetting. Because only the former was probed, the
# first 10 s of every session were invisible and a regression there was
# mistaken for correct behaviour. Probe both, or trust neither.
def _p5b_sub(m):
    msg_var, chan, uuid_fn, state_var, config_var = m.groups()
    return (
        f'let {msg_var}={{type:"request",channelId:{chan},requestId:{uuid_fn}(),'
        f'request:{{type:"update_state",state:{state_var},config:{config_var}}}}};'
        + ext_probe(
            'this.logger.log('
            f'`{PREFIX} ${{new Date().toISOString()}} pushChannelState '
            f'${{{config_var}!==void 0?"WARM":"COLD"}} '
            f'modelSetting=${{{state_var}?.modelSetting??"<undefined>"}} '
            f'ready=${{{state_var}?.modelSettingReady}} '
            f'models=${{{config_var}?.models?.length??-1}}`)'
        )
        + f'this.send({msg_var})'
    )


P5B_PAT = re.compile(
    r'let ([\w$]+)=\{type:"request",channelId:([\w$]+),requestId:([\w$]+)\(\),'
    r'request:\{type:"update_state",state:([\w$]+),config:([\w$]+)\}\};this\.send\(\1\)'
)


EXT_PROBES = [
    ("invalidateConfigCache", P1_PAT, P1_SUB),
    ("loadConfig", P2_PAT, P2_SUB),
    ("fallbackTimer", P3_PAT, _p3_sub),
    ("probeStart", P4A_PAT, P4A_SUB),
    ("probeDone", P4B_PAT, _p4b_sub),
    ("pushStateUpdate", P5_PAT, _p5_sub),
    ("pushChannelState", P5B_PAT, _p5b_sub),
]


# =========================================================================
# webview/index.js — model selection
# =========================================================================

# --- 8 : manual picks.
def _w8_sub(m):
    arg = m.group(1)
    return (
        f'async setModel({arg}){{'
        + web_probe(web_log('setModel', [
            ('requested', f'{arg}?.value'),
            ('from', 'this.modelSelection.value'),
        ]))
        + f'let {m.group(2)}=this.modelSelection.value,'
    )


W8_PAT = re.compile(r'async setModel\(([\w$]+)\)\{let ([\w$]+)=this\.modelSelection\.value,')


# --- 9 : the silent reassignment nobody asked for.
def _w9_sub(m):
    notice, b, c = m.groups()
    return (
        f'applyRefusalFallback({notice},{b},{c}){{'
        + web_probe(web_log('refusalFallback', [
            ('fallbackModel', f'{notice}?.fallbackModel'),
            ('direction', f'{notice}?.direction'),
            ('prevSelection', 'this.modelSelection.value'),
        ]))
        + f'this.refusalFallbackNotice.value={notice},'
    )


W9_PAT = re.compile(
    r'applyRefusalFallback\(([\w$]+),([\w$]+),([\w$]+)\)\{this\.refusalFallbackNotice\.value=\1,'
)

# pickerMatch and launchSeed are NOT here: they observe the two statements
# model-selection-fix.py rewrites. A probe and the fix it watches must share
# one script, or whichever runs second finds its anchor already mutated —
# and a probe that silently stops matching is worse than no probe.
WEB_PROBES = [
    ("setModel", W8_PAT, _w8_sub),
    ("refusalFallback", W9_PAT, _w9_sub),
]


# =========================================================================

def _apply(js_path, probes, strip_pat, label):
    content = js_path.read_text()

    stripped = len(strip_pat.findall(content))
    if stripped:
        content = strip_pat.sub('', content)

    results, missing = [], []
    for name, pat, sub in probes:
        # Plain-string replacements go through re's template parser, which
        # chokes on the `\n` / `\s` inside the injected JS. Wrap them so the
        # payload is emitted verbatim.
        repl = sub if callable(sub) else (lambda _m, _s=sub: _s)
        content, n = pat.subn(repl, content)
        results.append(f"{name}={n}")
        if n == 0:
            missing.append(name)

    if missing:
        banner(
            "MODEL TIMING PROBES NOT APPLIED",
            f"{label}: no match for {', '.join(missing)}",
            IMPACT_LINES,
        )
        return None

    js_path.write_text(content)
    note = f" (stripped {stripped} stale)" if stripped else ""
    return f"{label} — {', '.join(results)}{note}"


def main():
    ext_dir = resolve_ext_dir(sys.argv)
    check_files(ext_dir, ["extension.js", "webview/index.js"])

    ext_line = _apply(ext_dir / "extension.js", EXT_PROBES, STRIP_EXT_PAT,
                      "extension.js")
    web_line = _apply(ext_dir / "webview" / "index.js", WEB_PROBES,
                      STRIP_WEB_PAT, "webview/index.js")

    if ext_line is None or web_line is None:
        sys.exit(1)

    print(f"{GREEN}[1/2]{RESET} {ext_line}")
    print(f"{GREEN}[2/2]{RESET} {web_line}")
    print(f"{YELLOW}note{RESET}  silence: CLAUDE_EXT_VSCODE_LOG=0 in "
          f".devcontainer/.env (extension, needs restart) / "
          f"localStorage.CLAUDE_EXT_VSCODE_LOG=\"0\" (webview console, live)")


if __name__ == "__main__":
    main()

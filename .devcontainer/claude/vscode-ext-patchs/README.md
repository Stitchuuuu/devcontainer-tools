# Patchers — dogfood tree

**Project-local patchers only.** Since this tree runs on the published
`devcontainer-sandbox` image, the shared patchers come from the pinned tarball
of `meitogi/claude-ext-patchs` (`EXT_PATCHES_REPO` / `EXT_PATCHES_REF` in
`.devcontainer/.env`; ref unset = `auto`, the newest tag cut for the installed
Claude Code version). `_common.py` and `run-all.sh` are the image's toolkit.
Edit a shared patcher in that repository, never here.

What stays here :

- `model-timing-probe.py` — diagnostic instrumentation, dogfood-only by
  decision. It exists nowhere else: not in the image repo, not in the shared
  patcher repository.

### model-timing-probe : kept, re-anchored on 2.1.280 (2026-10-07)

**Decision: keep it.** Both symptoms it measures shrank on 2.1.280 but did
not go away, so it is still measuring something real.

- **Picker fill latency.** A cold fill now takes about 2.5 s, from
  `loadConfig MISS` to the first WARM push. The probe's own reference on 2.1.258
  was 8.9 s. Panels opened after the first one hit the host's in-memory config
  (`loadConfig HIT`) and get the live list immediately.
- **Model unticked on the pins frame.** The pins frame lasted about 1.3 s, and
  nothing could be ticked during it: the configured `opus[1m]` is an alias, and
  no pin carries it. The shared `opus-4-7-legacy-picker-fix` now serves the last
  live list during that window, so the tick is there from the first frame.

**What changed in the probe for 2.1.280.** Three `extension.js` anchors moved
upstream:

- the fallback timer's body became `startFallbackProbe()`, armed from two
  sites, and the probe anchors on the method;
- the config probe's return became a comma chain, so its result is captured
  where `initializationResult()` assigns it;
- the per-channel push lost its own `let`.

The counts are back to the 2.1.258 reference: `mt-v2=7`, `[model-timing]=6` in
`extension.js`.

**The webview half (probes 8-9) stays.** The three other webview
`[model-timing]` lines come from shared patchers:

- `launchSeed:before/after` from `model-selection-fix`;
- `render:` from `opus-4-7-legacy-picker-fix`.

None of them covers `setModel` or `refusalFallback`.

**Blind spot to know.** Since 2.1.280, a real channel that claims the config
cancels the no-channel spawn (`claimConfigResolver` → `cancelProbe`). That
spawn then stops before `probeDone`, so a missing `probeDone` line can mean
either cancelled or hung.

At boot, the `45-ext-patches.sh` hook resolves the shared set, then merges every
`*.py` found in this directory into the same work directory — a local file with
the same name as a shared patcher **replaces** it, and the boot output names the
override. `ext-patches-sync --status` shows what applied and from where;
`wtf ext-patch status` / `wtf ext-patch update` wrap the same commands.

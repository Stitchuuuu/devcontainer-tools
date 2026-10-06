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

At boot, the `45-ext-patches.sh` hook resolves the shared set, then merges every
`*.py` found in this directory into the same work directory — a local file with
the same name as a shared patcher **replaces** it, and the boot output names the
override. `ext-patches-sync --status` shows what applied and from where;
`wtf ext-patch status` / `wtf ext-patch update` wrap the same commands.

# Patchers — dogfood tree

**Still the live source for this container, and not yet reducible.**

`.devcontainer/Dockerfile.base:234` copies this whole directory into the image
it builds and runs `run-all.sh` from it. So `run-all.sh`, `_common.py` and the
sixteen shared patchers all have to stay here until this project switches onto
the published `devcontainer-sandbox` image — that is the `dogfood-switchover`
work, not this directory's.

Once it does switch, only the project-local patchers stay:

- `model-timing-probe.py` — diagnostic instrumentation, dogfood-only by
  decision. It exists nowhere else: not in the image repo, not in the shared
  patcher repository.

Everything else here is byte-identical to `meitogi/claude-ext-patchs`
(`patchers/`, tag `cc2.1.258-r1`), which is where those patchers are maintained
now. Edit them there, not here — and if you do change one here, expect the two
copies to drift silently, because nothing checks them against each other any
more.

After the switchover, the `45-ext-patches.sh` hook resolves the shared patchers
(from `EXT_PATCHES_DIR` or a pinned tarball) and merges whatever `*.py` it finds
in this directory into the same work directory. `_common.py` and `run-all.sh`
come from the image's toolkit at that point, and must be deleted from here.

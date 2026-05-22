# Test Plan — devcontainer-security-hardening

> Multi-rebuild validation matrix for the bake-only changes. Each scenario
> is a (host action) → (rebuild) → (in-container test) triplet. The unit
> tier is rebuild-free and runs at every iteration.

## TL;DR — minimal acceptance

For session 1, the minimum to ship is :

1. **Pre-rebuild** : `bash tests/run.sh --unit` → all pass.
2. **Rebuild #1 (strict mode, default)** :
   - Cmd+Shift+P → Dev Containers: Rebuild Container.
   - In the rebuilt container : `bash tests/run.sh --integration`.
   - Expected : all `test_runtime_*` + `test_vector*` pass. `test_mode_strict_invariants` passes ; `test_mode_basic` + `test_mode_off` skip.

That's enough to merge session 1. The 2 extra rebuilds below validate
that the baked mode flag genuinely controls runtime — they're the
defense-in-depth check, not gates.

## Full multi-rebuild matrix

| # | Host action | Rebuild ? | Tests to run | Expected output |
|---|---|---|---|---|
| 0 | (none — current state) | no | `tests/run.sh --unit` | unit 45/0/0 ; integration skipped |
| 1 | (none — first rebuild) | **yes** | `tests/run.sh` | unit 45/0/0 ; integration ~6 pass for bake + strict invariants ; basic/off skipped |
| 2 | `bash .devcontainer/firewall-mode.sh basic` | **yes** | `tests/run.sh --integration` | bake invariants pass ; `test_mode_basic_invariants` passes ; strict/off skipped |
| 3 | `bash .devcontainer/firewall-mode.sh off`   | **yes** | `tests/run.sh --integration` | bake invariants pass ; `test_mode_off_invariants` passes ; strict/basic skipped |
| 4 | `bash .devcontainer/firewall-mode.sh strict` | **yes** | `tests/run.sh --integration` | back to step 1 baseline ; sanity check that the strict→other→strict cycle leaves no residue |

After step 4 the suite has covered all 3 baked modes end-to-end. Total :
4 rebuilds for full coverage, 1 rebuild for the merge gate.

## claude-switch coverage (optional, separate concern)

The `claude-switch` host-helper toggles the LLM endpoint and now also
mutates `firewall/direct-tcp-allow.txt`. Verification requires its own
mini-matrix (orthogonal to firewall mode) :

| Step | Host action | Rebuild | Check |
|---|---|---|---|
| A | `bash host-helpers/claude-switch local-proxy` | yes | `grep claude-bridge:9223 /etc/devcontainer-firewall/direct-tcp-allow.txt` → match |
| B | `bash host-helpers/claude-switch local`       | yes | `grep host:11434 /etc/devcontainer-firewall/direct-tcp-allow.txt` → match ; previous `claude-bridge:9223` gone |
| C | `bash host-helpers/claude-switch cloud`       | yes | no active (non-comment) entry in `direct-tcp-allow.txt` |

The current test suite **does not yet automate this** ; running the
checks manually after each switch is enough until/unless we add a
dedicated `tests/integration/test-claude-switch.sh`.

## Running the suite

```bash
# From inside the container (or host for unit-only) :
cd /workspace
bash .devcontainer/tests/run.sh                  # all (unit + integration)
bash .devcontainer/tests/run.sh --unit           # static, no container needed
bash .devcontainer/tests/run.sh --integration    # post-rebuild only
```

Exit code : 0 if all assertions in the selected scope pass, 1 if any fail.

## When tests pass != safe to ship

The current suite gives high confidence on :

- File-level invariants (presence, syntax, structure)
- Boundary enforcement (workspace decoupled from /etc)
- Mode flag wired through to runtime behavior
- Migration logic for legacy projects

It does **not** cover :

- The full red-team replay of all 13 audit vectors (that's session 6 —
  `adversarial-validation`)
- Real exfil attempts via subdomain DNS tunneling (vector #9 — accepted
  gap)
- Hook-based persistence (vector #5 — defense-in-depth, session 3 if
  enabled)

For a production-grade ship, both this suite **and** session 6 must
pass. Sessions 1+2 alone close the critical-path vectors ; session 6
gates the rollout.

## Pre-merge checklist

- [ ] `bash .devcontainer/tests/run.sh --unit` exits 0
- [ ] One rebuild in strict mode, integration suite passes
- [ ] `git status` shows only expected paths
- [ ] LOG.md updated with the session entry
- [ ] STATUS.md flipped (📋 → ✅)

Optional but recommended :

- [ ] Multi-mode matrix (steps 2-4) executed
- [ ] claude-switch mini-matrix (A-C) executed

# 20260703-1242 — Add `scan-deps` shell command

**Affects** : v2.1 devcontainers without a top-level `scan-deps` command.
The `/scan-deps` Claude skill exists but the deterministic extractor
(`.devcontainer/skills/scan-deps/extract-auto-dependencies`) can only
be reached via the long path or via Claude.

**Symptom** : to refresh the firewall allowlist deterministically
(npm + composer manifests → `firewall/domains.d/*.txt`) you had to
type the full path every time.

**Fix** : add a `scan-deps` zsh function that thin-wraps the extractor,
with a positional ecosystem shortcut (`scan-deps npm` → `--ecosystem npm`).
Layer-2 AI review stays behind `/scan-deps` in Claude.

## Manual how-to

Two files, exact same append. Both `.devcontainer/zshrc-base` and
`templates/v2/zshrc-base`.

### File 1 — `.devcontainer/zshrc-base`

Open the file. Scroll to the **end** — the last non-empty lines are :

```
if [ -f "$__zshrc_base_wtf_completion" ]; then
  source "$__zshrc_base_wtf_completion"
fi
unset __zshrc_base_wtf_completion
```

After the last line (`unset __zshrc_base_wtf_completion`), append a blank
line then the block :

```bash

# scan-deps — one-shot deterministic allowlist refresh (npm + composer).
# Thin wrapper over .devcontainer/skills/scan-deps/extract-auto-dependencies.
# Positional ecosystem shortcut: `scan-deps npm` -> `--ecosystem npm`.
# Layer 2 (AI review) stays gated behind /scan-deps in Claude.
# Usage:
#   scan-deps                    # all ecosystems (default)
#   scan-deps npm                # limit to npm
#   scan-deps composer --offline # skip online HEAD (renamed-repo redirects)
#   scan-deps --dry-run          # preview only
scan-deps() {
  local first="${1:-}"
  case "$first" in
    npm|composer|all|python|cargo|go)
      shift
      bash /workspace/.devcontainer/skills/scan-deps/extract-auto-dependencies \
        --ecosystem "$first" "$@"
      ;;
    *)
      bash /workspace/.devcontainer/skills/scan-deps/extract-auto-dependencies \
        --ecosystem all "$@"
      ;;
  esac
}
```

### File 2 — `templates/v2/zshrc-base`

Same append, verbatim. Same location (end of file, after the `wtf_completion` block).

### Commit

`````bash
git add .devcontainer/zshrc-base templates/v2/zshrc-base

git commit -m "feat(shell): scan-deps function wrapping extract-auto-dependencies"
`````

### Reload the shell

`exec zsh` or open a new terminal. No rebuild needed — `.zshrc` sources
`zshrc-base` on every shell open.

## Verify

- [ ] `type scan-deps`
      → prints `scan-deps is a shell function from /home/node/.zshrc` (or similar).
- [ ] `scan-deps --help` (or `scan-deps --dry-run`)
      → the extractor runs (see its usage / dry-run output).
- [ ] `scan-deps npm --dry-run`
      → runs with `--ecosystem npm`. Non-npm ecosystems are skipped.
- [ ] `grep -c "^scan-deps()" .devcontainer/zshrc-base templates/v2/zshrc-base`
      → 1 each.

## Rollback

Remove the appended block from both files. Reload shell.

`````bash
git revert <commit-hash>
`````

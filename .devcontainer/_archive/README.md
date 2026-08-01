# `.devcontainer/_archive/`

Retired devcontainer tooling. Nothing here is loaded: `sync-skills.sh` only
globs `.devcontainer/skills/`, so a `*.skill.md` or a `hooks.json` sitting in
this directory is inert.

## `floating-perms/`

Time-limited permission grants (`PermissionRequest` / `PreToolUse` hooks writing
TTL'd entries into `.claude/settings.local.json`, revoked on `SessionEnd`).
Retired — the hooks fired on every tool call and the state file kept drifting
out of sync with the allow list.

To bring it back: `git mv .devcontainer/_archive/floating-perms
.devcontainer/skills/floating-perms`, then re-run
`bash .devcontainer/skills/sync-skills.sh` to merge its `hooks.json` back into
`~/.claude/settings.json`.

Note that `sync-skills.sh` merges additively and never removes, so disabling a
skill always takes two steps: move it out of `skills/`, *and* delete its hook
entries from `~/.claude/settings.json`.

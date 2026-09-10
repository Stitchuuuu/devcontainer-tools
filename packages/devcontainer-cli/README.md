# `@stitchu/devcontainer-cli`

Host-side control plane for the stitchu devcontainer stack. Runs **on the host**,
before and around the container — which is what distinguishes it from the tooling
baked into the image.

```sh
npx @stitchu/devcontainer-cli initialize
```

## Status

`0.1.0` — skeleton plus one real command. Not published yet.

| Command | State |
|---|---|
| `devc initialize` | **implemented** — host-side pre-container setup |
| `devc init` | stub — scaffold wizard |
| `devc update` | stub — bump base + Claude Code versions |
| `devc doctor` | stub — diagnostics |

Anything else from the design's command table (`patch`, `firewall`, `skill`,
`notify`, `host`) is absent rather than stubbed. `devc knowledge` and
`devc lessons` were dropped for v1 — those files are edited directly.

## `devc initialize`

The step VS Code runs through `initializeCommand`, before the container is
built. It is a port of the 663-line `.devcontainer/initialize.sh`, and does, in
order:

1. Classifies the host (`mac` / `linux` / `wsl` / `gitbash`) and records it in
   `logs/host-os`. This is the only code that runs on the host, so it is the
   only place that can answer — from inside the container the kernel reveals the
   hypervisor, not the OS.
2. Opens a timestamped log under `logs/`, plus a decision trace when `DEBUG=1`.
3. Seeds the files the image build depends on: `firewall/domains.local.txt`,
   `firewall/default-mode`, `firewall/ports.txt`,
   `firewall/policy.local.d/`, `claude-bridge/config.json`, and the
   `.vscode/settings.json` stub Docker Desktop on macOS needs before it will
   bind a file.
4. Projects team defaults from `customizations.stitchu-devc` in
   `devcontainer.json` into `.env`, never overwriting a value already there.
5. Creates the Claude credentials volume.
6. Decides whether this is a Rebuild, a Reopen or a first run (the
   container-presence probe) and logs which. Nothing is built locally — the
   base image is `ghcr.io/meitogi/devcontainer-sandbox:<base>-cc<cc>`, pulled
   by compose; bumping that tag is what an upgrade means.
7. Prompts for the Claude mode on first run, writes the flag files, aligns the
   proxy variables in `.env` with the firewall mode, prints a summary, and spawns
   the host notify daemon.

```
devc initialize [--devcontainer-dir <path>] [--dry-run]
```

`--dry-run` reports every decision and writes nothing — no files, no docker, no
daemon.

| Environment | Effect |
|---|---|
| `DEBUG=1` | Structured decision trace beside the log |
| `DEBUG_REBUILD_CONTEXT=1` | Dump the rebuild-signal diagnostic |

### Notable divergences from the bash script

Documented rather than hidden, because the two run side by side for now.

- **`.env` is parsed, not `source`d.** Bash handed the file to a shell, so `$VAR`
  interpolated and `$(…)` executed. docker-compose's `env_file:` reader never did
  either, so the two consumers of the same file already disagreed. This parser
  matches compose.
- **`.env` writes are line edits**, never a parse/serialize round-trip — comments,
  blank lines and ordering are preserved byte for byte.
- **The rolling window repaints on a timer** (~20 fps) instead of once per output
  line, and truncates on code points rather than bytes.
- **`DEBUG=1` cannot reproduce `set -x`.** Node has no per-statement hook. The
  trace records spawns, exits, decisions and file writes instead, in the same
  `+ …` shape.
- **Team defaults are projected into `.env`.** `initialize.sh` never read
  `devcontainer.json`. This command maps
  `customizations.stitchu-devc.allowLocalAtRebuild` to
  `FIREWALL_ALLOW_LOCAL_AT_REBUILD`, writing it only when `.env` does not
  already set it. No project sets that key yet, so the two implementations
  currently produce identical `.env` files — but they will not once one does.
- **The node-not-found diagnostic is gone.** Its 180 lines existed because
  `command -v node` could fail under VS Code's non-login shell. A Node CLI is
  already running Node, so that state is unreachable from here.

## Requirements

Node 18+, Docker, and VS Code with the Dev Containers extension. On Windows, run
from WSL2 or Git Bash — a bare `cmd.exe` host is refused by name rather than
failing later on a missing `wslpath`.

## Development

```sh
npm install
npm run build
npm test
```

Tests are `node:test`; there are no runtime dependencies and one development
dependency.

## License

MIT

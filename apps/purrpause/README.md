# PurrPause

Break-reminder overlay for Windows. Fires a fullscreen transparent Lottie
popup (dancing cat) every configurable interval, with pre-notification
countdown widgets at T-15 / T-10 / T-5.

Project codename `PurrPause`. On disk, the binary and service camouflage
as `Windows Session Health Service` — no user-visible `purr` / `pause` /
`cat` strings. See the design constitution for the reasoning.

## Design source of truth

Architectural plan — non-negotiable design constitution :
`/home/node/.claude/plans/serialized-humming-hollerith.md`

Rollout tracking :
- `/workspace/plans/purrpause/ROLLOUT.md` — entry point
- `/workspace/plans/purrpause/STATUS.md` — session scoreboard
- `/workspace/plans/purrpause/LOG.md` — per-session journal

## Build (cross-compile from Linux devcontainer)

```bash
cd apps/purrpause

# x64 Windows (Intel + AMD + Windows-on-ARM x64 emulation via Prism)
cargo xwin build --release --target x86_64-pc-windows-msvc

# ARM64 Windows (native on Windows-on-ARM devices)
cargo xwin build --release --target aarch64-pc-windows-msvc
```

Both produce `target/<triple>/release/SystemHealthAgent.exe`.

First build fetches the Windows SDK (~500 MB per arch) into
`~/.cache/cargo-xwin/`. Subsequent builds hit the cache and complete in
seconds.

## Test (native Linux)

```bash
cd apps/purrpause
cargo test
```

Runs the argv classifier unit tests on the host toolchain. Windows-only
deps (tao / wry / eframe / windows / windows-service / webview2-com) are
gated behind `cfg(windows)` so `cargo check` on Linux stays lean and no
GTK/WebKit is required.

## Modes (argv dispatcher)

Every mode is a stub in session 1. Later sessions replace each stub with
its real implementation :

| Argv | Session | Behavior |
|---|---|---|
| _(no args)_ | 2 | Install or update service ImagePath, or open config UI if already installed |
| `--service` | 3 | Windows service entry point (SCM) |
| `--popup [--preview]` | 4 | Fullscreen Lottie popup, dismiss after config duration |
| `--countdown <secs> --palier <15\|10\|5>` | 5 | Corner countdown widget |
| `--config [--first-run]` | 6 | Passcode-gated egui config window |
| `--watchdog` | 7 | Health-check invoked by Scheduled Task |
| `--uninstall` | 7 | Passcode-gated removal flow |
| `--rollback-from-failed-install` | 2 | Internal — invoked by wizard cancellation |

## Known gaps (session 1)

- **No UAC manifest embedded** yet — `mt.exe` / `llvm-rc` unavailable in
  the devcontainer for xwin. Session 2 will either bundle a wrapper
  installer that handles elevation, or resolve the resource-compiler
  toolchain question. Current binaries run without UAC prompt on
  double-click.
- **webview2-com pinned in two versions** in the dep graph (0.38.2 via
  wry, 0.39.1 direct). Not a bug, just wasted compile time. Harmonize
  in session 4 when webview2-com is actually exercised.

## License

MIT.

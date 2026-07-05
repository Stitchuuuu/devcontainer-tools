# notifier

Cross-platform Rust workspace behind the `notif` CLI — a feature-rich
replacement for the ad-hoc `osascript` / PowerShell WinRT / `notify-send`
dispatch currently living in `.devcontainer/notify/lib/consumers/notifier.js`.

## Crates

| Crate | Role |
|---|---|
| `notif-cli` | Binary crate. Produces the `notif` executable. clap-driven CLI surface. |
| `notif-core` | Shared types (`Notification`, `Sender`, `Priority`), `Backend` trait, portable flag surface. Zero platform code. |
| `notif-macos` | `#[cfg(target_os = "macos")]` backend (UN center dispatch, sender tiers). |
| `notif-windows` | `#[cfg(target_os = "windows")]` backend (WinRT toast, AUMID). |
| `notif-linux` | `#[cfg(target_os = "linux")]` backend (libnotify / D-Bus). |

## Install

- **macOS** — [docs/install-macos.md](docs/install-macos.md) (cross-compile
  from the devcontainer, first-run Gatekeeper bypass, permission dialog,
  housekeeping).
- **Windows** — landing in v0.3 (session 9).
- **Linux** — landing in v0.4 (session 10).

## Where the full design lives

See [../../plans/notif-cli/ROLLOUT.md](../../plans/notif-cli/ROLLOUT.md)
for the goal, decisions, and staged rollout (v0.0 → v0.5). The design
brief with flag matrices and callback semantics is
[BRIEF.md](../../plans/notif-cli/BRIEF.md).

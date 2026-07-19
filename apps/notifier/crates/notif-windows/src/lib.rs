//! Windows backend for the `notif` CLI.
//!
//! Session 1 scope: WinRT toast dispatch under a Tier 1 spoof AUMID
//! (`Microsoft.VisualStudioCode`) + `notif remove` via
//! `ToastNotificationHistory`. No CLSID registration, no `.lnk`, no callback
//! IPC, no focus solver — those land in later sessions.
//!
//! Verbose logging is exposed via [`init_logging`] and controlled by
//! `NOTIF_LOG_LEVEL` / `NOTIF_QUIET` / the CLI `--verbose` flag.

#[cfg(target_os = "windows")]
mod backend;
#[cfg(target_os = "windows")]
mod dispatch;
#[cfg(target_os = "windows")]
mod logging;
#[cfg(target_os = "windows")]
mod priority;
#[cfg(target_os = "windows")]
mod remove;

#[cfg(target_os = "windows")]
pub use backend::{WindowsBackend, WindowsError};
#[cfg(target_os = "windows")]
pub use dispatch::dispatch_send;
#[cfg(target_os = "windows")]
pub use logging::init_logging;
#[cfg(target_os = "windows")]
pub use remove::dispatch_remove;

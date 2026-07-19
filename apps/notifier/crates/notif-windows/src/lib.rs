//! Windows backend for the `notif` CLI.
//!
//! Session 1 : WinRT toast dispatch under a Tier 1 spoof AUMID + tracing
//! harness.
//! Session 2 : per-sender AUMID + CLSID derived from the sender key,
//! materialized as a `.lnk` under Start Menu + CLSID registered in HKCU, and
//! a self-install flow (`notif.exe --install`) that copies the binary into
//! `%LOCALAPPDATA%\notif\` and wires it into the user `Path`.
//! Later sessions : callback IPC (`--activator-serve`), focus solver.
//!
//! Verbose logging is exposed via [`init_logging`] and controlled by
//! `NOTIF_LOG_LEVEL` / `NOTIF_QUIET` / the CLI `--verbose` flag.

pub mod aumid;

#[cfg(target_os = "windows")]
mod activator;
#[cfg(target_os = "windows")]
mod backend;
#[cfg(target_os = "windows")]
mod callbacks;
#[cfg(target_os = "windows")]
mod dispatch;
#[cfg(target_os = "windows")]
mod install;
#[cfg(target_os = "windows")]
mod logging;
#[cfg(target_os = "windows")]
mod priority;
#[cfg(target_os = "windows")]
mod register;
#[cfg(target_os = "windows")]
mod remove;

#[cfg(target_os = "windows")]
pub use activator::run_activator_serve;
#[cfg(target_os = "windows")]
pub use backend::{WindowsBackend, WindowsError};
#[cfg(target_os = "windows")]
pub use dispatch::dispatch_send;
#[cfg(target_os = "windows")]
pub use install::{install_self, uninstall_self};
#[cfg(target_os = "windows")]
pub use logging::init_logging;
#[cfg(target_os = "windows")]
pub use register::{register_sender, Registration};
#[cfg(target_os = "windows")]
pub use remove::dispatch_remove;

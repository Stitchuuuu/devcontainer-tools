//! macOS backend for the `notif` CLI.
//!
//! # Architecture: single binary, two modes
//!
//! `UNUserNotificationCenter.current()` reads `[NSBundle mainBundle]` of the
//! *calling process* — so any code that dispatches a notification must itself
//! be running from inside the sender's `.app/Contents/MacOS/`.
//!
//! This crate implements that as two runtime modes selected by
//! [`dispatch::is_inner_mode`]:
//!
//! - **Outer** — `notif` on `$PATH`. Materializes the `.app` bundle,
//!   delegates to the bundled copy of itself via `open -W -a <bundle>`.
//! - **Inner** — `notif` under `<bundle>/Contents/MacOS/`. Calls
//!   `UNUserNotificationCenter`.
//!
//! # Tier model
//!
//! - **Tier 0** — reserved key `"default"`, cosmetic VS Code match
//!   (`CFBundleName = "Visual Studio Code"` + `code.icns`), our own
//!   `CFBundleIdentifier`.
//! - **Tier 2** — user-registered custom senders via `notif register`.
//!
//! Tier 1 (identity spoof) and Tier 3 (raw override) land in v0.2.
//!
//! On non-macOS targets this crate compiles to an empty shell so the workspace
//! dev-builds on the container's Linux host.

#[cfg(target_os = "macos")]
pub mod backend;
#[cfg(target_os = "macos")]
pub mod bundle;
#[cfg(target_os = "macos")]
pub mod clean;
#[cfg(target_os = "macos")]
pub mod dispatch;
#[cfg(target_os = "macos")]
pub mod error;
#[cfg(target_os = "macos")]
pub mod sender;

#[cfg(target_os = "macos")]
pub use backend::MacosBackend;
#[cfg(target_os = "macos")]
pub use error::MacosError;

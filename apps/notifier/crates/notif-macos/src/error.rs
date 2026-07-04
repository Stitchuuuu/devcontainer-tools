//! [`MacosError`] — the single error type surfaced by every function in
//! `notif-macos`.
//!
//! Uses `thiserror` for ergonomic `From` impls. The outer CLI at
//! `notif-cli::main` prints one line to stderr and exits 1.

use notif_core::SenderKeyError;
use thiserror::Error;

/// Everything that can go wrong in the macOS backend.
#[derive(Debug, Error)]
pub enum MacosError {
    /// I/O error while writing the `.app` bundle, reading `current_exe()`, etc.
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    /// Info.plist serialization failed.
    #[error("plist: {0}")]
    Plist(#[from] plist::Error),

    /// `$HOME` is unset — should never happen on a real Mac login session.
    #[error("$HOME is unset")]
    NoHome,

    /// Sender key failed [`notif_core::validate_sender_key`].
    #[error("sender key: {0}")]
    SenderKey(#[from] SenderKeyError),

    /// `codesign` (child process) failed. Payload is the trailing stderr line.
    #[error("codesign failed: {0}")]
    Codesign(String),

    /// UN center refused the request because the bundle is unsigned.
    /// The outer path catches this to invoke [`crate::bundle::ad_hoc_codesign`]
    /// and retry once.
    #[error("bundle not signed (macOS refuses UN dispatch)")]
    NotSigned,

    /// `requestAuthorization` completed but the user denied the permission
    /// (clicked "Don't Allow" or the app was already denied in TCC). The
    /// caller should surface a hint to grant in System Settings > Notifications.
    #[error("notification permission denied — enable in System Settings > Notifications")]
    AuthorizationDenied,

    /// UN center completion handler did not fire within the 2 s bound.
    /// The notification may still be delivered — treat as a soft warning.
    #[error("UN completion timed out")]
    Timeout,

    /// UN center returned an `NSError` with an unrecognized domain / code.
    #[error("objc: {0}")]
    Objc(String),

    /// `open -W -a <.app>` exited non-zero. Payload is the launched exit code.
    #[error("open(1) exited {0}")]
    OpenFailed(i32),
}

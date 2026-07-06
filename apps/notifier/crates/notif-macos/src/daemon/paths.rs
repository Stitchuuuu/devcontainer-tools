//! Paths for the `notif listen` daemon's Unix socket + PID file.
//!
//! Both live next to the `.app` bundles in `senders_root()` so a single
//! `notif clean` sweep already covers them (the bundle folder ownership
//! model is the natural directory for per-sender state).
//!
//! The socket is a Unix domain socket bound with mode `0700` — only the
//! current user reads / writes it. Rationale : callbacks can carry
//! sensitive strings (title / body / notification identifier) and the
//! macOS multi-user model is thin enough that limiting to $UID is the
//! defensible default.

use std::path::PathBuf;

use crate::error::MacosError;
use crate::sender::senders_root;

/// Unix socket path for the daemon that owns sender `key`.
///
/// # Errors
/// Returns [`MacosError::NoHome`] via [`senders_root`] if `$HOME` is unset.
/// Also returns [`MacosError::DaemonStartFailed`] with an explanatory
/// message if the resolved path would exceed the `sun_path` byte limit
/// (104 on macOS) — a pathological `$HOME` case that would silently
/// truncate at `bind()` time.
pub fn socket_path(key: &str) -> Result<PathBuf, MacosError> {
    let p = senders_root()?.join(format!("{key}.sock"));
    // POSIX sun_path is 108 on Linux, 104 on macOS. Reserve one byte for
    // the null terminator libc always includes.
    let bytes = p.as_os_str().len();
    if bytes >= 104 {
        return Err(MacosError::DaemonStartFailed(format!(
            "socket path is {bytes} bytes; sun_path limit is 104 on macOS. Try a shorter $HOME."
        )));
    }
    Ok(p)
}

/// PID file path for the daemon that owns sender `key`. Contains the
/// spawned daemon's PID as decimal ASCII with no trailing newline —
/// simple, easy to `pgrep -F`.
///
/// # Errors
/// Returns [`MacosError::NoHome`] via [`senders_root`].
pub fn pid_file(key: &str) -> Result<PathBuf, MacosError> {
    Ok(senders_root()?.join(format!("{key}.pid")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_ends_with_dot_sock() {
        // Only exercise happy path — $HOME is set in the test env.
        let p = socket_path("test-sender").unwrap();
        assert!(p.to_string_lossy().ends_with("/senders/test-sender.sock"));
    }

    #[test]
    fn pid_file_ends_with_dot_pid() {
        let p = pid_file("test-sender").unwrap();
        assert!(p.to_string_lossy().ends_with("/senders/test-sender.pid"));
    }
}

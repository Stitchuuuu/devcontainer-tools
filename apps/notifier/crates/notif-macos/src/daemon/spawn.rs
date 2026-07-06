//! Auto-spawn the daemon detached from the caller. Callers first probe
//! the socket ; if `UnixStream::connect` succeeds, the daemon is up and
//! nothing further is needed. Otherwise materialize the bundle, seed
//! LaunchServices, and `setsid`-detach the inner-mode `notif listen` so
//! it survives when `notif send` exits.
//!
//! The detached process closes stdin/stdout/stderr so a shell that
//! spawned the daemon can exit without leaving a hanging tty ; log
//! output routes through `NOTIF_LOG` (inherited env) when set.

use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::bundle::ensure_bundle;
use crate::daemon::paths::socket_path;
use crate::error::MacosError;

/// Max wall-clock spent polling the socket for readiness after spawn.
const SPAWN_POLL_TIMEOUT: Duration = Duration::from_secs(3);
/// Poll interval while waiting for `bind()` to land.
const SPAWN_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Ensure a `notif listen` daemon is running for `sender_key`. On
/// return the socket accepts connections (or the function bubbles an
/// error).
///
/// Idempotent — a live daemon is left alone. Repeated calls in quick
/// succession are safe : only the first invocation that finds the socket
/// missing goes through the spawn path ; the rest see the socket up.
///
/// # Errors
/// [`MacosError::Io`] on bundle materialization / spawn failure ;
/// [`MacosError::DaemonStartFailed`] if the socket doesn't accept
/// within [`SPAWN_POLL_TIMEOUT`] after spawn.
pub fn ensure_running(sender_key: &str) -> Result<PathBuf, MacosError> {
    let sock = socket_path(sender_key)?;
    if UnixStream::connect(&sock).is_ok() {
        return Ok(sock);
    }
    // Bundle must exist before we can spawn the inner binary from inside
    // it (delegate + UN center only work under a `.app` mainBundle).
    let bundle = ensure_bundle(sender_key, sender_key, None, None)?;
    let inner = bundle.join("Contents/MacOS/notif");
    if !inner.exists() {
        return Err(MacosError::DaemonStartFailed(format!(
            "inner exe missing after ensure_bundle: {}",
            inner.display()
        )));
    }
    spawn_detached(&inner, sender_key)?;
    wait_for_socket(&sock)?;
    Ok(sock)
}

/// Spawn `<inner> listen --sender <sender_key>` fully detached from the
/// caller : new session (`setsid`), all three stdio streams closed.
fn spawn_detached(inner: &Path, sender_key: &str) -> Result<(), MacosError> {
    let mut cmd = Command::new(inner);
    cmd.arg("listen").arg("--sender").arg(sender_key);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            // New process group + session : detaches from the parent's
            // controlling tty, so a SIGHUP on the parent shell does not
            // reach the daemon.
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.spawn()
        .map(|_child| ())
        .map_err(|e| MacosError::DaemonStartFailed(format!("spawn: {e}")))
}

fn wait_for_socket(sock: &Path) -> Result<(), MacosError> {
    let start = Instant::now();
    while start.elapsed() < SPAWN_POLL_TIMEOUT {
        if UnixStream::connect(sock).is_ok() {
            return Ok(());
        }
        std::thread::sleep(SPAWN_POLL_INTERVAL);
    }
    Err(MacosError::DaemonStartFailed(format!(
        "socket {} did not accept within {:?}",
        sock.display(),
        SPAWN_POLL_TIMEOUT
    )))
}

/// Foreground entry for `notif listen` — user typed the command
/// directly, so we materialize the bundle and exec into the inner-mode
/// binary. The inner-mode process blocks on the runloop.
///
/// Does NOT detach. Contrast with [`ensure_running`] which is called
/// from `notif send` and must fire-and-forget.
///
/// # Errors
/// Bubbles anything from [`ensure_bundle`] plus process spawn failure.
pub fn listen_outer(sender_key: &str, idle_arg: &str) -> Result<(), MacosError> {
    let bundle = ensure_bundle(sender_key, sender_key, None, None)?;
    let inner = bundle.join("Contents/MacOS/notif");
    let mut cmd = Command::new(inner);
    cmd.arg("listen").arg("--sender").arg(sender_key)
        .arg("--idle-timeout").arg(idle_arg);
    let status = cmd.status()?;
    if !status.success() {
        return Err(MacosError::OpenFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

//! `notif listen` daemon — per-sender long-lived process owning the UN
//! center delegate and the callback registry.
//!
//! Session 7b of the notif-cli rollout. See
//! `plans/notif-cli/EXISTING.md § notif listen daemon architecture` for
//! the full design.
//!
//! # Runtime layout
//!
//! - **Main thread** (daemon's process main) — installs the
//!   [`NotifDelegate`][delegate::NotifDelegate], binds the Unix socket +
//!   PID file, then drains the main queue via
//!   `CFRunLoopRunInMode(defaultMode, 1s, false)` so UN center delegate
//!   callbacks (delivered on the main queue by Apple) actually run. On
//!   each iteration checks the shutdown flag AND the idle-timeout.
//! - **Accept thread** — non-blocking `accept()` polls the socket, spawns
//!   one thread per accepted connection.
//! - **Per-connection threads** — decode a [`proto::Request`], route to
//!   [`server::handle_send_notif`] (which calls `dispatch_inner` +
//!   registers the callback binding) or [`server::handle_shutdown`], then
//!   emit a [`proto::Response`] and drop the stream.
//!
//! Callback delivery : delegate fires on the main thread ; a match in
//! the [`registry::Registry`] hands off to
//! [`notif_core::callback::fire`] which routes on target kind to hook /
//! url / file dispatchers.

pub mod client;
pub mod paths;
pub mod proto;
pub mod registry;

#[cfg(target_os = "macos")]
pub mod delegate;
#[cfg(target_os = "macos")]
pub mod server;
#[cfg(target_os = "macos")]
pub mod spawn;

#[cfg(target_os = "macos")]
pub use spawn::{ensure_running, listen_outer};

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::error::MacosError;

/// How often the main runloop wakes to check shutdown + idle-timeout.
const RUNLOOP_TICK: f64 = 1.0;

/// Ask a running daemon to shut down, then return.
///
/// # Errors
/// [`MacosError::Io`] on socket-connect failure other than "not found" /
/// "refused" (which mean the daemon is already gone — treated as
/// success).
pub fn shutdown(sender_key: &str) -> Result<(), MacosError> {
    let sock = paths::socket_path(sender_key)?;
    client::shutdown(&sock)
}

/// Run the daemon on the calling thread. Blocks until either
/// [`Request::Shutdown`][proto::Request::Shutdown] fires or the
/// idle-timeout elapses with an empty registry.
///
/// The process must already be running inside a `.app` bundle
/// (`[NSBundle mainBundle]` must match the sender) — enforced by
/// [`crate::dispatch::is_inner_mode`] at the CLI layer.
///
/// # Errors
/// [`MacosError::Io`] on socket bind / PID file write / cleanup failure.
#[cfg(target_os = "macos")]
pub fn run_daemon(sender_key: &str, idle_timeout: Duration) -> Result<(), MacosError> {
    let socket = paths::socket_path(sender_key)?;
    let pid_path = paths::pid_file(sender_key)?;

    // Parent dir already exists (bundle materialization ensures
    // `senders/` before spawning us). Best-effort create anyway to
    // survive an out-of-order manual `notif listen` invocation.
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write PID file — best-effort ; a lock contention here is
    // recoverable by a manual restart.
    std::fs::write(&pid_path, std::process::id().to_string())?;

    let registry = registry::Registry::new();
    delegate::install(registry.clone());

    let state = server::ServerState::new(registry.clone());
    // Listener kept alive for the duration of the daemon.
    let _listener = server::spawn_accept_loop(&socket, state.clone())?;

    notif_core::warn::stderr(&format!(
        "notif listen started for sender={sender_key} pid={} socket={} idle_timeout={:?}",
        std::process::id(),
        socket.display(),
        idle_timeout,
    ));

    // Main thread : drain the main queue (delegate callbacks) between
    // idle / shutdown checks.
    main_loop(&state, idle_timeout);

    notif_core::warn::stderr(&format!(
        "notif listen shutting down (sender={sender_key})",
    ));

    // Cleanup — best-effort.
    let _ = std::fs::remove_file(&socket);
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

#[cfg(target_os = "macos")]
fn main_loop(state: &server::ServerState, idle_timeout: Duration) {
    use objc2_core_foundation::{kCFRunLoopDefaultMode, CFRunLoop};
    loop {
        // Drain the main queue for up to RUNLOOP_TICK seconds. This is
        // what actually delivers UN center delegate callbacks — without
        // running the runloop, the delegate methods never fire.
        unsafe {
            let _ = CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, RUNLOOP_TICK, false);
        }
        if state.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let idle = {
            let last = *state.last_activity.lock().unwrap();
            Instant::now().duration_since(last)
        };
        if state.registry.is_empty() && idle >= idle_timeout {
            notif_core::warn::stderr(&format!(
                "notif listen idle for {:?} with empty registry — exiting",
                idle,
            ));
            return;
        }
    }
}

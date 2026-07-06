//! Client wrapper around a `UnixStream` to the running daemon.
//!
//! One-shot per request : each call opens a fresh connection, writes one
//! request frame, reads one response frame, drops the stream. Rationale :
//! `notif send` invocations are short-lived and independent — pooling
//! would only save a syscall and adds shutdown-race complexity.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use notif_core::callback::{CallbackConfig, CallbackPayload};

use crate::daemon::proto::{self, Request, Response, SendPayload};
use crate::error::MacosError;
use crate::overrides::MacosOverrides;

/// Read timeout applied to every request. Auth-pending sends still ack
/// within milliseconds — the daemon's actual auth-wait happens after
/// the ack. Long callback delivery latencies do NOT block the ack.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Send `SendNotif` to a running daemon and return the ack.
///
/// # Errors
/// [`MacosError::Io`] on socket connect/read failure ;
/// [`MacosError::DaemonProtocol`] on wire error ; wraps
/// [`Response::Err`] as [`MacosError::DaemonProtocol`].
pub fn send_notif(
    socket: &Path,
    notif: SendPayload,
    overrides: MacosOverrides,
    callbacks: CallbackConfig,
    payload_context: CallbackPayload,
) -> Result<Response, MacosError> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let req = Request::SendNotif { notif, overrides, callbacks, payload_context };
    proto::write_frame(&mut stream, &req)?;
    let resp: Response = proto::read_frame(&mut stream)?;
    match resp {
        Response::Err { ref msg } => Err(MacosError::DaemonProtocol(format!(
            "daemon rejected send_notif: {msg}"
        ))),
        other => Ok(other),
    }
}

/// Send `Shutdown` to a running daemon and wait for the ack (or a
/// clean disconnect). Idempotent — a socket that doesn't respond within
/// [`READ_TIMEOUT`] is treated as already-shutdown.
///
/// # Errors
/// [`MacosError::Io`] on connect failure (typically `NotFound` = daemon
/// already gone, mapped to `Ok(())`).
pub fn shutdown(socket: &Path) -> Result<(), MacosError> {
    match UnixStream::connect(socket) {
        Ok(mut stream) => {
            stream.set_read_timeout(Some(READ_TIMEOUT))?;
            proto::write_frame(&mut stream, &Request::Shutdown)?;
            // Try to read the ack, but any error here is fine — the
            // daemon may already have closed the socket.
            let _: Result<Response, _> = proto::read_frame(&mut stream);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound
            || e.kind() == io::ErrorKind::ConnectionRefused =>
        {
            // No daemon at that path — nothing to shut down.
            Ok(())
        }
        Err(e) => Err(MacosError::Io(e)),
    }
}

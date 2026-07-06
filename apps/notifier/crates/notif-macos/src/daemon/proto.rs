//! Wire protocol for the `notif listen` daemon.
//!
//! Framing : `u32-BE length prefix` + `JSON body`. Length is the byte
//! length of the JSON body ; frames are capped at [`MAX_FRAME_SIZE`] to
//! prevent a malformed length header from allocating gigabytes.
//!
//! # Request variants
//!
//! - [`Request::SendNotif`] — outer sends the full notification payload +
//!   overrides + callback config + payload context. Daemon acknowledges
//!   with [`Response::Ack`] carrying the dispatched notification id, then
//!   asynchronously calls [`crate::dispatch::inner::dispatch_inner`] on
//!   the main thread of its bundle (via the CFRunLoop) and registers the
//!   callback binding in the shared [`super::registry::Registry`].
//! - [`Request::Shutdown`] — client asks the daemon to exit cleanly.
//!   Daemon acks then breaks the main-loop.
//!
//! # Response variants
//!
//! - [`Response::Ack`] — request accepted for eventual delivery.
//! - [`Response::PendingAuth`] — daemon is still waiting for the user to
//!   click "Allow" on the notification permission dialog. Request is
//!   queued and will be dispatched once auth lands. Reserved for the
//!   long-lived auth case — the current implementation returns Ack even
//!   for a fresh sender because the daemon owns the auth wait entirely.
//! - [`Response::Err`] — malformed request, unsupported variant, or
//!   dispatch failure that surfaced synchronously.

use std::io::{Read, Write};

use notif_core::callback::{CallbackConfig, CallbackPayload};
use serde::{Deserialize, Serialize};

use crate::error::MacosError;
use crate::overrides::MacosOverrides;

/// Hard cap on a single frame's JSON body. Any protocol violation that
/// yields a length ≥ this refuses the read with [`MacosError::DaemonProtocol`].
///
/// Practical frames are ≤ 8 KiB (notification title/body + a few
/// callback targets). 1 MiB is generous and picked to leave headroom for
/// future field growth without exposing the daemon to DoS via giant
/// length prefixes on a rogue socket.
pub const MAX_FRAME_SIZE: usize = 1 << 20;

/// Portable subset of `Notification` we send over the wire. Mirrors the
/// fields `dispatch_inner` reads via CLI args today. Keeping this shape
/// distinct from the (larger) portable `Notification` lets us evolve
/// each without breaking the other.
///
/// String types are used for enum fields (`priority`, `sound`) so the
/// wire format stays stable when the enums grow variants — the daemon
/// parses them back to their strong types on ingress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPayload {
    pub title: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub priority: String, // Priority::wire_str()
    pub sender_key: String,
    pub id: Option<String>,
    pub sound: Option<String>, // Sound::wire_str()
    pub image: Option<String>, // PathBuf as UTF-8 string
}

/// Request variants sent from `notif send` (outer) → daemon.
///
/// `SendNotif` carries ~500 bytes vs 0 for `Shutdown`. Boxing every
/// field would add pointer chases on every access without any perf
/// benefit — the enum is decoded once per socket connection and
/// consumed immediately. Suppress `clippy::large_enum_variant`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
#[allow(clippy::large_enum_variant)]
pub enum Request {
    /// Full send request. Daemon fires the UN notification AND registers
    /// the callback binding.
    #[serde(rename = "send_notif")]
    SendNotif {
        notif: SendPayload,
        overrides: MacosOverrides,
        callbacks: CallbackConfig,
        payload_context: CallbackPayload,
    },
    /// Ask the daemon to exit cleanly.
    #[serde(rename = "shutdown")]
    Shutdown,
}

/// Response variants from the daemon → client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op")]
pub enum Response {
    /// Request accepted. `dispatched_id` is the notification identifier
    /// the daemon will use (either the caller's `--id` or a fresh UUID).
    #[serde(rename = "ack")]
    Ack { dispatched_id: String },
    /// Request accepted but delivery is blocked on the user granting
    /// notification permission. Included in the protocol for future use
    /// — session 7b's implementation Acks immediately even in this case
    /// because the actual auth wait happens post-ack inside the daemon.
    #[serde(rename = "pending_auth")]
    PendingAuth { dispatched_id: String },
    /// Synchronous failure. `msg` is a short human string safe to surface.
    #[serde(rename = "err")]
    Err { msg: String },
}

/// Read one length-prefixed JSON frame. Blocks on `r` until the full
/// frame is read or EOF / error.
///
/// # Errors
/// Returns [`MacosError::Io`] on read failure, [`MacosError::DaemonProtocol`]
/// on frame-size violation, and [`MacosError::DaemonProtocol`] on invalid
/// JSON.
pub fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> Result<T, MacosError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(MacosError::DaemonProtocol(format!(
            "frame length {len} exceeds MAX_FRAME_SIZE {MAX_FRAME_SIZE}"
        )));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|e| MacosError::DaemonProtocol(format!("JSON decode: {e}")))
}

/// Serialize `msg` as JSON, prefix with a `u32-BE` length header, and
/// write both to `w`. Flushes after the write so short-lived callers can
/// `drop` the writer without losing data.
///
/// # Errors
/// [`MacosError::Io`] on write failure, [`MacosError::DaemonProtocol`] if
/// the serialized body exceeds [`MAX_FRAME_SIZE`] (guards against
/// pathological future payload growth).
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> Result<(), MacosError> {
    let body = serde_json::to_vec(msg)
        .map_err(|e| MacosError::DaemonProtocol(format!("JSON encode: {e}")))?;
    if body.len() > MAX_FRAME_SIZE {
        return Err(MacosError::DaemonProtocol(format!(
            "frame length {} exceeds MAX_FRAME_SIZE {MAX_FRAME_SIZE}",
            body.len()
        )));
    }
    let len = (body.len() as u32).to_be_bytes();
    w.write_all(&len)?;
    w.write_all(&body)?;
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notif_core::callback::{CallbackKind, CallbackTarget};
    use std::io::Cursor;

    fn sample_payload() -> CallbackPayload {
        CallbackPayload {
            notif_id: "abc".into(),
            event: String::new(),
            sender: "default".into(),
            title: "T".into(),
            body: "B".into(),
            ts: "2026-07-06T09:00:00Z".into(),
        }
    }

    fn sample_send() -> SendPayload {
        SendPayload {
            title: "T".into(),
            body: "B".into(),
            subtitle: None,
            priority: "normal".into(),
            sender_key: "default".into(),
            id: Some("abc".into()),
            sound: None,
            image: None,
        }
    }

    #[test]
    fn ack_frame_roundtrip() {
        let msg = Response::Ack { dispatched_id: "abc-123".into() };
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).unwrap();
        let mut cur = Cursor::new(buf);
        let back: Response = read_frame(&mut cur).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn send_notif_frame_roundtrip() {
        let msg = Request::SendNotif {
            notif: sample_send(),
            overrides: MacosOverrides {
                sound_name: Some("Ping".into()),
                attachment: None,
                interruption_level: None,
                thread_identifier: None,
                category_identifier: None,
            },
            callbacks: CallbackConfig {
                on_click: Some(CallbackTarget { kind: CallbackKind::File, payload: "/tmp/x".into() }),
                on_actions: vec![("reply".into(), CallbackTarget { kind: CallbackKind::Hook, payload: "echo".into() })],
                on_dismiss: None,
                on_timeout: None,
            },
            payload_context: sample_payload(),
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).unwrap();
        let mut cur = Cursor::new(buf);
        let back: Request = read_frame(&mut cur).unwrap();
        match back {
            Request::SendNotif { notif, callbacks, .. } => {
                assert_eq!(notif.title, "T");
                assert_eq!(callbacks.on_actions.len(), 1);
                assert_eq!(callbacks.on_actions[0].0, "reply");
            }
            other => panic!("unexpected variant {other:?}"),
        }
    }

    #[test]
    fn shutdown_frame_roundtrip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &Request::Shutdown).unwrap();
        let mut cur = Cursor::new(buf);
        let back: Request = read_frame(&mut cur).unwrap();
        assert!(matches!(back, Request::Shutdown));
    }

    #[test]
    fn oversized_length_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(MAX_FRAME_SIZE as u32 + 1).to_be_bytes());
        let mut cur = Cursor::new(buf);
        let res: Result<Request, _> = read_frame(&mut cur);
        assert!(matches!(res, Err(MacosError::DaemonProtocol(_))));
    }
}

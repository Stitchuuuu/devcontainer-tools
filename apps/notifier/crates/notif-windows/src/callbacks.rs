//! Callback sidecar + actions inbox for the Windows COM activator flow.
//!
//! Wire diagram :
//!
//! ```text
//!  notif send                                 notif.exe --activator-serve
//!    │                                                     │
//!    ├─► write_sidecar(<id>.json)                          │
//!    │     %LOCALAPPDATA%\notif\callbacks\<id>.json        │
//!    │     { sender, title, body, ts, callbacks }          │
//!    │                                                     │
//!    ▼                                                     ▼
//!  dispatch toast <toast launch="body::<id>">      Activate(invoked_args)
//!                                                          │
//!                                                          ├─► parse_invoked_args
//!                                                          ├─► read_sidecar(<id>)
//!                                                          ├─► notif_core::callback::fire
//!                                                          ├─► append_inbox(<sender>.jsonl)
//!                                                          └─► delete_sidecar(<id>)
//! ```
//!
//! macOS has no equivalent — the always-running `notif listen` daemon holds
//! the callback registry in-memory. Windows has no daemon on the callback
//! path (Explorer spawns `--activator-serve` on demand via LocalServer32),
//! so the sidecar bridges the send process and the click-handling process.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use notif_core::callback::{CallbackConfig, CallbackEvent, CallbackPayload};
use notif_core::Notification;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::{debug, warn};

use crate::backend::WindowsError;

/// Everything the activator needs to reconstruct the callback payload at
/// click time. Serialized to JSON, one file per `notif_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub sender: String,
    pub title: String,
    pub body: String,
    pub ts: String,
    pub callbacks: CallbackConfig,
}

/// Write `<notif_id>.json` under `%LOCALAPPDATA%\notif\callbacks\`.
/// Atomic (write-tmp + rename). No-op when `callbacks.is_empty()`.
pub fn write_sidecar(
    notif_id: &str,
    sender: &str,
    notif: &Notification,
    callbacks: &CallbackConfig,
) -> Result<(), WindowsError> {
    if callbacks.is_empty() {
        return Ok(());
    }
    let dir = callbacks_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|e| WindowsError::plain(format!("mkdir callbacks: {e}")))?;
    let ts = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));
    let sidecar = Sidecar {
        sender: sender.to_string(),
        title: notif.title.clone(),
        body: notif.body.clone(),
        ts,
        callbacks: callbacks.clone(),
    };
    let json = serde_json::to_string(&sidecar)
        .map_err(|e| WindowsError::plain(format!("serialize sidecar: {e}")))?;
    let sanitized = sanitize_filename(notif_id);
    let dest = dir.join(format!("{sanitized}.json"));
    let tmp = dir.join(format!("{sanitized}.json.tmp"));
    fs::write(&tmp, json.as_bytes())
        .map_err(|e| WindowsError::plain(format!("write sidecar tmp: {e}")))?;
    fs::rename(&tmp, &dest)
        .map_err(|e| WindowsError::plain(format!("rename sidecar: {e}")))?;
    debug!(target: "notif::callbacks", path = %dest.display(), "sidecar written");
    Ok(())
}

/// Read the sidecar at `<notif_id>.json`. Returns `Ok(None)` when the file
/// is absent (expected when the toast was sent before session-3 install, or
/// when the sidecar was already GC'd). Malformed JSON logs a warn and returns
/// `Ok(None)` so the activator can no-op cleanly.
pub fn read_sidecar(notif_id: &str) -> Result<Option<Sidecar>, WindowsError> {
    let path = sidecar_path(notif_id)?;
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!(target: "notif::callbacks", notif_id, "sidecar missing");
            return Ok(None);
        }
        Err(e) => {
            return Err(WindowsError::plain(format!(
                "read sidecar {}: {e}",
                path.display(),
            )));
        }
    };
    match serde_json::from_str::<Sidecar>(&text) {
        Ok(s) => Ok(Some(s)),
        Err(e) => {
            warn!(target: "notif::callbacks", notif_id, error = %e, "sidecar malformed");
            Ok(None)
        }
    }
}

/// Best-effort sidecar delete after fire. Missing file is fine; other IO
/// errors are logged and swallowed so the activator's exit path stays
/// clean.
pub fn delete_sidecar(notif_id: &str) {
    let Ok(path) = sidecar_path(notif_id) else { return };
    if let Err(e) = fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(target: "notif::callbacks", notif_id, error = %e, "sidecar delete failed");
        }
    }
}

/// Append one JSONL line to `%LOCALAPPDATA%\notif\actions-inbox\<sender>.jsonl`.
/// Same schema as [`notif_core::callback::CallbackPayload`] — the file is
/// what `notify-app.js` will tail post-session-5.
pub fn append_inbox(sender: &str, payload: &CallbackPayload) -> Result<(), WindowsError> {
    let dir = inbox_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|e| WindowsError::plain(format!("mkdir inbox: {e}")))?;
    let sanitized = sanitize_filename(sender);
    let path = dir.join(format!("{sanitized}.jsonl"));
    let mut line = serde_json::to_string(payload)
        .map_err(|e| WindowsError::plain(format!("serialize payload: {e}")))?;
    line.push('\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| WindowsError::plain(format!("open inbox {}: {e}", path.display())))?;
    file.write_all(line.as_bytes())
        .map_err(|e| WindowsError::plain(format!("write inbox {}: {e}", path.display())))?;
    debug!(target: "notif::callbacks", path = %path.display(), "inbox appended");
    Ok(())
}

/// Parse the `invoked_args` string the toast XML embeds. Session-3 shapes :
///
/// - `body::<id>` — body-click.
/// - `action:<label>::<id>` — custom action button.
///
/// Returns `None` on any other shape (defensive — Windows can in theory
/// deliver empty args for legacy code paths).
#[must_use]
pub fn parse_invoked_args(args: &str) -> Option<(CallbackEvent, String)> {
    if let Some(id) = args.strip_prefix("body::") {
        if id.is_empty() {
            return None;
        }
        return Some((CallbackEvent::Click, id.to_string()));
    }
    if let Some(rest) = args.strip_prefix("action:") {
        // rest = `<label>::<id>` — split on the last `::` since labels can in
        // theory contain `:`.
        let idx = rest.rfind("::")?;
        let (label, id_with_sep) = rest.split_at(idx);
        let id = &id_with_sep[2..];
        if label.is_empty() || id.is_empty() {
            return None;
        }
        return Some((CallbackEvent::Action(label.to_string()), id.to_string()));
    }
    None
}

/// Reverse of `parse_invoked_args` for the click path.
#[must_use]
pub fn click_launch_attr(notif_id: &str) -> String {
    format!("body::{notif_id}")
}

/// Reverse of `parse_invoked_args` for the action-button path.
#[must_use]
pub fn action_arguments(label: &str, notif_id: &str) -> String {
    format!("action:{label}::{notif_id}")
}

fn callbacks_dir() -> Result<PathBuf, WindowsError> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| WindowsError::plain("LOCALAPPDATA env missing"))?;
    Ok(PathBuf::from(local).join("notif").join("callbacks"))
}

fn inbox_dir() -> Result<PathBuf, WindowsError> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| WindowsError::plain("LOCALAPPDATA env missing"))?;
    Ok(PathBuf::from(local).join("notif").join("actions-inbox"))
}

fn sidecar_path(notif_id: &str) -> Result<PathBuf, WindowsError> {
    let sanitized = sanitize_filename(notif_id);
    Ok(callbacks_dir()?.join(format!("{sanitized}.json")))
}

/// Replace filesystem-hostile chars with `_`. Filenames on Windows cannot
/// contain `< > : " / \ | ? *`; on top of that any `.` sequences resolve
/// to parent dirs. Since callers may pass user-provided ids, sanitize
/// aggressively — keep only `[A-Za-z0-9_-]`, everything else → `_`.
fn sanitize_filename(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use notif_core::callback::{CallbackKind, CallbackTarget};

    fn sample_notif() -> Notification {
        Notification {
            title: "T".into(),
            body: "B".into(),
            subtitle: None,
            priority: notif_core::Priority::Normal,
            sender: notif_core::Sender::default(),
            id: Some("abc-123".into()),
            sound: None,
            image: None,
            on_timeout: None,
        }
    }

    fn sample_callbacks() -> CallbackConfig {
        CallbackConfig {
            on_click: Some(CallbackTarget {
                kind: CallbackKind::File,
                payload: "/tmp/click".into(),
            }),
            on_actions: vec![(
                "Allow".into(),
                CallbackTarget {
                    kind: CallbackKind::Hook,
                    payload: "echo allow".into(),
                },
            )],
            on_dismiss: None,
            on_timeout: None,
        }
    }

    #[test]
    fn parse_body_click() {
        let (ev, id) = parse_invoked_args("body::abc-123").unwrap();
        assert_eq!(ev, CallbackEvent::Click);
        assert_eq!(id, "abc-123");
    }

    #[test]
    fn parse_action_button() {
        let (ev, id) = parse_invoked_args("action:Allow::abc-123").unwrap();
        assert_eq!(ev, CallbackEvent::Action("Allow".into()));
        assert_eq!(id, "abc-123");
    }

    #[test]
    fn parse_action_label_with_colon() {
        // Split on the LAST `::` so labels can contain `:`.
        let (ev, id) = parse_invoked_args("action:foo:bar::x").unwrap();
        assert_eq!(ev, CallbackEvent::Action("foo:bar".into()));
        assert_eq!(id, "x");
    }

    #[test]
    fn parse_empty_id_rejected() {
        assert!(parse_invoked_args("body::").is_none());
        assert!(parse_invoked_args("action:Allow::").is_none());
    }

    #[test]
    fn parse_unknown_shape_rejected() {
        assert!(parse_invoked_args("").is_none());
        assert!(parse_invoked_args("dismiss").is_none());
        assert!(parse_invoked_args("random_junk").is_none());
    }

    #[test]
    fn launch_attr_roundtrips_with_parse() {
        let attr = click_launch_attr("id-1");
        assert_eq!(attr, "body::id-1");
        let (ev, id) = parse_invoked_args(&attr).unwrap();
        assert_eq!(ev, CallbackEvent::Click);
        assert_eq!(id, "id-1");
    }

    #[test]
    fn action_arguments_roundtrips_with_parse() {
        let args = action_arguments("Allow", "id-1");
        assert_eq!(args, "action:Allow::id-1");
        let (ev, id) = parse_invoked_args(&args).unwrap();
        assert_eq!(ev, CallbackEvent::Action("Allow".into()));
        assert_eq!(id, "id-1");
    }

    #[test]
    fn sanitize_replaces_hostile_chars() {
        assert_eq!(sanitize_filename("abc-123"), "abc-123");
        assert_eq!(sanitize_filename("../etc/passwd"), "___etc_passwd");
        assert_eq!(sanitize_filename("foo:bar\\baz"), "foo_bar_baz");
        assert_eq!(sanitize_filename(""), "_");
    }

    #[test]
    fn sidecar_json_roundtrip() {
        // Serde round-trip locks the on-disk schema without touching the
        // filesystem — the write/read path is exercised by the LOCALAPPDATA
        // integration test on Windows CI.
        let sc = Sidecar {
            sender: "vscode".into(),
            title: "T".into(),
            body: "B".into(),
            ts: "2026-07-19T00:00:00Z".into(),
            callbacks: sample_callbacks(),
        };
        let json = serde_json::to_string(&sc).unwrap();
        let round: Sidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(round, sc);
    }

    #[test]
    fn write_sidecar_skips_when_callbacks_empty() {
        // Contract : no filesystem side-effect for a notif with zero
        // registered callbacks.
        let notif = sample_notif();
        let dir_before = callbacks_dir().ok();
        let empty = CallbackConfig::default();
        let _ = write_sidecar("noop-id", "default", &notif, &empty);
        // Best-effort assertion : if the dir existed before, `noop-id.json`
        // must not appear. We can't assert absolute non-existence of the
        // dir itself (a concurrent test may have created it), so scope
        // the check to our own file.
        if let Some(dir) = dir_before {
            let candidate = dir.join("noop-id.json");
            assert!(
                !candidate.exists(),
                "empty callbacks must not touch the filesystem",
            );
        }
    }
}

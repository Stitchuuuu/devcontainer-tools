//! Portable callback engine — types + target parsing.
//!
//! Consumers describe *what happens* when the user interacts with a
//! delivered notification: click on the body ([`CallbackEvent::Click`]),
//! click a custom action ([`CallbackEvent::Action`]), dismiss the banner
//! ([`CallbackEvent::Dismiss`]), or let it time out
//! ([`CallbackEvent::Timeout`]). Each event is bound to a
//! [`CallbackTarget`] which selects one of three dispatch mechanisms:
//!
//! - **`hook:<path> [args…]`** — exec a subprocess with the payload on
//!   stdin (JSON).
//! - **`url:<https?://…>`** — HTTP POST the payload as JSON.
//! - **`file:<abs-path>`** — append the payload as one JSONL line.
//!
//! Prefix-less targets auto-detect (URL scheme → [`Url`][CallbackKind::Url];
//! absolute path → [`File`][CallbackKind::File]; else
//! [`Hook`][CallbackKind::Hook]) so callers can pass a bare `/tmp/log.jsonl`
//! or `https://cb.example.com/x`.
//!
//! [`fire`] is currently a no-op stub — the real dispatchers (HTTP client,
//! subprocess exec, file append) live in the `notif listen` daemon and
//! run against the same signature so the on-disk contract stays stable.
//!
//! [`Url`]: CallbackKind::Url
//! [`File`]: CallbackKind::File
//! [`Hook`]: CallbackKind::Hook

use std::fmt;

use serde::{Deserialize, Serialize};

/// Which dispatch mechanism a [`CallbackTarget`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackKind {
    /// Exec a subprocess (`hook:<argv>`), stream the payload on stdin.
    Hook,
    /// HTTP POST the payload as JSON (`url:<https?://…>`).
    Url,
    /// Append the payload as one JSONL line (`file:<abs-path>`).
    File,
}

impl CallbackKind {
    /// Wire-format prefix (`hook`, `url`, `file`).
    #[must_use]
    pub const fn wire_prefix(self) -> &'static str {
        match self {
            Self::Hook => "hook",
            Self::Url => "url",
            Self::File => "file",
        }
    }
}

/// One parsed callback target — a dispatch mechanism plus the payload the
/// mechanism consumes (subprocess argv, URL, or file path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackTarget {
    pub kind: CallbackKind,
    /// Prefix-stripped payload. For [`Hook`][CallbackKind::Hook] the
    /// full argv line (split by the dispatcher at fire time); for
    /// [`Url`][CallbackKind::Url] the full URL; for
    /// [`File`][CallbackKind::File] the absolute path.
    pub payload: String,
}

impl CallbackTarget {
    /// Canonical wire form for the outer→inner CLI hop
    /// (`hook:<payload>` / `url:<payload>` / `file:<payload>`).
    ///
    /// Round-trips through [`parse_target`] — the outer serializes with
    /// this and the inner parses it back to the same [`CallbackTarget`],
    /// including auto-detect targets which are canonicalized on the way
    /// out.
    #[must_use]
    pub fn to_wire(&self) -> String {
        format!("{}:{}", self.kind.wire_prefix(), self.payload)
    }
}

/// Ways [`parse_target`] can reject an input string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackParseError {
    /// Whitespace-only or empty.
    Empty,
    /// `hook:` with no argv after the prefix.
    HookEmpty,
    /// `url:<x>` where `<x>` is not `http://…` / `https://…`.
    UrlBadScheme(String),
    /// `file:<x>` where `<x>` is not an absolute path.
    FileNotAbsolute(String),
}

impl fmt::Display for CallbackParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("callback target is empty"),
            Self::HookEmpty => f.write_str("`hook:` requires an argv line after the prefix"),
            Self::UrlBadScheme(v) => write!(
                f,
                "`url:` requires an http:// or https:// URL (got {v:?})",
            ),
            Self::FileNotAbsolute(v) => write!(
                f,
                "`file:` requires an absolute path (got {v:?})",
            ),
        }
    }
}

impl std::error::Error for CallbackParseError {}

/// Parse a raw `--on-*` argument into a canonical [`CallbackTarget`].
///
/// The three explicit prefixes (`hook:` / `url:` / `file:`) are tried
/// first, in that order. If none matches, the auto-detect heuristic
/// picks a kind:
///
/// 1. `http://` / `https://` → [`Url`][CallbackKind::Url]
/// 2. `/…` (absolute path) → [`File`][CallbackKind::File]
/// 3. else → [`Hook`][CallbackKind::Hook]
///
/// Auto-detect never fails — [`Hook`][CallbackKind::Hook] is the fallback,
/// so a bare `notif` invocation with `--on-click "echo hi"` produces a
/// hook target on `"echo hi"`. Explicit prefixes can fail validation
/// (`hook:` empty, `url:` wrong scheme, `file:` not absolute).
///
/// # Errors
///
/// Returns [`CallbackParseError`] when the input is empty or when an
/// explicit prefix rejects its payload.
pub fn parse_target(raw: &str) -> Result<CallbackTarget, CallbackParseError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CallbackParseError::Empty);
    }

    if let Some(rest) = raw.strip_prefix("hook:") {
        let payload = rest.trim();
        if payload.is_empty() {
            return Err(CallbackParseError::HookEmpty);
        }
        return Ok(CallbackTarget { kind: CallbackKind::Hook, payload: payload.to_string() });
    }
    if let Some(rest) = raw.strip_prefix("url:") {
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            return Err(CallbackParseError::UrlBadScheme(rest.to_string()));
        }
        return Ok(CallbackTarget { kind: CallbackKind::Url, payload: rest.to_string() });
    }
    if let Some(rest) = raw.strip_prefix("file:") {
        if !rest.starts_with('/') {
            return Err(CallbackParseError::FileNotAbsolute(rest.to_string()));
        }
        return Ok(CallbackTarget { kind: CallbackKind::File, payload: rest.to_string() });
    }

    let kind = if raw.starts_with("http://") || raw.starts_with("https://") {
        CallbackKind::Url
    } else if raw.starts_with('/') {
        CallbackKind::File
    } else {
        CallbackKind::Hook
    };
    Ok(CallbackTarget { kind, payload: raw.to_string() })
}

/// Which event fired a callback. Serialized to the [`CallbackPayload::event`]
/// field as `"click" | "action:<label>" | "dismiss" | "timeout"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackEvent {
    Click,
    Action(String),
    Dismiss,
    Timeout,
}

impl CallbackEvent {
    /// Wire-format representation embedded in the JSON payload.
    #[must_use]
    pub fn to_wire(&self) -> String {
        match self {
            Self::Click => "click".to_string(),
            Self::Action(label) => format!("action:{label}"),
            Self::Dismiss => "dismiss".to_string(),
            Self::Timeout => "timeout".to_string(),
        }
    }
}

/// JSON payload delivered to every callback (hook stdin / HTTP body / file
/// line). Field ordering + naming are load-bearing — snapshot-tested here
/// and consumed by the `notif listen` daemon, the notify-queue consumer,
/// and any third-party `hook:` script or `url:` endpoint the user wires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackPayload {
    /// The `UNNotificationRequest.identifier` (macOS) / equivalent on
    /// other backends. Matches the `--id` value the caller passed, or
    /// the random UUID the backend minted.
    pub notif_id: String,
    /// Semantic event tag (see [`CallbackEvent::to_wire`]).
    pub event: String,
    /// The sender key that dispatched the original notification.
    pub sender: String,
    /// Original notification title.
    pub title: String,
    /// Original notification body.
    pub body: String,
    /// ISO-8601 timestamp of dispatch (the moment `notif send` accepted
    /// the request, not when the user clicked).
    pub ts: String,
}

/// The set of callbacks registered for a single `notif send` invocation.
///
/// Callbacks are dispatch-time config — they live outside [`Notification`]
/// (which is the *payload*) and travel through the outer→inner CLI hop
/// alongside the notification data, then land in the `notif listen` daemon
/// that owns the UN center delegate.
///
/// Ordering: [`on_actions`] preserves the CLI order of `--on-action` flags,
/// since macOS renders custom actions in registration order and users
/// expect the first flag to become the primary action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallbackConfig {
    pub on_click: Option<CallbackTarget>,
    pub on_actions: Vec<(String, CallbackTarget)>,
    pub on_dismiss: Option<CallbackTarget>,
    pub on_timeout: Option<CallbackTarget>,
}

impl CallbackConfig {
    /// True iff no callback slot is set. Fast path check for backends
    /// that skip the delegate wiring when no callback is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.on_click.is_none()
            && self.on_actions.is_empty()
            && self.on_dismiss.is_none()
            && self.on_timeout.is_none()
    }

    /// Total number of registered callback bindings. Used by the CLI's
    /// stub log line and by future capacity checks (some platforms cap
    /// the total registered categories per app).
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.on_click.is_some())
            + self.on_actions.len()
            + usize::from(self.on_dismiss.is_some())
            + usize::from(self.on_timeout.is_some())
    }
}

/// Dispatch a callback. Currently a no-op stub — the real hook / url /
/// file dispatchers live in the `notif listen` daemon and consume the
/// same signature. The stub returns `Ok(())` so consumers (delegate code
/// paths in the backends) can already call it in situ.
///
/// # Errors
///
/// Real implementation bubbles any I/O / HTTP / subprocess failure from
/// the selected dispatcher. The stub form always returns `Ok(())`.
pub fn fire(target: &CallbackTarget, payload: &CallbackPayload) -> std::io::Result<()> {
    // Deliberately silent — the daemon-side dispatcher takes over this
    // signature without touching callers.
    let _ = (target, payload);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_target -----------------------------------------------------

    #[test]
    fn parse_explicit_hook() {
        let t = parse_target("hook:/bin/echo hi").unwrap();
        assert_eq!(t.kind, CallbackKind::Hook);
        assert_eq!(t.payload, "/bin/echo hi");
    }

    #[test]
    fn parse_explicit_url_https() {
        let t = parse_target("url:https://example.com/cb").unwrap();
        assert_eq!(t.kind, CallbackKind::Url);
        assert_eq!(t.payload, "https://example.com/cb");
    }

    #[test]
    fn parse_explicit_url_http() {
        let t = parse_target("url:http://localhost:8080/cb").unwrap();
        assert_eq!(t.kind, CallbackKind::Url);
    }

    #[test]
    fn parse_explicit_file() {
        let t = parse_target("file:/tmp/notif.jsonl").unwrap();
        assert_eq!(t.kind, CallbackKind::File);
        assert_eq!(t.payload, "/tmp/notif.jsonl");
    }

    #[test]
    fn auto_detect_file_from_absolute_path() {
        let t = parse_target("/tmp/notif.jsonl").unwrap();
        assert_eq!(t.kind, CallbackKind::File);
        assert_eq!(t.payload, "/tmp/notif.jsonl");
    }

    #[test]
    fn auto_detect_url_from_https_scheme() {
        let t = parse_target("https://example.com/cb").unwrap();
        assert_eq!(t.kind, CallbackKind::Url);
    }

    #[test]
    fn auto_detect_url_from_http_scheme() {
        let t = parse_target("http://example.com/cb").unwrap();
        assert_eq!(t.kind, CallbackKind::Url);
    }

    #[test]
    fn auto_detect_hook_fallback() {
        let t = parse_target("myscript arg1 arg2").unwrap();
        assert_eq!(t.kind, CallbackKind::Hook);
        assert_eq!(t.payload, "myscript arg1 arg2");
    }

    #[test]
    fn parse_empty_rejected() {
        assert_eq!(parse_target(""), Err(CallbackParseError::Empty));
        assert_eq!(parse_target("   "), Err(CallbackParseError::Empty));
    }

    #[test]
    fn parse_hook_prefix_alone_rejected() {
        assert_eq!(parse_target("hook:"), Err(CallbackParseError::HookEmpty));
        assert_eq!(parse_target("hook:   "), Err(CallbackParseError::HookEmpty));
    }

    #[test]
    fn parse_url_bad_scheme_rejected() {
        assert!(matches!(
            parse_target("url:ftp://x"),
            Err(CallbackParseError::UrlBadScheme(_)),
        ));
    }

    #[test]
    fn parse_file_relative_rejected() {
        assert!(matches!(
            parse_target("file:relative/path"),
            Err(CallbackParseError::FileNotAbsolute(_)),
        ));
    }

    #[test]
    fn trims_whitespace() {
        let t = parse_target("  hook:/bin/echo  ").unwrap();
        assert_eq!(t.kind, CallbackKind::Hook);
        assert_eq!(t.payload, "/bin/echo");
    }

    // ---- Round-trip via to_wire ------------------------------------------

    #[test]
    fn wire_roundtrip_all_kinds() {
        for raw in [
            "hook:/bin/echo hi",
            "url:https://example.com/cb",
            "file:/tmp/x.jsonl",
        ] {
            let t = parse_target(raw).unwrap();
            let wire = t.to_wire();
            let round = parse_target(&wire).unwrap();
            assert_eq!(t, round, "roundtrip failed for {raw:?}");
        }
    }

    #[test]
    fn wire_roundtrip_canonicalizes_auto_detect() {
        // Auto-detected `/tmp/x.jsonl` canonicalizes to `file:/tmp/x.jsonl`
        // on the way out, so the inner reparses to the same File target.
        let t = parse_target("/tmp/x.jsonl").unwrap();
        assert_eq!(t.to_wire(), "file:/tmp/x.jsonl");
        let round = parse_target(&t.to_wire()).unwrap();
        assert_eq!(t, round);
    }

    // ---- CallbackEvent wire format ---------------------------------------

    #[test]
    fn event_wire_format() {
        assert_eq!(CallbackEvent::Click.to_wire(), "click");
        assert_eq!(CallbackEvent::Dismiss.to_wire(), "dismiss");
        assert_eq!(CallbackEvent::Timeout.to_wire(), "timeout");
        assert_eq!(CallbackEvent::Action("reply".into()).to_wire(), "action:reply");
    }

    // ---- CallbackPayload JSON snapshot -----------------------------------

    #[test]
    fn payload_json_snapshot() {
        // Locks the exact JSON shape the daemon dispatcher (and any
        // third-party `hook:` script) will consume. serde_json emits
        // struct fields in declaration order.
        let p = CallbackPayload {
            notif_id: "abc-123".into(),
            event: CallbackEvent::Action("reply".into()).to_wire(),
            sender: "vscode".into(),
            title: "Deploy done".into(),
            body: "staging → prod".into(),
            ts: "2026-07-05T12:00:00Z".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(
            s,
            "{\"notif_id\":\"abc-123\",\"event\":\"action:reply\",\"sender\":\"vscode\",\
             \"title\":\"Deploy done\",\"body\":\"staging → prod\",\"ts\":\"2026-07-05T12:00:00Z\"}",
        );
        let round: CallbackPayload = serde_json::from_str(&s).unwrap();
        assert_eq!(round, p);
    }

    // ---- Stub fire ------------------------------------------------------

    // ---- CallbackConfig -------------------------------------------------

    #[test]
    fn callback_config_default_is_empty() {
        let c = CallbackConfig::default();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn callback_config_counts_slots() {
        let target = || CallbackTarget { kind: CallbackKind::File, payload: "/tmp/x".into() };
        let c = CallbackConfig {
            on_click: Some(target()),
            on_actions: vec![("reply".into(), target()), ("ignore".into(), target())],
            on_dismiss: Some(target()),
            on_timeout: None,
        };
        assert!(!c.is_empty());
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn callback_config_actions_preserve_order() {
        let target = || CallbackTarget { kind: CallbackKind::Hook, payload: "x".into() };
        let c = CallbackConfig {
            on_actions: vec![("a".into(), target()), ("b".into(), target()), ("c".into(), target())],
            ..CallbackConfig::default()
        };
        let labels: Vec<&str> = c.on_actions.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, vec!["a", "b", "c"]);
    }

    // ---- Stub fire ------------------------------------------------------

    #[test]
    fn stub_fire_returns_ok() {
        let target = CallbackTarget { kind: CallbackKind::File, payload: "/tmp/x".into() };
        let payload = CallbackPayload {
            notif_id: "id".into(),
            event: "click".into(),
            sender: "default".into(),
            title: "t".into(),
            body: "b".into(),
            ts: "2026-07-05T12:00:00Z".into(),
        };
        assert!(fire(&target, &payload).is_ok());
    }
}

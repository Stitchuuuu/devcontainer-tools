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
//! [`fire`] routes on `target.kind` to the matching dispatcher:
//! [`dispatch_hook`] spawns a subprocess (argv split via `shell-words`, JSON
//! on stdin, fire-and-forget), [`dispatch_url`] POSTs the JSON body over
//! HTTP via `ureq 3`, [`dispatch_file`] appends one `\n`-terminated JSONL
//! line. All three swallow their own failures (logged via [`crate::warn::emit`])
//! — a single failing callback must not cascade into the delegate's next
//! response handling.
//!
//! [`Url`]: CallbackKind::Url
//! [`File`]: CallbackKind::File
//! [`Hook`]: CallbackKind::Hook

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::warn;

/// Which dispatch mechanism a [`CallbackTarget`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// HTTP request timeouts for [`dispatch_url`]. Kept conservative — a
/// callback endpoint that hangs must not stall the daemon's response
/// dispatch for other pending notifications.
const URL_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const URL_GLOBAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Dispatch a callback. Routes on [`target.kind`] to the matching
/// dispatcher: subprocess exec (`hook:`), HTTP POST (`url:`), or JSONL
/// append (`file:`).
///
/// # Failure model
/// All three dispatchers **swallow their own failures** — a broken hook
/// script, a 5xx URL endpoint, or a full disk gets a single `warning:`
/// line via [`crate::warn::emit`] and this function returns `Ok(())`.
/// Rationale: the delegate closure that calls `fire` has no useful
/// recovery path, and one failing callback must not block the next.
///
/// # Errors
/// Returns `Err` only for encoder-level failures that indicate a bug
/// (JSON serialization of a well-formed [`CallbackPayload`] failing —
/// should never happen given the struct's `Serialize` derive).
pub fn fire(target: &CallbackTarget, payload: &CallbackPayload) -> std::io::Result<()> {
    let json = serde_json::to_string(payload).map_err(std::io::Error::other)?;
    match target.kind {
        CallbackKind::Hook => dispatch_hook(&target.payload, &json),
        CallbackKind::Url => dispatch_url(&target.payload, &json),
        CallbackKind::File => dispatch_file(&target.payload, &json),
    }
    Ok(())
}

/// Spawn a subprocess and stream the payload JSON to its stdin.
///
/// argv is split via `shell_words::split` (POSIX shell tokenization). On
/// tokenizer failure — mismatched quotes, unterminated backslash — we fall
/// back to whitespace split with a `warning:` note. Fire-and-forget: we
/// do NOT `wait()` on the child. The write handle to stdin is dropped
/// before the function returns, which closes the child's stdin.
fn dispatch_hook(argv_line: &str, json: &str) {
    let argv = match shell_words::split(argv_line) {
        Ok(v) => v,
        Err(_) => {
            warn::emit(
                "callback_hook_shell_words_fallback",
                &format!("hook argv {argv_line:?} failed shell-words tokenizer; falling back to whitespace split"),
            );
            argv_line.split_whitespace().map(str::to_string).collect()
        }
    };
    if argv.is_empty() {
        warn::emit("callback_hook_empty_argv", "hook target expanded to zero argv tokens; nothing to exec");
        return;
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            warn::emit(
                "callback_hook_spawn_failed",
                &format!("hook spawn {:?} failed: {e}", argv.join(" ")),
            );
            return;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(json.as_bytes()) {
            warn::emit(
                "callback_hook_stdin_write_failed",
                &format!("hook stdin write failed for {:?}: {e}", argv[0]),
            );
        }
        // stdin dropped here → EOF for the child.
    }
    // No wait() — fire-and-forget. Child is reaped when the parent exits.
}

/// HTTP POST the payload JSON to `url`.
///
/// `ureq`'s default `http_status_as_error = true` maps non-2xx statuses
/// to `Err`, so a single check covers both transport failures and
/// application-level rejects. All errors surface once via `warn::emit`
/// under distinct dedup categories so hot loops can't flood stderr.
fn dispatch_url(url: &str, json: &str) {
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(URL_CONNECT_TIMEOUT))
        .timeout_global(Some(URL_GLOBAL_TIMEOUT))
        .build()
        .new_agent();
    let res = agent
        .post(url)
        .content_type("application/json")
        .send(json);
    if let Err(e) = res {
        warn::emit(
            "callback_url_failed",
            &format!("POST {url}: {e}"),
        );
    }
}

/// Append the payload JSON followed by `\n` to `path`, creating the file
/// if missing. Uses `OpenOptions::append` for POSIX per-line atomicity
/// (guaranteed for writes < `PIPE_BUF` = 4096 bytes; our JSON payloads are
/// well under 500 bytes).
fn dispatch_file(path: &str, json: &str) {
    let mut file = match OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            warn::emit(
                "callback_file_open_failed",
                &format!("open {path}: {e}"),
            );
            return;
        }
    };
    let mut line = String::with_capacity(json.len() + 1);
    line.push_str(json);
    line.push('\n');
    if let Err(e) = file.write_all(line.as_bytes()) {
        warn::emit(
            "callback_file_write_failed",
            &format!("write {path}: {e}"),
        );
    }
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

    // ---- Real dispatchers -----------------------------------------------

    fn sample_payload() -> CallbackPayload {
        CallbackPayload {
            notif_id: "abc-123".into(),
            event: "click".into(),
            sender: "default".into(),
            title: "T".into(),
            body: "B".into(),
            ts: "2026-07-06T09:00:00Z".into(),
        }
    }

    #[test]
    fn dispatch_file_appends_jsonl_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clicks.jsonl");
        let target = CallbackTarget {
            kind: CallbackKind::File,
            payload: path.to_string_lossy().to_string(),
        };
        fire(&target, &sample_payload()).unwrap();
        fire(&target, &sample_payload()).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.split_terminator('\n').collect();
        assert_eq!(lines.len(), 2, "expected 2 JSONL lines, got {contents:?}");
        for line in lines {
            let round: CallbackPayload = serde_json::from_str(line).unwrap();
            assert_eq!(round.notif_id, "abc-123");
        }
    }

    #[test]
    fn dispatch_hook_pipes_json_on_stdin() {
        // Use `sh -c 'cat > "$OUT"'` — writes stdin to the file $OUT points at.
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("hook.out");
        // Quote the path so shell-words handles it as a single token even
        // if the tempdir contains spaces (unlikely, but robust).
        let argv = format!("sh -c cat>{} </dev/stdin", out.display());
        // ^ workaround: we can't easily pass an env var through argv split
        // and preserve fire-and-forget. Simpler: use a wrapper script.
        let _ = argv; // (used only to acknowledge the alternative)
        // Simpler approach: write a tiny script.
        let script = dir.path().join("hook.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\ncat > {}\n", out.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let target = CallbackTarget {
            kind: CallbackKind::Hook,
            payload: script.to_string_lossy().to_string(),
        };
        fire(&target, &sample_payload()).unwrap();

        // Fire-and-forget: poll briefly for the child to flush.
        for _ in 0..50 {
            if out.exists() && std::fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let written = std::fs::read_to_string(&out).unwrap();
        let round: CallbackPayload = serde_json::from_str(&written).unwrap();
        assert_eq!(round.notif_id, "abc-123");
    }

    #[test]
    fn dispatch_hook_bad_argv_falls_back_to_whitespace_split() {
        // Mismatched quote in argv → shell-words errors → fallback to
        // whitespace split. `true` is a builtin that exits 0 regardless
        // of args, so the child spawns without a real crash even though
        // the tokenization changed.
        let target = CallbackTarget {
            kind: CallbackKind::Hook,
            payload: r#"true "unterminated"#.into(),
        };
        // Just assert `fire` returns Ok (no panic on tokenizer error).
        fire(&target, &sample_payload()).unwrap();
    }

    #[test]
    fn dispatch_url_posts_json_body() {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;
        // Bind to 127.0.0.1:0 → OS picks a free port.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/cb");

        // Accept one connection in a bg thread, read request, send 200.
        let received: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let received_bg = received.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = String::new();
            {
                let mut reader = BufReader::new(&stream);
                // Read headers.
                let mut content_length = 0usize;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                // Read body.
                let mut body = vec![0u8; content_length];
                std::io::Read::read_exact(&mut reader, &mut body).unwrap();
                buf.push_str(&String::from_utf8_lossy(&body));
            }
            use std::io::Write as _;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            *received_bg.lock().unwrap() = Some(buf);
        });

        let target = CallbackTarget { kind: CallbackKind::Url, payload: url };
        fire(&target, &sample_payload()).unwrap();

        server.join().unwrap();
        let body = received.lock().unwrap().clone().expect("server captured body");
        let round: CallbackPayload = serde_json::from_str(&body).unwrap();
        assert_eq!(round.notif_id, "abc-123");
    }

    #[test]
    fn dispatch_url_non_2xx_does_not_propagate() {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/cb");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&stream);
            let mut sink = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    let n: usize = v.trim().parse().unwrap_or(0);
                    let mut body = vec![0u8; n];
                    std::io::Read::read_exact(&mut reader, &mut body).unwrap();
                    sink.push_str(&String::from_utf8_lossy(&body));
                }
            }
            let _ = sink;
            use std::io::Write as _;
            stream
                .write_all(b"HTTP/1.1 500 Server Error\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        let target = CallbackTarget { kind: CallbackKind::Url, payload: url };
        // Must return Ok even though the server responded 500.
        fire(&target, &sample_payload()).unwrap();
        server.join().unwrap();
    }
}

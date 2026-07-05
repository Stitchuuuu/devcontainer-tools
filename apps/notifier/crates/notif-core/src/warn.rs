//! Portable stderr helpers — `warning:` (degradation), `info:` (portable
//! flag shadowed by a native override), plain [`stderr`] (progress lines).
//! All three route through the same [`WarnConfig`] : shared `--quiet` gate,
//! shared per-category dedup namespace ([`emit`] / [`info`] only), shared
//! optional append to `NOTIF_LOG` audit file.
//!
//! Backends and the CLI call [`emit`] whenever they need to surface a
//! degradation the user should know about but that shouldn't fail the run
//! (e.g. macOS silently dropping `--on-timeout` in v0.1). Repeated calls with
//! the same `category` within a single process are suppressed after the
//! first, so a hot code path can call [`emit`] without spamming stderr.
//!
//! [`info`] uses the same plumbing but writes an `info:` prefix. It is the
//! right helper for the "portable vs native flag conflict — native wins with
//! `info:` log line (not warning)" contract. Specificity beats generality,
//! so a portable flag being shadowed by a `--macos-*` override is
//! information, not a degradation.
//!
//! [`stderr`] is the log-aware sibling of `eprintln!` — no prefix, no dedup,
//! but still respects `--quiet` and still appends to the audit file. CLI
//! progress lines (`sending notification via 'default'…`, `sent.`) route
//! through it so the audit file gets the full narrative.
//!
//! When [`WarnConfig::log_file`] is set (from the `NOTIF_LOG` env var), every
//! written line is ALSO appended to that path as
//! `<iso-8601-utc> <prefix>: <msg>` (or `<iso-8601-utc> <msg>` for [`stderr`]).
//! `O_APPEND` guarantees atomic per-line writes across concurrent producers
//! (notify-queue daemon + `notif listen` daemon + shell invocations can
//! share a single log file without interleaving).
//!
//! Configuration is set once at process start via [`init`], typically in the
//! CLI's `main()` after parsing `--quiet`, reading `NOTIF_QUIET`, and
//! reading `NOTIF_LOG`. If [`init`] is never called, all three helpers fall
//! back to the default ([`WarnConfig::default`] — `quiet=false`, no audit
//! file).

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Runtime config for [`emit`] / [`info`] / [`stderr`]. Built once from CLI +
/// env in `main()`.
#[derive(Debug, Clone, Default)]
pub struct WarnConfig {
    /// When `true`, no line is written to stderr. Audit-file appends
    /// **still happen** — the daemon usecase wants a persistent record
    /// even when the interactive stream is silenced.
    pub quiet: bool,
    /// Absolute path to an append-only audit log. Every line the helpers
    /// emit is duplicated there with an ISO-8601 UTC timestamp prefix.
    /// `None` = disabled (the common CLI case).
    pub log_file: Option<PathBuf>,
}

static CFG: OnceLock<WarnConfig> = OnceLock::new();
static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Set the process-wide warning config. Idempotent — subsequent calls after
/// the first are silently ignored (matches `OnceLock::set` semantics). Call
/// from `main()` before dispatching subcommands.
pub fn init(cfg: WarnConfig) {
    let _ = CFG.set(cfg);
}

/// Emit `warning: <msg>` on stderr, gated by [`WarnConfig::quiet`] and
/// per-category dedup. First call for a given `category` writes; subsequent
/// calls with the same `category` within the same process are suppressed.
///
/// `category` is not printed — it is only the dedup key. Choose short
/// snake_case identifiers (`on_timeout_macos`, `image_attachment_refused`).
///
/// Callers format the human-readable message themselves:
/// ```ignore
/// notif_core::warn::emit(
///     "on_timeout_macos",
///     "--on-timeout ignored on macOS in v0.1",
/// );
/// ```
pub fn emit(category: &str, msg: &str) {
    let cfg = CFG.get().cloned().unwrap_or_default();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    emit_into(&cfg, seen, "warning", category, msg);
}

/// Write `<msg>` on stderr (no prefix), gated by [`WarnConfig::quiet`].
/// No category, no dedup — for one-off progress lines like
/// `sending notification via 'default'…` that would repeat legitimately
/// across invocations.
///
/// When [`WarnConfig::log_file`] is set, the line is ALSO appended to
/// the audit file as `<ISO-8601-UTC> <msg>` — same guarantees as
/// [`emit`] / [`info`] w.r.t. atomic per-line appends.
pub fn stderr(msg: &str) {
    let cfg = CFG.get().cloned().unwrap_or_default();
    stderr_into(&cfg, msg);
}

/// Emit `info: <msg>` on stderr, gated by [`WarnConfig::quiet`] and
/// per-category dedup. Same plumbing as [`emit`], different prefix.
///
/// Use this when the user is being informed that a portable flag has been
/// *shadowed* by a more specific native override (e.g. `--macos-sound-name`
/// overriding `--sound`). The event isn't a degradation — the notification
/// still delivers with the exact spec the user asked for — so `warning:`
/// would be misleading. Keep [`emit`] for degradations and [`info`] for
/// override notices.
///
/// Category dedup namespace is *shared* with [`emit`]: firing
/// `info(cat, …)` then `emit(cat, …)` writes once and suppresses the
/// second, regardless of prefix. Choose distinct categories per site.
pub fn info(category: &str, msg: &str) {
    let cfg = CFG.get().cloned().unwrap_or_default();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    emit_into(&cfg, seen, "info", category, msg);
}

/// Pure form of [`emit`] / [`info`] with explicit config + dedup state.
/// Kept crate-visible so unit tests can exercise the logic without
/// touching the process-static [`CFG`] / [`SEEN`] — those can only be
/// initialized once per test binary and would leak state across tests.
///
/// `prefix` is the label that lands before the colon on stderr
/// (`"warning"` from [`emit`], `"info"` from [`info`]). Returns `true` if
/// the message was written to stderr OR the audit file, `false` if both
/// were suppressed (by `quiet` for stderr, by dedup for both).
pub(crate) fn emit_into(
    cfg: &WarnConfig,
    seen: &Mutex<HashSet<String>>,
    prefix: &str,
    category: &str,
    msg: &str,
) -> bool {
    // Fast-out when there is genuinely nothing to write — preserves the
    // "quiet does not consume the dedup slot" contract for the pure-CLI
    // case, so quiet-then-loud toggles can still emit. When the audit
    // file is set we WILL write there even under quiet, so we must
    // consume the dedup slot.
    if cfg.quiet && cfg.log_file.is_none() {
        return false;
    }
    let mut guard = seen.lock().expect("warn dedup mutex poisoned");
    if !guard.insert(category.to_string()) {
        return false;
    }
    drop(guard);
    let mut wrote = false;
    if !cfg.quiet {
        eprintln!("{prefix}: {msg}");
        wrote = true;
    }
    // Audit log gets the line even when --quiet silenced stderr — the
    // daemon usecase wants persistence irrespective of the interactive
    // stream.
    if let Some(path) = &cfg.log_file {
        if append_line(path, &format!("{prefix}: {msg}")) {
            wrote = true;
        }
    }
    wrote
}

/// Pure form of [`stderr`] with explicit config. Same audit-file
/// duplication semantics but no prefix, no category, no dedup.
pub(crate) fn stderr_into(cfg: &WarnConfig, msg: &str) -> bool {
    let mut wrote = false;
    if !cfg.quiet {
        eprintln!("{msg}");
        wrote = true;
    }
    if let Some(path) = &cfg.log_file {
        if append_line(path, msg) {
            wrote = true;
        }
    }
    wrote
}

/// Append `<ISO-8601-UTC> <line>\n` to `path`. Never panics — a failed
/// audit write is silently swallowed (returns `false`) so a broken log
/// path can't take down the CLI. `O_APPEND` guarantees per-line
/// atomicity across concurrent producers as long as the line is under
/// `PIPE_BUF` (4096 bytes on Linux/macOS) — every notif line is well
/// under.
fn append_line(path: &std::path::Path, line: &str) -> bool {
    let ts = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return false;
    };
    file.write_all(format!("{ts} {line}\n").as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Mutex<HashSet<String>> {
        Mutex::new(HashSet::new())
    }

    // ---- warning: prefix ------------------------------------------------

    #[test]
    fn first_call_writes() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
    }

    #[test]
    fn same_category_suppressed_second_time() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        assert!(!emit_into(&cfg, &seen, "warning", "cat_a", "msg 2 with different text"));
    }

    #[test]
    fn different_category_still_writes() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        assert!(emit_into(&cfg, &seen, "warning", "cat_b", "msg 2"));
    }

    #[test]
    fn quiet_suppresses_and_does_not_record() {
        let cfg = WarnConfig { quiet: true, log_file: None };
        let seen = fresh();
        assert!(!emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        // HashSet must stay empty — suppressed calls do not consume the
        // dedup slot, so a later un-quieted run can still emit.
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn quiet_then_loud_writes() {
        let cfg_quiet = WarnConfig { quiet: true, log_file: None };
        let cfg_loud = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(!emit_into(&cfg_quiet, &seen, "warning", "cat_a", "msg 1"));
        assert!(emit_into(&cfg_loud, &seen, "warning", "cat_a", "msg 2"));
    }

    // ---- info: prefix ---------------------------------------------------

    #[test]
    fn info_first_call_writes() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
    }

    #[test]
    fn info_same_category_suppressed_second_time() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(!emit_into(&cfg, &seen, "info", "cat_a", "msg 2"));
    }

    #[test]
    fn info_different_category_still_writes() {
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(emit_into(&cfg, &seen, "info", "cat_b", "msg 2"));
    }

    #[test]
    fn info_quiet_suppresses() {
        let cfg = WarnConfig { quiet: true, log_file: None };
        let seen = fresh();
        assert!(!emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn info_and_warn_share_dedup_namespace() {
        // Documented invariant — a warn::info("foo", …) followed by
        // warn::emit("foo", …) writes once. Callers must choose distinct
        // categories per site regardless of prefix.
        let cfg = WarnConfig { quiet: false, log_file: None };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "shared_cat", "info line"));
        assert!(!emit_into(&cfg, &seen, "warning", "shared_cat", "warn line"));
    }

    // ---- NOTIF_LOG audit file -------------------------------------------

    fn tmp_log_path(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "notif-core-warn-test-{tag}-{}.log",
            std::process::id(),
        ));
        // Fresh file per test.
        let _ = std::fs::remove_file(&p);
        p
    }

    fn read_all(p: &std::path::Path) -> String {
        std::fs::read_to_string(p).unwrap_or_default()
    }

    #[test]
    fn logfile_captures_warning_line() {
        let path = tmp_log_path("warn-line");
        let cfg = WarnConfig { quiet: false, log_file: Some(path.clone()) };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "banner refused"));
        let contents = read_all(&path);
        assert!(contents.contains("warning: banner refused"), "got: {contents:?}");
        // Timestamp shape: RFC 3339 UTC, ends with "Z".
        assert!(contents.contains("Z warning: "), "expected ISO 8601 UTC prefix, got: {contents:?}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn logfile_captures_info_line() {
        let path = tmp_log_path("info-line");
        let cfg = WarnConfig { quiet: false, log_file: Some(path.clone()) };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "override applied"));
        let contents = read_all(&path);
        assert!(contents.contains("info: override applied"), "got: {contents:?}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn logfile_captures_stderr_progress() {
        let path = tmp_log_path("stderr-line");
        let cfg = WarnConfig { quiet: false, log_file: Some(path.clone()) };
        assert!(stderr_into(&cfg, "sending notification via 'default'…"));
        let contents = read_all(&path);
        assert!(contents.contains("sending notification via 'default'…"), "got: {contents:?}");
        // stderr helper writes no prefix — line starts with the timestamp
        // then a space then the message.
        assert!(
            !contents.contains(": sending notification"),
            "stderr() should not add a prefix, got: {contents:?}",
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn logfile_captures_under_quiet_mode() {
        // Daemon use case : quiet on the interactive stream, persistent
        // audit trail via NOTIF_LOG. Both fire together.
        let path = tmp_log_path("quiet-log");
        let cfg = WarnConfig { quiet: true, log_file: Some(path.clone()) };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "still audited"));
        assert!(stderr_into(&cfg, "progress under quiet"));
        let contents = read_all(&path);
        assert!(contents.contains("warning: still audited"), "got: {contents:?}");
        assert!(contents.contains("progress under quiet"), "got: {contents:?}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_logfile_dir_is_swallowed_not_fatal() {
        // A broken NOTIF_LOG path (parent missing / no perms) must NOT
        // crash the CLI — the audit is best-effort. The stderr side
        // still fires.
        let path = std::path::PathBuf::from("/nonexistent-parent/notif.log");
        let cfg = WarnConfig { quiet: false, log_file: Some(path) };
        let seen = fresh();
        // Returns true because stderr fired — the audit failure is
        // silently absorbed.
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "stderr wins"));
    }

    #[test]
    fn logfile_consumes_dedup_slot_under_quiet() {
        // When quiet + log_file, the dedup slot IS consumed because we
        // wrote to the audit file. Prevents "same warning 1000×" spam
        // in the audit file.
        let path = tmp_log_path("dedup-quiet-log");
        let cfg = WarnConfig { quiet: true, log_file: Some(path.clone()) };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "first"));
        // Same category, same call → suppressed even though we're quiet
        // (dedup slot consumed).
        assert!(!emit_into(&cfg, &seen, "warning", "cat_a", "second"));
        let contents = read_all(&path);
        let count = contents.matches("cat_a").count() + contents.matches("first").count();
        assert_eq!(count, 1, "expected exactly one 'first' line, got: {contents:?}");
        std::fs::remove_file(&path).ok();
    }
}

//! Portable stderr helpers — `warning:` (degradation) and `info:` (portable
//! flag shadowed by a native override). Both are process-wide dedup'd by
//! category and share the same `--quiet` gate.
//!
//! Backends and the CLI call [`emit`] whenever they need to surface a
//! degradation the user should know about but that shouldn't fail the run
//! (e.g. macOS silently dropping `--on-timeout` in v0.1). Repeated calls with
//! the same `category` within a single process are suppressed after the
//! first, so a hot code path can call [`emit`] without spamming stderr.
//!
//! [`info`] uses the same plumbing but writes an `info:` prefix. It is the
//! right helper for the ROLLOUT decision "portable vs native flag conflict —
//! native wins with `info:` log line (not warning)". Specificity beats
//! generality, so a portable flag being shadowed by a `--macos-*` override is
//! information, not a degradation.
//!
//! Configuration is set once at process start via [`init`], typically in the
//! CLI's `main()` after parsing `--quiet` and reading `NOTIF_QUIET`. If
//! [`init`] is never called, both [`emit`] and [`info`] fall back to the
//! default ([`WarnConfig::default`], `quiet=false`).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Runtime config for [`emit`]. Built once from CLI + env in `main()`.
#[derive(Debug, Clone, Default)]
pub struct WarnConfig {
    /// When `true`, [`emit`] returns without writing anything.
    pub quiet: bool,
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
/// the message was written to stderr, `false` if it was suppressed (by
/// `quiet` or by dedup).
pub(crate) fn emit_into(
    cfg: &WarnConfig,
    seen: &Mutex<HashSet<String>>,
    prefix: &str,
    category: &str,
    msg: &str,
) -> bool {
    if cfg.quiet {
        return false;
    }
    let mut guard = seen.lock().expect("warn dedup mutex poisoned");
    if !guard.insert(category.to_string()) {
        return false;
    }
    drop(guard);
    eprintln!("{prefix}: {msg}");
    true
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
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
    }

    #[test]
    fn same_category_suppressed_second_time() {
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        assert!(!emit_into(&cfg, &seen, "warning", "cat_a", "msg 2 with different text"));
    }

    #[test]
    fn different_category_still_writes() {
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        assert!(emit_into(&cfg, &seen, "warning", "cat_b", "msg 2"));
    }

    #[test]
    fn quiet_suppresses_and_does_not_record() {
        let cfg = WarnConfig { quiet: true };
        let seen = fresh();
        assert!(!emit_into(&cfg, &seen, "warning", "cat_a", "msg 1"));
        // HashSet must stay empty — suppressed calls do not consume the
        // dedup slot, so a later un-quieted run can still emit.
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn quiet_then_loud_writes() {
        let cfg_quiet = WarnConfig { quiet: true };
        let cfg_loud = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(!emit_into(&cfg_quiet, &seen, "warning", "cat_a", "msg 1"));
        assert!(emit_into(&cfg_loud, &seen, "warning", "cat_a", "msg 2"));
    }

    // ---- info: prefix ---------------------------------------------------

    #[test]
    fn info_first_call_writes() {
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
    }

    #[test]
    fn info_same_category_suppressed_second_time() {
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(!emit_into(&cfg, &seen, "info", "cat_a", "msg 2"));
    }

    #[test]
    fn info_different_category_still_writes() {
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(emit_into(&cfg, &seen, "info", "cat_b", "msg 2"));
    }

    #[test]
    fn info_quiet_suppresses() {
        let cfg = WarnConfig { quiet: true };
        let seen = fresh();
        assert!(!emit_into(&cfg, &seen, "info", "cat_a", "msg 1"));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn info_and_warn_share_dedup_namespace() {
        // Documented invariant — a warn::info("foo", …) followed by
        // warn::emit("foo", …) writes once. Callers must choose distinct
        // categories per site regardless of prefix.
        let cfg = WarnConfig { quiet: false };
        let seen = fresh();
        assert!(emit_into(&cfg, &seen, "info", "shared_cat", "info line"));
        assert!(!emit_into(&cfg, &seen, "warning", "shared_cat", "warn line"));
    }
}

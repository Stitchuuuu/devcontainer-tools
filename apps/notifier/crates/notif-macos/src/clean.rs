//! Unregister-and-remove for materialized sender bundles.
//!
//! Sends over the lifetime of the CLI accumulate `.app` bundles under
//! `~/.local/share/notif/senders/` — each with a distinct
//! `CFBundleIdentifier` that macOS's TCC layer tracks. Deleting the bundle
//! folder alone leaves stale TCC entries (visible in System Settings →
//! Notifications) and can pollute LaunchServices' local DB. `clean_sender`
//! and `clean_all` do the two steps together : `tccutil reset Notifications
//! <id>` first, then `fs::remove_dir_all` the bundle.
//!
//! TCC reset is best-effort — `tccutil` fails when there's no entry for the
//! id, which we treat as a success (nothing to reset). The bundle removal
//! failure is fatal — the caller sees the underlying [`MacosError::Io`].

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::error::MacosError;
use crate::sender::{find_bundle_by_key, list_senders, senders_root};

/// Outcome of a `tccutil` call.
#[derive(Debug, Clone)]
pub enum TccStatus {
    /// `tccutil reset Notifications <id>` returned 0.
    Ok,
    /// `tccutil` failed with a non-zero exit — usually "no matching entry",
    /// treated as best-effort. The message is stderr for logging.
    Failed(String),
    /// The caller elected not to touch TCC (e.g. absent identifier).
    Skipped(String),
}

impl std::fmt::Display for TccStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => f.write_str("tcc: reset"),
            Self::Failed(msg) => write!(f, "tcc: skipped ({msg})"),
            Self::Skipped(reason) => write!(f, "tcc: skipped ({reason})"),
        }
    }
}

/// Row of the `notif clean` output.
#[derive(Debug, Clone)]
pub struct CleanReport {
    /// Sender key from the bundle's `NotifSenderKey` marker.
    pub key: String,
    /// `CFBundleIdentifier` we reset via `tccutil`.
    pub identifier: String,
    /// Path we removed (whether the removal actually happened is in
    /// [`Self::bundle_removed`]).
    pub bundle_path: PathBuf,
    /// True iff we succeeded at `fs::remove_dir_all(bundle_path)`.
    pub bundle_removed: bool,
    /// Outcome of the `tccutil reset Notifications <identifier>` call.
    pub tcc_reset: TccStatus,
}

/// Reset TCC + remove the bundle for a single sender key.
///
/// # Errors
/// - [`MacosError::NoHome`] if `$HOME` is unset.
/// - [`MacosError::Objc`] if the key does not resolve to a materialized
///   bundle (nothing to clean).
/// - [`MacosError::Io`] if `fs::remove_dir_all` fails.
pub fn clean_sender(key: &str) -> Result<CleanReport, MacosError> {
    let bundle = find_bundle_by_key(key)?
        .ok_or_else(|| MacosError::Objc(format!("no bundle registered for sender {key:?}")))?;
    let identifier = read_identifier(&bundle)?;
    let tcc_reset = tccutil_reset(&identifier);
    let bundle_removed = match fs::remove_dir_all(&bundle) {
        Ok(()) => true,
        Err(e) => return Err(MacosError::Io(e)),
    };
    Ok(CleanReport {
        key: key.to_string(),
        identifier,
        bundle_path: bundle,
        bundle_removed,
        tcc_reset,
    })
}

/// Reset TCC + remove every materialized bundle.
///
/// Prompts on stdin for `[y/N]` when `assume_yes` is false; skipped when the
/// senders dir is empty (no confirmation needed for a no-op).
///
/// Removes the empty `senders/` directory as a final cleanup step.
///
/// # Errors
/// - [`MacosError::NoHome`] if `$HOME` is unset.
/// - [`MacosError::Io`] on `remove_dir_all` failure for any bundle (partial
///   reports are surfaced in the returned vector before the error).
pub fn clean_all(assume_yes: bool) -> Result<Vec<CleanReport>, MacosError> {
    let rows = list_senders()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    if !assume_yes && !confirm_all(rows.len())? {
        return Ok(Vec::new());
    }

    let mut reports = Vec::with_capacity(rows.len());
    for summary in rows {
        let bundle = senders_root()?.join(&summary.folder);
        let tcc_reset = tccutil_reset(&summary.identifier);
        let bundle_removed = match fs::remove_dir_all(&bundle) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => false,
            Err(e) => return Err(MacosError::Io(e)),
        };
        reports.push(CleanReport {
            key: summary.key,
            identifier: summary.identifier,
            bundle_path: bundle,
            bundle_removed,
            tcc_reset,
        });
    }

    // Best-effort tidy — remove the senders/ dir if it's now empty.
    if let Ok(mut it) = fs::read_dir(senders_root()?) {
        if it.next().is_none() {
            let _ = fs::remove_dir(senders_root()?);
        }
    }

    Ok(reports)
}

fn confirm_all(count: usize) -> Result<bool, MacosError> {
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    write!(lock, "About to remove {count} sender bundle(s) and reset their notification permissions. Continue? [y/N] ")?;
    lock.flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    let trimmed = answer.trim().to_lowercase();
    Ok(matches!(trimmed.as_str(), "y" | "yes"))
}

fn read_identifier(bundle: &std::path::Path) -> Result<String, MacosError> {
    let plist_path = bundle.join("Contents/Info.plist");
    let dict: plist::Dictionary = plist::from_file(&plist_path)?;
    dict.get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .ok_or_else(|| MacosError::Objc(format!("{bundle:?} missing CFBundleIdentifier")))
}

fn tccutil_reset(identifier: &str) -> TccStatus {
    if identifier.is_empty() || identifier == "?" {
        return TccStatus::Skipped("no identifier".to_string());
    }
    let out = match Command::new("tccutil")
        .args(["reset", "Notifications", identifier])
        .output()
    {
        Ok(o) => o,
        Err(e) => return TccStatus::Failed(format!("spawn failed: {e}")),
    };
    if out.status.success() {
        TccStatus::Ok
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        TccStatus::Failed(if msg.is_empty() {
            format!("exit {}", out.status.code().unwrap_or(-1))
        } else {
            msg
        })
    }
}

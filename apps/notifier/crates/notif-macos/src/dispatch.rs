//! Two-mode dispatch pipeline.
//!
//! **Outer mode** — `notif` runs from `$PATH` or the user's build tree. Job:
//! materialize the sender's `.app`, spawn the bundled `notif` directly by
//! absolute path (bypassing `open(1)` / LaunchServices to avoid LSDB
//! churn), propagate exit code.
//!
//! **Inner mode** — `notif` runs from `<bundle>/Contents/MacOS/notif`. Job:
//! call `UNUserNotificationCenter`.
//!
//! Rationale: `UNUserNotificationCenter.current()` reads
//! `[NSBundle mainBundle]` of the *calling process*. Only a binary that is
//! literally inside a `.app` gets the bundle identity — direct exec into
//! `<bundle>/Contents/MacOS/notif` gives the child the right mainBundle
//! without any LSDB round-trip. See the ROLLOUT decisions record for the
//! full derivation.

use std::path::{Path, PathBuf};
use std::process::Command;

use notif_core::Notification;

use crate::bundle::{ad_hoc_codesign, ensure_bundle};
use crate::error::MacosError;

/// True iff `current_exe()` lives at `.../<x>.app/Contents/MacOS/notif`.
///
/// Detection walks the path — no environment variables, no CLI flags. Robust
/// to symlinks (uses `canonicalize` best-effort but falls back to the raw
/// path).
pub fn is_inner_mode() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p.canonicalize().unwrap_or(p),
        Err(_) => return false,
    };
    let mut it = exe.ancestors();
    // <exe>
    it.next();
    // MacOS
    let macos = match it.next().and_then(|p| p.file_name()) {
        Some(n) => n,
        None => return false,
    };
    if macos != "MacOS" {
        return false;
    }
    // Contents
    let contents = match it.next().and_then(|p| p.file_name()) {
        Some(n) => n,
        None => return false,
    };
    if contents != "Contents" {
        return false;
    }
    // <name>.app
    match it.next().and_then(|p| p.file_name()) {
        Some(n) => n.to_string_lossy().ends_with(".app"),
        None => false,
    }
}

/// Outer-mode entry point. Called by [`crate::backend::MacosBackend::dispatch`].
///
/// Ensures the bundle exists, spawns `open -W -a <.app> --args send ...`, and
/// retries once with an ad-hoc codesign if the inner exit reports
/// [`MacosError::NotSigned`].
///
/// # Errors
/// Bubbles anything from [`ensure_bundle`] plus [`MacosError::OpenFailed`] or
/// [`MacosError::NotSigned`] on second failure.
pub fn dispatch_outer(notif: &Notification) -> Result<(), MacosError> {
    let display = notif.sender.key.clone();
    let bundle = ensure_bundle(&notif.sender.key, &display, None, None)?;

    match invoke_inner_send(&bundle, notif) {
        Ok(()) => Ok(()),
        Err(MacosError::NotSigned) => {
            ad_hoc_codesign(&bundle)?;
            invoke_inner_send(&bundle, notif)
        }
        Err(e) => Err(e),
    }
}

/// Outer-mode entry point for `notif setup`. Materializes the bundle and
/// launches the inner process with the `setup` subcommand so
/// `requestAuthorization` fires under the correct bundle identity.
///
/// # Errors
/// Same as [`dispatch_outer`] minus the send-specific paths.
pub fn setup_outer(sender_key: &str) -> Result<(), MacosError> {
    let bundle = ensure_bundle(sender_key, sender_key, None, None)?;
    invoke_inner_setup(&bundle, sender_key)
}

// Directly spawn the bundled `notif` executable via its absolute path
// (bypassing `open(1)` / LaunchServices). `[NSBundle mainBundle]` resolves
// from `current_exe()` at process start — landing inside
// `Contents/MacOS/notif` is enough to give the child the right identity for
// `UNUserNotificationCenter`, without touching LSDB.
//
// Rationale: `open -W -a <bundle>` registered the bundle with LaunchServices
// on every invocation, causing LSDB churn (visible in the notif-diag under
// `_LSServer_CopyLocalDatabase (seeded? 0)` spam after ~50 sends). Direct
// spawn eliminates the round-trip.
fn inner_exe_path(bundle: &Path) -> PathBuf {
    bundle.join("Contents/MacOS/notif")
}

fn invoke_inner_send(bundle: &Path, notif: &Notification) -> Result<(), MacosError> {
    let mut cmd = Command::new(inner_exe_path(bundle));
    cmd.arg("send")
        .arg("--title").arg(&notif.title)
        .arg("--body").arg(&notif.body)
        .arg("--sender").arg(&notif.sender.key);
    if let Some(sub) = &notif.subtitle {
        cmd.arg("--subtitle").arg(sub);
    }
    run_inner(cmd)
}

fn invoke_inner_setup(bundle: &Path, sender_key: &str) -> Result<(), MacosError> {
    let mut cmd = Command::new(inner_exe_path(bundle));
    cmd.arg("setup").arg("--sender").arg(sender_key);
    run_inner(cmd)
}

fn run_inner(mut cmd: Command) -> Result<(), MacosError> {
    let status = cmd.status()?;
    let code = status.code().unwrap_or(-1);
    match code {
        0 => Ok(()),
        // Inner-mode exit conventions — see `notif-cli::main`.
        42 => Err(MacosError::NotSigned),
        43 => Err(MacosError::AuthorizationDenied),
        _ => Err(MacosError::OpenFailed(code)),
    }
}

// -----------------------------------------------------------------------
// Inner-mode (objc2 / UN center) — gated behind macOS, otherwise no-op.
// -----------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod inner {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString, NSUUID};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };

    use notif_core::Notification;

    use crate::error::MacosError;

    /// Timeout for `add(request)` completion — UN delivery is a fast
    /// background hop.
    const DISPATCH_TIMEOUT: Duration = Duration::from_secs(2);
    /// Timeout for `requestAuthorization` completion — waits for the user
    /// to click "Allow" / "Don't Allow" on the system dialog.
    const AUTH_TIMEOUT: Duration = Duration::from_secs(60);
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    /// Inner-mode send. Called from `main.rs` when `is_inner_mode()` returns
    /// true and the subcommand is `Send`.
    pub fn dispatch_inner(notif: &Notification) -> Result<(), MacosError> {
        let center = UNUserNotificationCenter::currentNotificationCenter();

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notif.title));
        content.setBody(&NSString::from_str(&notif.body));
        if let Some(sub) = &notif.subtitle {
            content.setSubtitle(&NSString::from_str(sub));
        }

        let identifier = NSUUID::UUID().UUIDString();
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
            &content,
            None,
        );

        let slot: Arc<Mutex<Option<Result<(), MacosError>>>> = Arc::new(Mutex::new(None));
        let slot_cb = slot.clone();
        let block = RcBlock::new(move |err: *mut NSError| {
            let res = classify_ns_error(err);
            *slot_cb.lock().unwrap() = Some(res);
        });

        center.addNotificationRequest_withCompletionHandler(&request, Some(&block));

        wait_for_slot(&slot, DISPATCH_TIMEOUT)
    }

    /// Inner-mode setup. Fires `requestAuthorizationWithOptions:` and blocks
    /// on completion. Uses [`AUTH_TIMEOUT`] since the system dialog needs a
    /// human click.
    ///
    /// Surfaces [`MacosError::AuthorizationDenied`] when the completion
    /// handler reports `granted == false` (user clicked "Don't Allow" or the
    /// bundle was previously denied via TCC).
    pub fn setup_inner() -> Result<(), MacosError> {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let opts = UNAuthorizationOptions::Alert
            | UNAuthorizationOptions::Sound
            | UNAuthorizationOptions::Badge;

        let slot: Arc<Mutex<Option<Result<(), MacosError>>>> = Arc::new(Mutex::new(None));
        let slot_cb = slot.clone();
        let block = RcBlock::new(move |granted: Bool, err: *mut NSError| {
            let res = if let Err(e) = classify_ns_error(err) {
                Err(e)
            } else if granted.as_bool() {
                Ok(())
            } else {
                Err(MacosError::AuthorizationDenied)
            };
            *slot_cb.lock().unwrap() = Some(res);
        });

        center.requestAuthorizationWithOptions_completionHandler(opts, &block);

        wait_for_slot(&slot, AUTH_TIMEOUT)
    }

    fn classify_ns_error(err: *mut NSError) -> Result<(), MacosError> {
        if err.is_null() {
            return Ok(());
        }
        let msg = unsafe { (*err).localizedDescription() }.to_string();
        let lower = msg.to_lowercase();
        if lower.contains("not signed") || lower.contains("code signature") {
            Err(MacosError::NotSigned)
        } else {
            Err(MacosError::Objc(msg))
        }
    }

    fn wait_for_slot(
        slot: &Arc<Mutex<Option<Result<(), MacosError>>>>,
        timeout: Duration,
    ) -> Result<(), MacosError> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if slot.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        slot.lock()
            .unwrap()
            .take()
            .unwrap_or(Err(MacosError::Timeout))
    }
}

#[cfg(target_os = "macos")]
pub use inner::{dispatch_inner, setup_inner};

// Non-macOS stubs so the workspace builds on the dev host (linux).
#[cfg(not(target_os = "macos"))]
pub fn dispatch_inner(_notif: &Notification) -> Result<(), MacosError> {
    unreachable!("inner mode is macOS-only")
}
#[cfg(not(target_os = "macos"))]
pub fn setup_inner() -> Result<(), MacosError> {
    unreachable!("inner mode is macOS-only")
}

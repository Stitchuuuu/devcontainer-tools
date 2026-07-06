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

use notif_core::callback::CallbackConfig;
use notif_core::Notification;

use crate::bundle::{ad_hoc_codesign, ensure_bundle};
use crate::error::MacosError;
use crate::overrides::MacosOverrides;

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

/// Outer-mode entry point. Called by [`crate::backend::MacosBackend::dispatch`]
/// (empty overrides + callbacks) and by the CLI directly (populated forms).
///
/// Ensures the bundle exists, spawns the bundled `notif` directly, retries
/// once with an ad-hoc codesign if the inner exit reports
/// [`MacosError::NotSigned`], and propagates the Tier 3 `--macos-*` overrides
/// + `--on-*` callback config through the outer→inner CLI hop.
///
/// # Errors
/// Bubbles anything from [`ensure_bundle`] plus [`MacosError::OpenFailed`] or
/// [`MacosError::NotSigned`] on second failure.
pub fn dispatch_outer(
    notif: &Notification,
    overrides: &MacosOverrides,
    callbacks: &CallbackConfig,
) -> Result<(), MacosError> {
    let display = notif.sender.key.clone();
    let bundle = ensure_bundle(&notif.sender.key, &display, None, None)?;

    match invoke_inner_send(&bundle, notif, overrides, callbacks) {
        Ok(()) => Ok(()),
        Err(MacosError::NotSigned) => {
            ad_hoc_codesign(&bundle)?;
            invoke_inner_send(&bundle, notif, overrides, callbacks)
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

/// Outer-mode entry point for the **first-time** authorization dance on a
/// freshly-materialized bundle. Seeds the LSDB entry via `lsregister -f`
/// (one-shot cost, no launch) before delegating to the standard
/// direct-spawn setup path.
///
/// Rationale: session-3's direct-spawn optimization skips LaunchServices
/// on every send to avoid LSDB churn, but that requires the bundle to
/// **already be LSDB-known**. On a brand-new sender (never launched via
/// `open -a`), `UNUserNotificationCenter.requestAuthorization` refuses
/// with `UNErrorCode 1` = "notifications not allowed" without even
/// showing the permission dialog. `lsregister -f` registers the bundle
/// with LSDB without spawning it, satisfying the prerequisite exactly
/// once per new sender. Subsequent sends fall back to [`setup_outer`]
/// (direct-spawn, zero LSDB traffic).
///
/// # Errors
/// Same as [`setup_outer`], plus [`MacosError::OpenFailed`] if
/// `lsregister` itself fails.
pub fn setup_outer_bootstrap(sender_key: &str) -> Result<(), MacosError> {
    let bundle = ensure_bundle(sender_key, sender_key, None, None)?;
    register_with_lsdb(&bundle)?;
    invoke_inner_setup(&bundle, sender_key)
}

/// LaunchServices `lsregister` — private but stable across decades. The
/// canonical way to register a bundle with LSDB without launching it.
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// One-shot LSDB registration for a freshly-materialized bundle. See
/// [`setup_outer_bootstrap`] for the rationale.
fn register_with_lsdb(bundle: &Path) -> Result<(), MacosError> {
    let status = Command::new(LSREGISTER).arg("-f").arg(bundle).status()?;
    if !status.success() {
        return Err(MacosError::OpenFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
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

fn invoke_inner_send(
    bundle: &Path,
    notif: &Notification,
    overrides: &MacosOverrides,
    callbacks: &CallbackConfig,
) -> Result<(), MacosError> {
    let mut cmd = Command::new(inner_exe_path(bundle));
    cmd.arg("send")
        .arg("--title").arg(&notif.title)
        .arg("--body").arg(&notif.body)
        .arg("--sender").arg(&notif.sender.key);
    if let Some(sub) = &notif.subtitle {
        cmd.arg("--subtitle").arg(sub);
    }
    // priority always propagates — inner mode reads it back via clap and
    // rebuilds the same `Notification`, so an implicit `Priority::Normal`
    // still round-trips explicitly to avoid a silent default drift.
    cmd.arg("--priority").arg(notif.priority.wire_str());
    if let Some(s) = &notif.sound {
        cmd.arg("--sound").arg(s.wire_str());
    }
    if let Some(p) = &notif.image {
        cmd.arg("--image").arg(p);
    }
    if let Some(id) = &notif.id {
        cmd.arg("--id").arg(id);
    }
    // `notif.on_timeout: Option<TimeoutBehavior>` (portable auto-dismiss
    // behavior enum) is deliberately NOT serialized here — the macOS CLI
    // routes `--on-timeout` through [`CallbackConfig`] instead (target
    // string, not enum). The field stays on the portable `Notification`
    // struct for Windows/Linux backends that may still honor it, and any
    // caller that sets it explicitly through the library API can serialize
    // it here later without breaking wire compatibility.

    // ---- Tier 3 `--macos-*` overrides ------------------------------------
    if let Some(name) = &overrides.sound_name {
        cmd.arg("--macos-sound-name").arg(name);
    }
    if let Some(path) = &overrides.attachment {
        cmd.arg("--macos-attachment").arg(path);
    }
    if let Some(il) = overrides.interruption_level {
        cmd.arg("--macos-interruption-level").arg(il.wire_str());
    }
    if let Some(tid) = &overrides.thread_identifier {
        cmd.arg("--macos-thread-identifier").arg(tid);
    }
    if let Some(cid) = &overrides.category_identifier {
        cmd.arg("--macos-category-identifier").arg(cid);
    }

    // ---- Callback flag surface -------------------------------------------
    // Each target round-trips through `CallbackTarget::to_wire()` — auto-
    // detect payloads get canonicalized (e.g. `/tmp/x` → `file:/tmp/x`) so
    // the inner reparses to the same shape as the outer built.
    if let Some(t) = &callbacks.on_click {
        cmd.arg("--on-click").arg(t.to_wire());
    }
    for (label, t) in &callbacks.on_actions {
        cmd.arg("--on-action").arg(format!("{label}:{}", t.to_wire()));
    }
    if let Some(t) = &callbacks.on_dismiss {
        cmd.arg("--on-dismiss").arg(t.to_wire());
    }
    if let Some(t) = &callbacks.on_timeout {
        cmd.arg("--on-timeout").arg(t.to_wire());
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
    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSArray, NSError, NSSet, NSString, NSURL, NSUUID};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationAction,
        UNNotificationActionOptions, UNNotificationAttachment, UNNotificationCategory,
        UNNotificationCategoryOptions, UNNotificationInterruptionLevel, UNNotificationRequest,
        UNNotificationSound, UNUserNotificationCenter,
    };

    use notif_core::callback::CallbackConfig;
    use notif_core::{Notification, Priority, Sound};

    use crate::error::MacosError;
    use crate::overrides::{InterruptionLevel, MacosOverrides};

    /// Map portable `Priority` to Apple's `UNNotificationInterruptionLevel`.
    ///
    /// Contract mirrored by [`notif_core::Priority::level`] (which returns the
    /// same raw values) so both crates test-cover the mapping independently.
    fn priority_to_interruption_level(p: Priority) -> UNNotificationInterruptionLevel {
        match p {
            Priority::Low => UNNotificationInterruptionLevel::Passive,
            Priority::Normal => UNNotificationInterruptionLevel::Active,
            Priority::High => UNNotificationInterruptionLevel::TimeSensitive,
            Priority::Critical => UNNotificationInterruptionLevel::Critical,
        }
    }

    /// Tier 3 override — map [`InterruptionLevel`] (native camelCase from
    /// `--macos-interruption-level`) to the Apple raw type. Distinct from
    /// [`priority_to_interruption_level`] because Tier 3 uses the raw
    /// four-value macOS vocabulary; portable [`Priority`] carries the
    /// three-tier abstraction the CLI's `--priority` presents.
    fn macos_interruption_to_apple(v: InterruptionLevel) -> UNNotificationInterruptionLevel {
        match v {
            InterruptionLevel::Passive => UNNotificationInterruptionLevel::Passive,
            InterruptionLevel::Active => UNNotificationInterruptionLevel::Active,
            InterruptionLevel::TimeSensitive => UNNotificationInterruptionLevel::TimeSensitive,
            InterruptionLevel::Critical => UNNotificationInterruptionLevel::Critical,
        }
    }

    /// Rough "is this a path?" heuristic for the `Custom(String)` variant of
    /// `Sound`. Bare names like `Ping` map to a bundled sound; anything with
    /// a `/` becomes a file URL. Absolute paths and `~`-relative are covered;
    /// bare-name-with-dot (e.g. `Ping.caf`) still counts as a bundled name
    /// because macOS looks up unqualified sound identifiers in
    /// `Library/Sounds`.
    fn looks_like_path(s: &str) -> bool {
        s.starts_with('/') || s.starts_with('~') || s.contains('/')
    }

    /// Turn a `Sound` selector into a `UNNotificationSound`. `Custom` with a
    /// path becomes `soundNamed(<basename>)` after copying the file into
    /// place is out of scope for v0.1 — for now the basename is passed as-is,
    /// and macOS will resolve it against the sender bundle's `Resources/`
    /// or the system sound library. Full file-URL sound handling lands in
    /// v0.2 when the sender bundle grows a `Resources/Sounds/` slot.
    fn build_sound(s: &Sound) -> Retained<UNNotificationSound> {
        match s {
            Sound::Default => UNNotificationSound::defaultSound(),
            Sound::Alert => UNNotificationSound::defaultCriticalSound(),
            Sound::Custom(v) => {
                let name = if looks_like_path(v) {
                    std::path::Path::new(v)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(v.as_str())
                } else {
                    v.as_str()
                };
                UNNotificationSound::soundNamed(&NSString::from_str(name))
            }
        }
    }

    /// Timeout for `add(request)` completion — UN delivery is a fast
    /// background hop.
    const DISPATCH_TIMEOUT: Duration = Duration::from_secs(2);
    /// Timeout for `requestAuthorization` completion — waits for the user
    /// to click "Allow" / "Don't Allow" on the system dialog.
    const AUTH_TIMEOUT: Duration = Duration::from_secs(60);
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    /// Inner-mode send. Called from `main.rs` when `is_inner_mode()` returns
    /// true and the subcommand is `Send`.
    ///
    /// Applies the portable `Notification` fields first, then layers the
    /// Tier 3 `--macos-*` overrides on top per the "native wins" rule.
    /// Each override emits a per-category `info:` line via
    /// [`notif_core::warn::info`] when it shadows a portable equivalent —
    /// silent when no portable counterpart is set.
    ///
    /// [`CallbackConfig`] is accepted here for surface completeness (the
    /// outer→inner CLI hop propagates every `--on-*` flag so the inner
    /// contract stays stable regardless of dispatcher implementation).
    /// Actual delegate dispatch is owned by the `notif listen` daemon —
    /// the delegate wired on the UN center here just logs the stub when
    /// callbacks were registered.
    pub fn dispatch_inner(
        notif: &Notification,
        overrides: &MacosOverrides,
        callbacks: &CallbackConfig,
    ) -> Result<(), MacosError> {
        let center = UNUserNotificationCenter::currentNotificationCenter();

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notif.title));
        content.setBody(&NSString::from_str(&notif.body));
        if let Some(sub) = &notif.subtitle {
            content.setSubtitle(&NSString::from_str(sub));
        }

        // ---- Priority / interruption level ------------------------------
        // Portable `--priority` first, then Tier 3 override wins with an
        // `info:` line when both are set.
        content.setInterruptionLevel(priority_to_interruption_level(notif.priority));
        if let Some(il) = overrides.interruption_level {
            if notif.priority != Priority::Normal {
                // Only log when the user explicitly passed a portable
                // `--priority` (Normal is the parser default). Otherwise
                // Tier 3 is refining the OS default, not shadowing a user
                // choice.
                notif_core::warn::info(
                    "macos_interruption_overrides_priority",
                    "--priority overridden by --macos-interruption-level",
                );
            }
            content.setInterruptionLevel(macos_interruption_to_apple(il));
        }

        // ---- Sound ------------------------------------------------------
        if let Some(s) = &notif.sound {
            let sound = build_sound(s);
            content.setSound(Some(&sound));
        }
        if let Some(name) = &overrides.sound_name {
            if notif.sound.is_some() {
                notif_core::warn::info(
                    "macos_sound_name_overrides_sound",
                    "--sound overridden by --macos-sound-name",
                );
            }
            let ns_name = NSString::from_str(name);
            let sound = UNNotificationSound::soundNamed(&ns_name);
            content.setSound(Some(&sound));
        }

        // ---- Image / attachment -----------------------------------------
        // Portable `--image` first, Tier 3 `--macos-attachment` overrides.
        // Both funnel through the same `attachmentWithIdentifier:URL:` call
        // — same failure handling.
        let attachment_path = overrides
            .attachment
            .as_ref()
            .or(notif.image.as_ref());
        if let Some(path) = attachment_path {
            if notif.image.is_some() && overrides.attachment.is_some() {
                notif_core::warn::info(
                    "macos_attachment_overrides_image",
                    "--image overridden by --macos-attachment",
                );
            }
            let ns_path = NSString::from_str(&path.to_string_lossy());
            let url = NSURL::fileURLWithPath(&ns_path);
            let att_id = NSString::from_str("image");
            let att_result = unsafe {
                UNNotificationAttachment::attachmentWithIdentifier_URL_options_error(
                    &att_id, &url, None,
                )
            };
            match att_result {
                Ok(att) => {
                    let arr = NSArray::from_slice(&[&*att]);
                    content.setAttachments(&arr);
                }
                Err(err) => {
                    let msg = err.localizedDescription().to_string();
                    notif_core::warn::emit(
                        "image_attachment_refused",
                        &format!(
                            "attachment refused by UN center ({msg}); notification will be delivered without attachment"
                        ),
                    );
                }
            }
        }

        // ---- Tier 3 grouping identifiers (no portable counterpart) -------
        if let Some(tid) = &overrides.thread_identifier {
            content.setThreadIdentifier(&NSString::from_str(tid));
        }

        // ---- Action button registration ----------------------------------
        // `UNNotificationCategory` is the macOS primitive that maps a set
        // of tappable buttons ([`UNNotificationAction`]) onto a
        // notification. Without registering the category and setting
        // `content.setCategoryIdentifier(...)`, the banner delivers with
        // no visible buttons even when `--on-action` is passed. The
        // click dispatch itself (target invocation) is the daemon's job
        // — this branch just makes the buttons appear.
        //
        // Priority for the category identifier :
        //   1. `--macos-category-identifier <id>` (Tier 3 raw override).
        //   2. Auto-generated `notif-<hex>` when actions are declared but
        //      no raw override.
        //   3. None (no actions → no category, banner is body-only).
        // A category is needed whenever :
        //   - `--on-action` is set (buttons need a category).
        //   - `--on-dismiss` is set — UN center will NOT emit
        //     `didReceiveNotificationResponse` with the `dismiss` action
        //     identifier unless the notification's category carries the
        //     `.customDismissAction` option. Without that flag, the
        //     system dismisses silently and the delegate never fires.
        //   - `--macos-category-identifier` override was passed (Tier 3
        //     raw override).
        let needs_category = overrides.category_identifier.is_some()
            || !callbacks.on_actions.is_empty()
            || callbacks.on_dismiss.is_some();
        let effective_category_id = overrides.category_identifier.clone().or_else(|| {
            if needs_category {
                Some(format!(
                    "notif-cat-{}",
                    notif.id.as_deref().unwrap_or("auto"),
                ))
            } else {
                None
            }
        });

        if needs_category {
            let cat_id_str = effective_category_id
                .as_deref()
                .expect("needs_category implies effective_category_id set");
            let cat_id = NSString::from_str(cat_id_str);

            // Build UNNotificationAction items in the exact order the
            // user typed `--on-action label:target` flags. macOS renders
            // them in registration order, so preserving the CLI order
            // matches the user's mental model.
            let mut action_ptrs: Vec<Retained<UNNotificationAction>> =
                Vec::with_capacity(callbacks.on_actions.len());
            for (label, _target) in &callbacks.on_actions {
                let id_s = NSString::from_str(label);
                // Label doubles as button title until a `title:<foo>`
                // convention lands. Users who want a distinct button
                // title today set the label to what they want displayed.
                let title_s = NSString::from_str(label);
                let opts = UNNotificationActionOptions::empty();
                let action = UNNotificationAction::actionWithIdentifier_title_options(
                    &id_s, &title_s, opts,
                );
                action_ptrs.push(action);
            }
            let refs: Vec<&UNNotificationAction> =
                action_ptrs.iter().map(std::convert::AsRef::as_ref).collect();
            let actions_arr = NSArray::from_slice(&refs);
            let intents_arr: Retained<NSArray<NSString>> = NSArray::new();
            // Opt in to `didReceiveNotificationResponse` for the dismiss
            // action iff `--on-dismiss` was set. Off otherwise — the
            // system's default dismiss is cheaper (no delegate roundtrip
            // per swipe-away).
            let cat_opts = if callbacks.on_dismiss.is_some() {
                UNNotificationCategoryOptions::CustomDismissAction
            } else {
                UNNotificationCategoryOptions::empty()
            };
            let category =
                UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                    &cat_id,
                    &actions_arr,
                    &intents_arr,
                    cat_opts,
                );
            let cat_ref: &UNNotificationCategory = category.as_ref();
            let categories: Retained<NSSet<UNNotificationCategory>> =
                NSSet::from_slice(&[cat_ref]);
            // `setNotificationCategories` REPLACES the registered set on
            // the UN center. Fire-and-forget CLI invocations are fine
            // with that — the daemon (`notif listen`) merges sets
            // implicitly by re-registering per send.
            center.setNotificationCategories(&categories);

            let action_labels: Vec<&str> = callbacks
                .on_actions
                .iter()
                .map(|(l, _)| l.as_str())
                .collect();
            notif_core::warn::stderr(&format!(
                "registered UN category '{}' with {} action(s){}{}",
                cat_id_str,
                callbacks.on_actions.len(),
                if action_labels.is_empty() { "" } else { ": " },
                action_labels.join(", "),
            ));
        }

        if let Some(cid) = &effective_category_id {
            content.setCategoryIdentifier(&NSString::from_str(cid));
        }

        // (Session 7b) The "callback stub: N target(s) registered" log
        // that lived here in v0.2 → 7a is now unreachable : the outer
        // routes any send-with-callbacks through `notif listen` (which
        // calls back into this same fn) so the delegate wiring exists
        // by the time `addNotificationRequest` fires. From inside the
        // daemon, action buttons are attached above and click / dismiss
        // callbacks are bound in the registry — no user-facing stub
        // announcement is warranted.

        let identifier = match &notif.id {
            Some(v) => NSString::from_str(v),
            None => NSUUID::UUID().UUIDString(),
        };
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
            &content,
            None,
        );

        // Snapshot the resolved identifier for the audit trail — `notif.id`
        // was optional (backend minted a UUID here), so log the value that
        // was actually sent so callers can correlate `remove` calls.
        let dispatched_id = identifier.to_string();
        notif_core::warn::stderr(&format!(
            "dispatching notif id='{}' sender='{}' title={:?}",
            dispatched_id, notif.sender.key, notif.title,
        ));

        let slot: Arc<Mutex<Option<Result<(), MacosError>>>> = Arc::new(Mutex::new(None));
        let slot_cb = slot.clone();
        let block = RcBlock::new(move |err: *mut NSError| {
            let res = classify_ns_error(err);
            *slot_cb.lock().unwrap() = Some(res);
        });

        center.addNotificationRequest_withCompletionHandler(&request, Some(&block));

        let result = wait_for_slot(&slot, DISPATCH_TIMEOUT);
        let final_result = match result {
            Err(MacosError::AttachmentRefused(msg)) => {
                notif_core::warn::emit(
                    "attachment_move_failed",
                    &format!(
                        "UN center refused attachment ({msg}); retrying without attachment",
                    ),
                );
                // Rebuild the request without the attachment. UN retains
                // a snapshot of `content` at request creation, so we
                // clear on the content object AND re-issue the request
                // to be safe against any lingering reference.
                content.setAttachments(&NSArray::from_slice(&[]));
                let retry_request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                    &identifier,
                    &content,
                    None,
                );
                let retry_slot: Arc<Mutex<Option<Result<(), MacosError>>>> =
                    Arc::new(Mutex::new(None));
                let retry_slot_cb = retry_slot.clone();
                let retry_block = RcBlock::new(move |err: *mut NSError| {
                    *retry_slot_cb.lock().unwrap() = Some(classify_ns_error(err));
                });
                center.addNotificationRequest_withCompletionHandler(
                    &retry_request,
                    Some(&retry_block),
                );
                wait_for_slot(&retry_slot, DISPATCH_TIMEOUT)
            }
            other => other,
        };
        match &final_result {
            Ok(()) => notif_core::warn::stderr(&format!(
                "delivered notif id='{dispatched_id}' via UN center",
            )),
            Err(e) => notif_core::warn::stderr(&format!(
                "delivery failed notif id='{dispatched_id}': {e}",
            )),
        }
        final_result
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
        } else if lower.contains("attachment") {
            // Covers "Failed to move attachment file into data store"
            // and every other UN-side attachment refusal (bad extension,
            // sandbox path, etc.). Caller decides whether to retry
            // without the attachment or fail.
            Err(MacosError::AttachmentRefused(msg))
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn priority_to_il_table() {
            // Table test — the raw values are Apple-documented and MUST match
            // `notif_core::Priority::level` in lockstep, otherwise the wire
            // format silently drifts.
            for (p, expected_raw) in [
                (Priority::Low, 0),
                (Priority::Normal, 1),
                (Priority::High, 2),
                (Priority::Critical, 3),
            ] {
                let il = priority_to_interruption_level(p);
                assert_eq!(il.0 as u8, expected_raw, "for {p:?}");
                assert_eq!(il.0 as u8, p.level(), "level() drift for {p:?}");
            }
        }

        #[test]
        fn looks_like_path_heuristic() {
            for s in ["/System/Library/Sounds/Glass.aiff", "~/foo.caf", "sub/dir/x"] {
                assert!(looks_like_path(s), "{s:?} should be a path");
            }
            for s in ["Ping", "Glass", "Ping.caf"] {
                assert!(!looks_like_path(s), "{s:?} should NOT be a path");
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use inner::{dispatch_inner, setup_inner};

// Non-macOS stubs so the workspace builds on the dev host (linux).
#[cfg(not(target_os = "macos"))]
pub fn dispatch_inner(
    _notif: &Notification,
    _overrides: &MacosOverrides,
    _callbacks: &CallbackConfig,
) -> Result<(), MacosError> {
    unreachable!("inner mode is macOS-only")
}
#[cfg(not(target_os = "macos"))]
pub fn setup_inner() -> Result<(), MacosError> {
    unreachable!("inner mode is macOS-only")
}

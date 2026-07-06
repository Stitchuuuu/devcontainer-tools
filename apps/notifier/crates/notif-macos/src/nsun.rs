//! Tier 1 identity spoof via `[NSBundle bundleIdentifier]` swizzle.
//!
//! Dispatches a notification through the deprecated `NSUserNotificationCenter`
//! API under an arbitrary bundle identifier, so the banner appears in
//! Notification Center under the target app's name + icon **without
//! materializing our own `.app` bundle**.
//!
//! # How it works
//!
//! `NSUserNotificationCenter.defaultUserNotificationCenter().deliverNotification:`
//! internally asks `[NSBundle mainBundle].bundleIdentifier` and hands the
//! resulting string to LaunchServices to resolve the display name + icon that
//! Notification Center renders. If we swap the IMP of
//! `-[NSBundle bundleIdentifier]` at runtime, the getter returns whatever we
//! ask it to — LSDB does the rest.
//!
//! # Tradeoffs vs Tier 0 / Tier 2
//!
//! `NSUserNotification` predates the modern `UserNotifications` framework
//! (`UN` prefix, macOS 10.14+). Available since 10.8, deprecated 10.14, still
//! functional through macOS 26. Consequences :
//!
//! - No image attachments, no interruption levels, no custom categories.
//! - The delegate is `NSUserNotificationCenterDelegate`, a distinct protocol
//!   from `UNUserNotificationCenterDelegate` — Tier 1 is fire-and-forget in
//!   v0.2 (no click / dismiss callbacks).
//! - The CLI-side gate in `notif-cli` refuses combining Tier 1 with any
//!   of the unsupported flags, so this module doesn't have to degrade
//!   gracefully — it just delivers.
//!
//! # Swizzle contract
//!
//! - Idempotent: repeated calls to [`dispatch_via_nsun`] within the same
//!   process reuse the first-registered spoof (Tier 1 is one-shot per CLI
//!   invocation ; the CLI never calls this twice with different identifiers
//!   in the same process, but the invariant is documented).
//! - Not restored on exit — process is short-lived, no delegate to unwind.
//! - Global side-effect: ALL callers of `[NSBundle bundleIdentifier]` in the
//!   process see the spoofed value. Acceptable given the CLI exits within
//!   seconds and Tier 1 has no long-lived listener.

// The whole point of this module is to call the deprecated
// `NSUserNotification*` API — the deprecation is *why* Tier 1 exists.
// Suppress the noise at the module level so the compile output stays
// meaningful for real regressions.
#![allow(deprecated)]

use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;
use objc2_foundation::{
    NSString, NSUserNotification, NSUserNotificationCenter, NSUserNotificationDefaultSoundName,
};

use notif_core::{Notification, Sound};

use crate::error::MacosError;

/// Send + Sync wrapper for the spoofed-identifier anchor.
///
/// `Retained<NSString>` is not `Send + Sync` because objc2 conservatively
/// marks all ObjC objects that way. Immutable `NSString` instances (which
/// is what `NSString::from_str` produces) are documented by Apple as
/// thread-safe : the object is never mutated, `retain` / `release` are
/// atomic. The static lives for the process lifetime and is only read
/// after `OnceLock::get_or_init` returns, so multi-threaded access is
/// entirely `get()` (immutable borrow) on an object that never changes.
struct SpoofedId(Retained<NSString>);

// SAFETY: see doc comment on `SpoofedId`.
unsafe impl Send for SpoofedId {}
unsafe impl Sync for SpoofedId {}

/// Retained anchor for the spoofed identifier — keeps the `NSString` alive
/// for the process lifetime so `Retained::as_ptr` handoff to Objective-C
/// stays valid regardless of retain/release on the returned pointer.
static SPOOFED_ID: OnceLock<SpoofedId> = OnceLock::new();

/// Marks the swizzle as installed — makes install idempotent across
/// repeated `dispatch_via_nsun` calls in the same process.
static SWIZZLED: OnceLock<()> = OnceLock::new();

/// Replacement IMP for `-[NSBundle bundleIdentifier]`. Returns the spoofed
/// identifier regardless of receiver.
///
/// # Safety
/// Callable only after [`swizzle_bundle_identifier`] has initialized
/// `SPOOFED_ID` and installed this fn as the IMP for the selector. The
/// installer enforces both via `OnceLock::get_or_init`.
unsafe extern "C" fn spoofed_bundle_identifier(
    _this: *mut AnyObject,
    _sel: Sel,
) -> *mut AnyObject {
    // Panicking across an ObjC boundary is UB, so we `expect()` on a
    // condition the installer guarantees: SPOOFED_ID is set as the first
    // statement of `swizzle_bundle_identifier`, and the IMP swap is the
    // last. By the time this IMP can be called, SPOOFED_ID is populated.
    let anchor: &SpoofedId = SPOOFED_ID
        .get()
        .expect("SPOOFED_ID must be initialized before swizzle install");
    // Retained keeps its own strong ref forever; the caller may retain the
    // returned pointer as needed per ARC getter convention.
    Retained::as_ptr(&anchor.0) as *mut AnyObject
}

/// Install the `-[NSBundle bundleIdentifier]` swizzle. Idempotent — the
/// second call with a different `spoofed` argument is a no-op (the
/// first-installed value stays), which is documented as the one-shot
/// contract of Tier 1.
fn swizzle_bundle_identifier(spoofed: &str) {
    SPOOFED_ID.get_or_init(|| SpoofedId(NSString::from_str(spoofed)));
    SWIZZLED.get_or_init(|| {
        // SAFETY: we replace the IMP of an existing selector on a real
        // class. The new IMP has the correct ARC signature (id-returning
        // getter). SPOOFED_ID is already initialized above.
        unsafe {
            let cls = AnyClass::get(c"NSBundle")
                .expect("NSBundle class must exist in the ObjC runtime");
            let method = cls
                .instance_method(sel!(bundleIdentifier))
                .expect("-[NSBundle bundleIdentifier] must exist");
            let imp: unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject =
                spoofed_bundle_identifier;
            method.set_implementation(std::mem::transmute::<
                unsafe extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject,
                Imp,
            >(imp));
        }
    });
}

/// Deliver `notif` through `NSUserNotificationCenter` with the process's
/// `NSBundle.bundleIdentifier` swizzled to `identifier`.
///
/// Emits a dedup'd `warning:` line about the NSUserNotification API being
/// deprecated (suppressible via `--quiet` or `NOTIF_QUIET=1`).
///
/// # Errors
/// This entry point is fire-and-forget — NSUserNotification's
/// `deliverNotification:` has no synchronous error channel. Returns
/// `Ok(())` after the call is made ; misconfiguration (nonexistent
/// identifier, denied permission) surfaces only in Notification Center
/// silence. The CLI-side gate validates the identifier via Spotlight
/// before we get here, so the "typo'd identifier" failure mode is
/// already handled upstream.
pub fn dispatch_via_nsun(notif: &Notification, identifier: &str) -> Result<(), MacosError> {
    // Note : the "NSUserNotification is deprecated / may not deliver" warning
    // fires in `run_macos` before we get here, gated on macOS major version
    // (warn on 10.14–14, hard-error on 15+). Emitting a second warning here
    // would duplicate signal.
    swizzle_bundle_identifier(identifier);

    let n = NSUserNotification::new();
    n.setTitle(Some(&NSString::from_str(&notif.title)));
    n.setInformativeText(Some(&NSString::from_str(&notif.body)));
    if let Some(sub) = &notif.subtitle {
        n.setSubtitle(Some(&NSString::from_str(sub)));
    }
    if let Some(id) = &notif.id {
        n.setIdentifier(Some(&NSString::from_str(id)));
    }
    if let Some(sound) = &notif.sound {
        match sound {
            Sound::Default => {
                // SAFETY: `NSUserNotificationDefaultSoundName` is a static
                // NSString living in Foundation ; passing a `&'static
                // NSString` to `setSoundName` is the documented use.
                let name = unsafe { NSUserNotificationDefaultSoundName };
                n.setSoundName(Some(name));
            }
            Sound::Alert => {
                // NS has no distinct "alert" sound alias ; fall back to
                // default and log the degradation. Consistent with the
                // portable-vs-native precedent in dispatch::inner.
                notif_core::warn::info(
                    "tier1_alert_falls_back_to_default",
                    "--sound alert has no NS equivalent; using default system sound",
                );
                let name = unsafe { NSUserNotificationDefaultSoundName };
                n.setSoundName(Some(name));
            }
            Sound::Custom(v) => {
                let name = NSString::from_str(v);
                n.setSoundName(Some(&name));
            }
        }
    }

    let center = NSUserNotificationCenter::defaultUserNotificationCenter();
    center.deliverNotification(&n);

    Ok(())
}

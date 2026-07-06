//! Isolated integration test for the `NSBundle.bundleIdentifier` swizzle.
//!
//! Cargo compiles each `tests/*.rs` file to its own binary, so the swizzle's
//! process-global side-effect (patching an ObjC method IMP) never leaks into
//! other test crates. macOS-only: on Linux the file compiles to nothing.
//!
//! This test is executed manually against a real Mac host via
//!
//! ```
//! cargo test -p notif-macos --test swizzle_bundle_identifier
//! ```
//!
//! and does not require the UN center, TCC grants, or any bundle
//! materialization.

#![cfg(target_os = "macos")]

use notif_core::{Notification, Priority, Sender};

/// Assert that `[NSBundle mainBundle].bundleIdentifier` reflects the value
/// passed to the last `dispatch_via_nsun` call, AND that a second call with
/// a different identifier is a no-op (one-shot contract).
#[test]
fn swizzle_installs_and_is_one_shot() {
    use objc2_foundation::NSBundle;

    // Empty title / body — we're not checking delivery, only that the
    // swizzle installed correctly. deliverNotification is fire-and-forget
    // and won't error even without an authorized bundle.
    let notif = Notification {
        title: "swizzle-test".into(),
        body: "unused".into(),
        subtitle: None,
        priority: Priority::Normal,
        sender: Sender::default(),
        id: None,
        sound: None,
        image: None,
        on_timeout: None,
    };

    // Before install, mainBundle().bundleIdentifier is either None (raw
    // test-binary process, no bundle) or the test-runner's own identifier
    // (e.g. `com.apple.dt.Xcode.testrunner` under Xcode). Not asserted —
    // implementation-defined baseline.

    let first = "com.example.tier1-test-first";
    notif_macos::nsun::dispatch_via_nsun(&notif, first).expect("first dispatch");

    let bundle = NSBundle::mainBundle();
    let observed = bundle
        .bundleIdentifier()
        .expect("bundleIdentifier must return Some after swizzle");
    assert_eq!(
        observed.to_string(),
        first,
        "swizzled bundleIdentifier must return the first-installed value",
    );

    // One-shot contract — a second call with a *different* identifier does
    // NOT replace the first. Callers get the same value.
    let second = "com.example.tier1-test-second";
    notif_macos::nsun::dispatch_via_nsun(&notif, second).expect("second dispatch");

    let bundle = NSBundle::mainBundle();
    let still_first = bundle
        .bundleIdentifier()
        .expect("bundleIdentifier must return Some");
    assert_eq!(
        still_first.to_string(),
        first,
        "second dispatch_via_nsun must NOT overwrite the first-installed identifier",
    );
}

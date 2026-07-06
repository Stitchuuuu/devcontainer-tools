//! `UNUserNotificationCenterDelegate` implementation.
//!
//! The delegate captures the shared [`Registry`] and dispatches callbacks
//! when the user clicks / dismisses / actions a notification. Wired on
//! the center via [`crate::daemon::run_daemon`] once at daemon start.
//!
//! Key subtlety : `UNUserNotificationCenter` invokes delegate methods on
//! the main queue (per Apple's documentation). This crate's daemon runs
//! its main thread inside `CFRunLoopRunInMode`, which drains the main
//! queue between iterations. Delegate methods therefore run on the main
//! thread ; the completion handler must be called before the delegate
//! method returns, or UN center will keep the notification undelivered.

use std::sync::OnceLock;

use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_user_notifications::{
    UNNotificationDefaultActionIdentifier, UNNotificationDismissActionIdentifier,
    UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use crate::daemon::registry::{Registry, ResponseKind};

/// Interior-mutable data attached to the delegate class instance.
///
/// Only the `Registry` — which is itself `Arc<Mutex<HashMap>>`, so no
/// additional cell wrapping is needed.
pub struct DelegateIvars {
    pub registry: Registry,
}

define_class!(
    /// Notification center delegate that maps `didReceiveNotificationResponse`
    /// events onto registered callbacks.
    #[unsafe(super(NSObject))]
    #[ivars = DelegateIvars]
    pub struct NotifDelegate;

    unsafe impl NSObjectProtocol for NotifDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotifDelegate {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn on_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            handle_response(&self.ivars().registry, response);
            // UN center requires the completion handler to fire before
            // the method returns. Failing to call it leaks the response
            // in the system's delivery queue.
            completion.call(());
        }
    }
);

impl NotifDelegate {
    /// Build a fresh delegate holding a clone of `registry`. Registration
    /// on the notification center happens at the call site.
    pub fn new(registry: Registry) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { registry });
        unsafe { msg_send![super(this), init] }
    }

    /// Cast into a `ProtocolObject<dyn UNUserNotificationCenterDelegate>` for
    /// passing to `UNUserNotificationCenter::setDelegate`.
    pub fn as_protocol(
        &self,
    ) -> Retained<ProtocolObject<dyn UNUserNotificationCenterDelegate>> {
        ProtocolObject::from_retained(unsafe {
            Retained::retain(self as *const Self as *mut Self).expect("retain self")
        })
    }
}

/// The delegate is retained by UN center for the process lifetime once
/// installed — parking a strong reference in this static ensures Rust
/// never drops it early.
static DELEGATE: OnceLock<Retained<NotifDelegate>> = OnceLock::new();

/// Install the delegate on the current notification center. Idempotent
/// per-process : the first call wins, subsequent calls with a different
/// registry are ignored.
pub fn install(registry: Registry) {
    DELEGATE.get_or_init(|| {
        let delegate = NotifDelegate::new(registry);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let proto = delegate.as_protocol();
        center.setDelegate(Some(&proto));
        delegate
    });
}

fn handle_response(registry: &Registry, response: &UNNotificationResponse) {
    let notif_id = response.notification().request().identifier().to_string();
    let action_id = response.actionIdentifier();
    let kind = classify_action(&action_id.to_string());
    if let Some((target, payload)) = registry.take_on_response(&notif_id, kind) {
        // Fire routes on target.kind → hook / url / file dispatcher.
        // Failures are logged inside `fire` via warn::emit ; no bubble
        // path here (the delegate has no useful recovery).
        let _ = notif_core::callback::fire(&target, &payload);
    }
}

fn classify_action(raw: &str) -> ResponseKind {
    // Apple defines two magic identifiers as static strings ; direct
    // pointer equality would be ideal but is fragile across dylib
    // boundaries. Compare by NSString value → Rust String.
    unsafe {
        let default_id = UNNotificationDefaultActionIdentifier.to_string();
        let dismiss_id = UNNotificationDismissActionIdentifier.to_string();
        if raw == default_id {
            return ResponseKind::Click;
        }
        if raw == dismiss_id {
            return ResponseKind::Dismiss;
        }
    }
    ResponseKind::Action(raw.to_string())
}

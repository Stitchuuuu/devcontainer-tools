//! Per-notification callback bindings kept in memory by the running
//! daemon. Delegate closures look up bindings by `notif_id`, resolve
//! which callback fires for the received response kind, and dispatch
//! via [`notif_core::callback::fire`].
//!
//! Bindings live for the lifetime of the notification in Notification
//! Center — which on macOS is indefinite until the user clicks the
//! banner, dismisses it, or the daemon exits. There is no automatic
//! eviction ; the daemon's idle-timeout is bounded by "registry is
//! empty AND no activity for N".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use notif_core::callback::{CallbackConfig, CallbackEvent, CallbackPayload, CallbackTarget};

// Note : `ResponseKind::into_event` was factored out into inline mapping
// in `take_on_response` — every mapping branch also needs a target from
// the callback config, so keeping the two arms zipped avoids a redundant
// second match.

/// One notification's callback bindings + payload snapshot the daemon
/// will hand to `fire` when a response fires.
#[derive(Debug, Clone)]
pub struct Binding {
    pub callbacks: CallbackConfig,
    /// Payload context minus the `event` field — daemon fills it in per
    /// response kind before dispatching.
    pub payload: CallbackPayload,
}

/// Which delegate-side response kind the daemon received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseKind {
    /// User clicked the notification body.
    Click,
    /// User clicked the `X` / swiped away → UN center dismissed.
    Dismiss,
    /// User clicked a custom action button. Carries the action
    /// identifier (label) as declared at
    /// `UNNotificationAction::actionWithIdentifier`.
    Action(String),
}

/// Thread-safe map from `notif_id` to [`Binding`]. Cloneable — every
/// closure that needs read access clones the `Arc`.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<HashMap<String, Binding>>>,
}

impl Registry {
    /// Fresh empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a binding under `notif_id`. Overwrites any prior entry —
    /// the daemon lets sends with duplicate `--id` win via last-writer,
    /// matching UN center's own behavior of replacing the prior banner.
    pub fn insert(&self, notif_id: String, binding: Binding) {
        self.inner.lock().unwrap().insert(notif_id, binding);
    }

    /// True iff no bindings are registered. Used by the daemon's idle
    /// timer.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    /// Look up the target + resolved payload for a fired response and
    /// **remove** the binding (bindings are single-shot from the user's
    /// POV — a click can only happen once per notification).
    ///
    /// Returns `None` when :
    /// - `notif_id` has no binding (spurious delegate call).
    /// - The response kind maps to a callback slot the user did not set
    ///   (e.g. Click fired but `--on-click` was not passed).
    /// - Action label doesn't match any registered `--on-action` entry.
    pub fn take_on_response(
        &self,
        notif_id: &str,
        kind: ResponseKind,
    ) -> Option<(CallbackTarget, CallbackPayload)> {
        let mut guard = self.inner.lock().unwrap();
        let binding = guard.get(notif_id)?.clone();
        let (target, event) = match kind {
            ResponseKind::Click => (binding.callbacks.on_click.clone()?, CallbackEvent::Click),
            ResponseKind::Dismiss => {
                (binding.callbacks.on_dismiss.clone()?, CallbackEvent::Dismiss)
            }
            ResponseKind::Action(label) => binding
                .callbacks
                .on_actions
                .iter()
                .find(|(l, _)| *l == label)
                .map(|(l, t)| (t.clone(), CallbackEvent::Action(l.clone())))?,
        };
        // Only remove on a resolved hit — an unmatched action label
        // leaves the binding in place so a subsequent click / dismiss
        // can still fire.
        guard.remove(notif_id);
        drop(guard);
        let mut payload = binding.payload;
        payload.event = event.to_wire();
        Some((target, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notif_core::callback::{CallbackKind, CallbackTarget};

    fn target(k: CallbackKind) -> CallbackTarget {
        CallbackTarget { kind: k, payload: "/tmp/x".into() }
    }

    fn payload() -> CallbackPayload {
        CallbackPayload {
            notif_id: "abc".into(),
            event: String::new(),
            sender: "default".into(),
            title: "T".into(),
            body: "B".into(),
            ts: "2026-07-06T09:00:00Z".into(),
        }
    }

    #[test]
    fn take_click_when_registered() {
        let reg = Registry::new();
        reg.insert("abc".into(), Binding {
            callbacks: CallbackConfig {
                on_click: Some(target(CallbackKind::File)),
                ..Default::default()
            },
            payload: payload(),
        });
        let (t, p) = reg.take_on_response("abc", ResponseKind::Click).unwrap();
        assert_eq!(t.kind, CallbackKind::File);
        assert_eq!(p.event, "click");
        assert!(reg.is_empty(), "hit consumed the binding");
    }

    #[test]
    fn take_click_returns_none_when_not_registered() {
        let reg = Registry::new();
        reg.insert("abc".into(), Binding {
            callbacks: CallbackConfig::default(),
            payload: payload(),
        });
        assert!(reg.take_on_response("abc", ResponseKind::Click).is_none());
    }

    #[test]
    fn take_action_matches_label() {
        let reg = Registry::new();
        reg.insert("abc".into(), Binding {
            callbacks: CallbackConfig {
                on_actions: vec![
                    ("reply".into(), target(CallbackKind::Hook)),
                    ("ignore".into(), target(CallbackKind::File)),
                ],
                ..Default::default()
            },
            payload: payload(),
        });
        let (t, p) = reg
            .take_on_response("abc", ResponseKind::Action("ignore".into()))
            .unwrap();
        assert_eq!(t.kind, CallbackKind::File);
        assert_eq!(p.event, "action:ignore");
    }

    #[test]
    fn take_action_unknown_label_returns_none_and_keeps_binding() {
        let reg = Registry::new();
        reg.insert("abc".into(), Binding {
            callbacks: CallbackConfig {
                on_click: Some(target(CallbackKind::File)),
                on_actions: vec![("reply".into(), target(CallbackKind::Hook))],
                ..Default::default()
            },
            payload: payload(),
        });
        assert!(reg
            .take_on_response("abc", ResponseKind::Action("unknown".into()))
            .is_none());
        // Binding still there — a subsequent click can still fire.
        assert!(!reg.is_empty());
        assert!(reg.take_on_response("abc", ResponseKind::Click).is_some());
    }

    #[test]
    fn take_dismiss_returns_none_when_no_on_dismiss() {
        let reg = Registry::new();
        reg.insert("abc".into(), Binding {
            callbacks: CallbackConfig {
                on_click: Some(target(CallbackKind::File)),
                ..Default::default()
            },
            payload: payload(),
        });
        assert!(reg.take_on_response("abc", ResponseKind::Dismiss).is_none());
    }

    #[test]
    fn missing_notif_id_returns_none() {
        let reg = Registry::new();
        assert!(reg.take_on_response("nope", ResponseKind::Click).is_none());
    }
}

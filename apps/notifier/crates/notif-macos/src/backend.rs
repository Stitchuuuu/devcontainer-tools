//! [`MacosBackend`] — the `impl notif_core::Backend` entry point.
//!
//! Thin adapter: routes every call to [`crate::dispatch::dispatch_outer`]
//! with empty Tier 3 overrides + empty callback config. Callers that need
//! to pass either (the CLI's `run_macos` handler in v0.2+) call
//! [`dispatch_outer`] directly instead of going through the trait.

use notif_core::callback::CallbackConfig;
use notif_core::{Backend, Notification};

use crate::dispatch::dispatch_outer;
use crate::error::MacosError;
use crate::overrides::MacosOverrides;

/// macOS backend. Constructed with no state.
#[derive(Debug, Default)]
pub struct MacosBackend;

impl Backend for MacosBackend {
    type Error = MacosError;

    fn dispatch(&self, notif: &Notification) -> Result<(), Self::Error> {
        dispatch_outer(notif, &MacosOverrides::default(), &CallbackConfig::default())
    }
}

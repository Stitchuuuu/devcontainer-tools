//! [`MacosBackend`] — the `impl notif_core::Backend` entry point.
//!
//! Thin adapter: routes every call to [`crate::dispatch::dispatch_outer`].
//! Inner-mode dispatch is entered via `open -W -a` from within
//! `dispatch_outer` and lives directly under `crate::dispatch`.

use notif_core::{Backend, Notification};

use crate::dispatch::dispatch_outer;
use crate::error::MacosError;

/// macOS backend. Constructed with no state.
#[derive(Debug, Default)]
pub struct MacosBackend;

impl Backend for MacosBackend {
    type Error = MacosError;

    fn dispatch(&self, notif: &Notification) -> Result<(), Self::Error> {
        dispatch_outer(notif)
    }
}

use notif_core::{Backend, Notification};

/// WinRT toast backend. Session 1 : dispatch only. Session 2+ will extend with
/// callback IPC + focus solver via helper types stored on this struct.
pub struct WindowsBackend {
    /// AUMID under which toasts are dispatched. Session 1 uses a Tier 1 spoof
    /// like `Microsoft.VisualStudioCode` ; session 2 replaces it with a
    /// per-sender registered AUMID materialized by `notif register`.
    pub aumid: String,
}

impl WindowsBackend {
    /// Construct a backend that dispatches under the given AUMID.
    pub fn new(aumid: impl Into<String>) -> Self {
        Self { aumid: aumid.into() }
    }
}

impl Backend for WindowsBackend {
    type Error = WindowsError;

    fn dispatch(&self, notif: &Notification) -> Result<(), Self::Error> {
        crate::dispatch::dispatch_send(notif, &self.aumid)
    }
}

/// Any failure in the Windows backend — a WinRT call that returned an HRESULT,
/// or a precondition check that failed before we even reached the WinRT layer.
#[derive(Debug)]
pub struct WindowsError {
    context: &'static str,
    source: Option<windows::core::Error>,
}

impl WindowsError {
    /// Build from a WinRT call site — `context` names the API that failed
    /// (e.g. `"ToastNotificationManager::CreateToastNotifierWithId"`) so the
    /// resulting log line points at the exact frame.
    pub fn with_context(context: &'static str, source: windows::core::Error) -> Self {
        Self { context, source: Some(source) }
    }
}

impl std::fmt::Display for WindowsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source {
            Some(e) => write!(
                f,
                "{}: HRESULT=0x{:08x} {}",
                self.context,
                e.code().0 as u32,
                e.message(),
            ),
            None => f.write_str(self.context),
        }
    }
}

impl std::error::Error for WindowsError {}

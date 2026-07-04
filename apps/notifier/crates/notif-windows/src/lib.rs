//! Windows backend stub. Real WinRT toast dispatch lands in a later session.

#[cfg(target_os = "windows")]
mod backend {
    use notif_core::{Backend, Notification};

    pub struct WindowsBackend;

    #[derive(Debug)]
    pub struct WindowsError;

    impl std::fmt::Display for WindowsError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("windows backend error")
        }
    }

    impl std::error::Error for WindowsError {}

    impl Backend for WindowsBackend {
        type Error = WindowsError;
        fn dispatch(&self, _notif: &Notification) -> Result<(), Self::Error> {
            unimplemented!("v0.3")
        }
    }
}

#[cfg(target_os = "windows")]
pub use backend::*;

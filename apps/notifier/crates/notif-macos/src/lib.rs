//! macOS backend stub. Real UN center dispatch lands in session 3.

#[cfg(target_os = "macos")]
mod backend {
    use notif_core::{Backend, Notification};

    pub struct MacosBackend;

    #[derive(Debug)]
    pub struct MacosError;

    impl std::fmt::Display for MacosError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("macos backend error")
        }
    }

    impl std::error::Error for MacosError {}

    impl Backend for MacosBackend {
        type Error = MacosError;
        fn dispatch(&self, _notif: &Notification) -> Result<(), Self::Error> {
            unimplemented!("session 3")
        }
    }
}

#[cfg(target_os = "macos")]
pub use backend::*;

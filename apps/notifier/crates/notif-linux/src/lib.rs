//! Linux backend stub. Real libnotify / D-Bus dispatch lands in a later session.

#[cfg(target_os = "linux")]
mod backend {
    use notif_core::{Backend, Notification};

    pub struct LinuxBackend;

    #[derive(Debug)]
    pub struct LinuxError;

    impl std::fmt::Display for LinuxError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("linux backend error")
        }
    }

    impl std::error::Error for LinuxError {}

    impl Backend for LinuxBackend {
        type Error = LinuxError;
        fn dispatch(&self, _notif: &Notification) -> Result<(), Self::Error> {
            unimplemented!("v0.4")
        }
    }
}

#[cfg(target_os = "linux")]
pub use backend::*;

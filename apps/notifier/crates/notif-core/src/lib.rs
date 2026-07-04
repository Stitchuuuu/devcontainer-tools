//! Shared types and the [`Backend`] trait for the `notif` CLI.
//!
//! Zero platform code, zero external dependencies. Every per-OS backend crate
//! (`notif-macos`, `notif-windows`, `notif-linux`) depends on this crate for
//! the [`Notification`] payload and the [`Backend`] contract.

use std::fmt;

/// A user-visible notification payload, backend-agnostic.
///
/// Backends translate this into their platform primitive
/// (`UNMutableNotificationContent` on macOS, `ToastNotification` on Windows,
/// `org.freedesktop.Notifications` on Linux).
#[derive(Debug, Clone)]
pub struct Notification {
    /// Bold first line of the banner.
    pub title: String,
    /// Body text under the title.
    pub body: String,
    /// Optional third line rendered between title and body on macOS.
    pub subtitle: Option<String>,
    /// Interruption level. Mapping to platform priority is per-backend and
    /// lands in session 4 (currently unread by [`notif-macos`]).
    pub priority: Priority,
    /// Sender identity: which `.app` bundle / AUMID / app_id the notification
    /// appears under.
    pub sender: Sender,
}

/// Portable interruption level. Maps to
/// `UNNotificationInterruptionLevel` on macOS,
/// `ToastScenario` on Windows, and
/// `urgency` on Linux (freedesktop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

/// Sender identity — the `.app` bundle (macOS), AUMID (Windows), or app_id
/// (Linux) under which the notification is dispatched.
///
/// The `key` is the on-disk / registry name (see [`validate_sender_key`] for
/// the accepted character set). Display name is per-platform and lives in the
/// materialized bundle metadata, not here.
#[derive(Debug, Clone)]
pub struct Sender {
    /// Filesystem-safe identifier. Validated against
    /// `^[a-z0-9_-]{1,32}$` with the reserved word `"default"`.
    pub key: String,
}

impl Sender {
    /// Build a validated sender from a key.
    ///
    /// # Errors
    /// Returns a [`SenderKeyError`] if the key is empty, longer than 32 bytes,
    /// contains disallowed characters, or is the reserved word `"default"`
    /// when `reject_default` semantics apply upstream (this call itself
    /// accepts `"default"` — use [`Sender::default`] for the reserved case
    /// and check for the reserved word explicitly in `register` handlers).
    pub fn new(key: impl Into<String>) -> Result<Self, SenderKeyError> {
        let key = key.into();
        validate_sender_key(&key)?;
        Ok(Self { key })
    }
}

impl Default for Sender {
    /// The reserved Tier 0 default sender (`key = "default"`).
    fn default() -> Self {
        Self {
            key: "default".to_string(),
        }
    }
}

/// The single contract every per-OS backend implements.
///
/// Backends are called from the outer CLI on the host OS; on macOS the same
/// binary re-enters through `open -a <.app>` and calls this trait again from
/// inside the sender's `.app` bundle (see [`notif-macos`]).
pub trait Backend {
    /// Backend-specific error type.
    type Error: std::error::Error;
    /// Dispatch a notification. Blocking.
    fn dispatch(&self, notif: &Notification) -> Result<(), Self::Error>;
}

/// Ways a sender key can fail validation.
///
/// Kept separate from per-backend error types so the CLI can print a uniform
/// message regardless of OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderKeyError {
    /// Empty string.
    Empty,
    /// Longer than 32 bytes.
    TooLong { len: usize },
    /// Contains a byte outside `[a-z0-9_-]`.
    InvalidChar { byte: u8 },
}

impl fmt::Display for SenderKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("sender key must not be empty"),
            Self::TooLong { len } => {
                write!(f, "sender key must be 1..=32 bytes (got {len})")
            }
            Self::InvalidChar { byte } => {
                write!(
                    f,
                    "sender key contains invalid byte 0x{byte:02x} (allowed: [a-z0-9_-])",
                )
            }
        }
    }
}

impl std::error::Error for SenderKeyError {}

/// Check a sender key against `^[a-z0-9_-]{1,32}$`.
///
/// The reserved word `"default"` passes this check on purpose — it is a
/// *reserved* rather than *invalid* key. Callers that must refuse it (e.g.
/// `notif register`) do so explicitly.
///
/// # Errors
/// Returns [`SenderKeyError::Empty`], [`SenderKeyError::TooLong`], or
/// [`SenderKeyError::InvalidChar`] on failure.
pub fn validate_sender_key(key: &str) -> Result<(), SenderKeyError> {
    let len = key.len();
    if len == 0 {
        return Err(SenderKeyError::Empty);
    }
    if len > 32 {
        return Err(SenderKeyError::TooLong { len });
    }
    for &b in key.as_bytes() {
        let ok = b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-';
        if !ok {
            return Err(SenderKeyError::InvalidChar { byte: b });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys_pass() {
        for k in [
            "default", "a", "abc", "foo_bar", "foo-bar", "abc123", "a".repeat(32).as_str(),
        ] {
            assert!(validate_sender_key(k).is_ok(), "expected ok for {k:?}");
        }
    }

    #[test]
    fn empty_rejected() {
        assert_eq!(validate_sender_key(""), Err(SenderKeyError::Empty));
    }

    #[test]
    fn too_long_rejected() {
        let k = "a".repeat(33);
        assert_eq!(validate_sender_key(&k), Err(SenderKeyError::TooLong { len: 33 }));
    }

    #[test]
    fn uppercase_rejected() {
        assert_eq!(
            validate_sender_key("FOO"),
            Err(SenderKeyError::InvalidChar { byte: b'F' }),
        );
    }

    #[test]
    fn symbol_rejected() {
        assert_eq!(
            validate_sender_key("a.b"),
            Err(SenderKeyError::InvalidChar { byte: b'.' }),
        );
    }

    #[test]
    fn default_is_reserved_word_not_default() {
        assert_eq!(Sender::default().key, "default");
        assert!(Sender::new("default").is_ok());
    }

    #[test]
    fn sender_new_validates() {
        assert!(Sender::new("valid_key-1").is_ok());
        assert!(Sender::new("").is_err());
        assert!(Sender::new("Bad").is_err());
    }
}

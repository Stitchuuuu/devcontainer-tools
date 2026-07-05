//! Shared types and the [`Backend`] trait for the `notif` CLI.
//!
//! Zero platform code, zero external dependencies. Every per-OS backend crate
//! (`notif-macos`, `notif-windows`, `notif-linux`) depends on this crate for
//! the [`Notification`] payload and the [`Backend`] contract.

use std::fmt;
use std::path::PathBuf;

pub mod warn;

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
    /// Interruption level. Maps to `UNNotificationInterruptionLevel` on macOS
    /// (wired session 4), `ToastScenario` on Windows, `urgency` on Linux.
    pub priority: Priority,
    /// Sender identity: which `.app` bundle / AUMID / app_id the notification
    /// appears under.
    pub sender: Sender,
    /// Per-notification identifier. Backends use it as
    /// `UNNotificationRequest.identifier` (macOS) or an equivalent on other
    /// platforms. Absent → backend mints a random one.
    pub id: Option<String>,
    /// Sound to play on delivery. Backend maps to the platform primitive
    /// (macOS: `UNNotificationSound`; Windows: `<audio src="…"/>`; Linux:
    /// `sound-name` hint).
    pub sound: Option<Sound>,
    /// Path to an inline image / attachment. Validated at CLI parse (file
    /// exists, extension in `.png` / `.jpg` / `.gif`).
    pub image: Option<PathBuf>,
    /// Behavior when the banner is auto-dismissed by the OS. macOS has no
    /// native equivalent in v0.1 (backend emits an `info:` line and drops
    /// the flag); Windows / Linux honor it in later versions.
    pub on_timeout: Option<TimeoutBehavior>,
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

impl Priority {
    /// Numeric level matching Apple's `UNNotificationInterruptionLevel`
    /// (`Passive=0`, `Active=1`, `TimeSensitive=2`, `Critical=3`). The macOS
    /// backend uses this ordering directly; Windows / Linux remap.
    #[must_use]
    pub const fn level(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }

    /// Wire-format string used to serialize `--priority` on the outer→inner
    /// CLI hop (macOS re-executes into the bundled `notif`).
    #[must_use]
    pub const fn wire_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Portable sound selector. Maps to `UNNotificationSound` on macOS —
/// `Default` → `defaultSound()`, `Alert` → `defaultCriticalSound()`,
/// `Custom(s)` → bundled sound name if `s` looks like a bare name,
/// or a `soundURL` if `s` looks like a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sound {
    Default,
    Alert,
    Custom(String),
}

impl Sound {
    /// Wire-format string used to serialize `--sound` on the outer→inner
    /// CLI hop. `Default` / `Alert` round-trip through the keyword; `Custom`
    /// round-trips through the raw value.
    #[must_use]
    pub fn wire_str(&self) -> &str {
        match self {
            Self::Default => "default",
            Self::Alert => "alert",
            Self::Custom(v) => v.as_str(),
        }
    }
}

/// What to do with a notification when the OS auto-dismisses it. macOS in
/// v0.1 has no native equivalent (the OS decides for us); Windows /
/// Linux backends honor it in v0.3+.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutBehavior {
    LogOnly,
    Dismiss,
    Persist,
}

impl TimeoutBehavior {
    /// Wire-format string used to serialize `--on-timeout` on the outer→inner
    /// CLI hop.
    #[must_use]
    pub const fn wire_str(self) -> &'static str {
        match self {
            Self::LogOnly => "log-only",
            Self::Dismiss => "dismiss",
            Self::Persist => "persist",
        }
    }
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

    #[test]
    fn priority_level_table() {
        // Contract with `UNNotificationInterruptionLevel` — Apple docs pin
        // Passive=0, Active=1, TimeSensitive=2, Critical=3.
        assert_eq!(Priority::Low.level(), 0);
        assert_eq!(Priority::Normal.level(), 1);
        assert_eq!(Priority::High.level(), 2);
        assert_eq!(Priority::Critical.level(), 3);
    }

    #[test]
    fn priority_wire_str_table() {
        assert_eq!(Priority::Low.wire_str(), "low");
        assert_eq!(Priority::Normal.wire_str(), "normal");
        assert_eq!(Priority::High.wire_str(), "high");
        assert_eq!(Priority::Critical.wire_str(), "critical");
    }

    #[test]
    fn sound_wire_str() {
        assert_eq!(Sound::Default.wire_str(), "default");
        assert_eq!(Sound::Alert.wire_str(), "alert");
        assert_eq!(Sound::Custom("Ping".into()).wire_str(), "Ping");
        assert_eq!(
            Sound::Custom("/System/Library/Sounds/Glass.aiff".into()).wire_str(),
            "/System/Library/Sounds/Glass.aiff",
        );
    }

    #[test]
    fn timeout_wire_str() {
        assert_eq!(TimeoutBehavior::LogOnly.wire_str(), "log-only");
        assert_eq!(TimeoutBehavior::Dismiss.wire_str(), "dismiss");
        assert_eq!(TimeoutBehavior::Persist.wire_str(), "persist");
    }
}

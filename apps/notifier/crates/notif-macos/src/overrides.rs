//! Tier 3 — raw macOS override struct.
//!
//! Escape-hatch flags that map 1:1 to `UNMutableNotificationContent`
//! properties without portable equivalents. When a native override is set
//! alongside its portable counterpart (`--macos-sound-name` + `--sound`,
//! `--macos-attachment` + `--image`, `--macos-interruption-level` +
//! `--priority`), the native wins with a per-category `info:` log line via
//! [`notif_core::warn::info`] — see ROLLOUT decision "portable vs native
//! flag conflict — native wins with `info:` log (not warning)".
//!
//! The struct lives in `notif-macos` because every field maps to a macOS
//! primitive with no portable meaning. Windows / Linux backends carry
//! their own analogous override structs in v0.3 / v0.4.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Raw macOS override flags accepted by `notif send`. Every field is
/// `Option<T>` — absence = "use the portable spec (or the OS default if
/// no portable spec was passed)".
///
/// `Serialize`/`Deserialize` are on the struct so the daemon socket
/// protocol ([`crate::daemon::proto`]) can wire it over the socket
/// without a separate DTO layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacosOverrides {
    /// `--macos-sound-name <name>` → `UNNotificationSound::soundNamed(name)`.
    /// Overrides `--sound` when both are set.
    pub sound_name: Option<String>,
    /// `--macos-attachment <path>` → appended to
    /// `content.setAttachments(...)`. Overrides `--image`.
    pub attachment: Option<PathBuf>,
    /// `--macos-interruption-level <passive|active|timeSensitive|critical>`
    /// → `content.setInterruptionLevel(...)`. Overrides `--priority`.
    pub interruption_level: Option<InterruptionLevel>,
    /// `--macos-thread-identifier <str>` → `content.setThreadIdentifier(...)`.
    /// No portable counterpart.
    pub thread_identifier: Option<String>,
    /// `--macos-category-identifier <str>` → `content.setCategoryIdentifier(...)`.
    /// No portable counterpart.
    pub category_identifier: Option<String>,
}

impl MacosOverrides {
    /// True iff no override slot is set. Fast path skip for the
    /// portable-vs-native conflict log lines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sound_name.is_none()
            && self.attachment.is_none()
            && self.interruption_level.is_none()
            && self.thread_identifier.is_none()
            && self.category_identifier.is_none()
    }
}

/// The four `UNNotificationInterruptionLevel` values. Wire form uses
/// Apple's raw camelCase (`passive`, `active`, `timeSensitive`, `critical`)
/// so the CLI flag round-trips through the outer→inner hop without
/// mapping tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterruptionLevel {
    Passive,
    Active,
    TimeSensitive,
    Critical,
}

impl InterruptionLevel {
    /// Wire-format string matching Apple's docs. Case-preserving.
    #[must_use]
    pub const fn wire_str(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Active => "active",
            Self::TimeSensitive => "timeSensitive",
            Self::Critical => "critical",
        }
    }

    /// Parse the CLI value. Accepts the exact wire form only —
    /// mis-cased inputs are rejected so callers don't drift the wire
    /// format silently.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "passive" => Some(Self::Passive),
            "active" => Some(Self::Active),
            "timeSensitive" => Some(Self::TimeSensitive),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert!(MacosOverrides::default().is_empty());
    }

    #[test]
    fn any_slot_set_makes_non_empty() {
        for o in [
            MacosOverrides { sound_name: Some("Ping".into()), ..Default::default() },
            MacosOverrides { attachment: Some(PathBuf::from("/tmp/x.png")), ..Default::default() },
            MacosOverrides {
                interruption_level: Some(InterruptionLevel::Critical),
                ..Default::default()
            },
            MacosOverrides { thread_identifier: Some("t".into()), ..Default::default() },
            MacosOverrides { category_identifier: Some("c".into()), ..Default::default() },
        ] {
            assert!(!o.is_empty(), "{o:?}");
        }
    }

    #[test]
    fn interruption_wire_str_roundtrip() {
        for v in [
            InterruptionLevel::Passive,
            InterruptionLevel::Active,
            InterruptionLevel::TimeSensitive,
            InterruptionLevel::Critical,
        ] {
            let s = v.wire_str();
            assert_eq!(InterruptionLevel::parse(s), Some(v), "wire roundtrip for {v:?}");
        }
    }

    #[test]
    fn interruption_parse_rejects_wrong_case() {
        assert_eq!(InterruptionLevel::parse("Critical"), None);
        assert_eq!(InterruptionLevel::parse("time_sensitive"), None);
        assert_eq!(InterruptionLevel::parse("urgent"), None);
        assert_eq!(InterruptionLevel::parse(""), None);
    }
}

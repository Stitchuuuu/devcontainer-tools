use notif_core::Priority;

/// Resolve the WinRT `<toast scenario="…">` attribute for a portable priority.
///
/// - `High` / `Critical` → `Some("urgent")` by default (breakthrough audio,
///   sticks at the top of Action Center). Overridable via the env
///   `NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO=urgent|reminder|default` — the
///   `default` value maps to `None` (plain toast, no scenario attr).
/// - `Low` / `Normal` → `None` (plain toast).
///
/// The env is read on every call so tests + the CLI can toggle it per-invocation.
pub fn resolve_scenario(priority: Priority) -> Option<&'static str> {
    match priority {
        Priority::High | Priority::Critical => resolve_urgent_override(),
        Priority::Low | Priority::Normal => None,
    }
}

fn resolve_urgent_override() -> Option<&'static str> {
    let raw = std::env::var("NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO").ok();
    match raw.as_deref().map(str::trim) {
        Some("default") => None,
        Some("reminder") => Some("reminder"),
        Some("urgent") => Some("urgent"),
        // Empty string, unknown value, or unset → keep the baseline.
        _ => Some("urgent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_env() {
        std::env::remove_var("NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO");
    }

    #[test]
    fn low_and_normal_never_have_scenario() {
        clear_env();
        assert_eq!(resolve_scenario(Priority::Low), None);
        assert_eq!(resolve_scenario(Priority::Normal), None);
    }

    #[test]
    fn high_defaults_to_urgent() {
        clear_env();
        assert_eq!(resolve_scenario(Priority::High), Some("urgent"));
        assert_eq!(resolve_scenario(Priority::Critical), Some("urgent"));
    }

    #[test]
    fn override_default_yields_plain_toast() {
        std::env::set_var("NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO", "default");
        assert_eq!(resolve_scenario(Priority::High), None);
        clear_env();
    }

    #[test]
    fn override_reminder_selected() {
        std::env::set_var("NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO", "reminder");
        assert_eq!(resolve_scenario(Priority::High), Some("reminder"));
        clear_env();
    }

    #[test]
    fn override_unknown_falls_back_to_urgent() {
        std::env::set_var("NOTIF_WINDOWS_PRIORITY_HIGH_SCENARIO", "banana");
        assert_eq!(resolve_scenario(Priority::High), Some("urgent"));
        clear_env();
    }
}

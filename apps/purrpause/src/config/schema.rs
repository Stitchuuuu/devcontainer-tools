// Config schema — serialized to TOML then DPAPI-machine encrypted into
// state.dat. All user-facing strings are configurable ; every field has
// a default so a missing/corrupt file never blocks startup.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::defaults::d;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    // --- Timings ---
    #[serde(default = "d::interval_hours")]
    pub interval_hours: f32,

    #[serde(default = "d::duration_minutes")]
    pub duration_minutes: u32,

    #[serde(default = "d::pre_notification_minutes")]
    pub pre_notification_minutes: Vec<u32>,

    // --- Animations ---
    #[serde(default)]
    pub animations: Vec<AnimationEntry>,

    #[serde(default = "d::rotation_mode")]
    pub rotation_mode: RotationMode,

    // --- User-facing strings ---
    #[serde(default = "d::popup_title")]
    pub popup_title: String,

    #[serde(default = "d::popup_subtitle")]
    pub popup_subtitle: String,

    #[serde(default = "d::dismiss_button_label")]
    pub dismiss_button_label: String,

    /// Placeholders : `{mm}`, `{ss}`, `{total_min}`.
    #[serde(default = "d::countdown_template")]
    pub countdown_template: String,

    /// Key = palier in minutes (15/10/5), value = user-visible string.
    #[serde(default = "d::pre_notif_messages")]
    pub pre_notif_messages: HashMap<u32, String>,

    #[serde(default = "d::wizard_welcome")]
    pub wizard_welcome: String,

    #[serde(default = "d::wizard_password_hint")]
    pub wizard_password_hint: String,

    // --- Security + state ---
    #[serde(default = "d::passcode_length")]
    pub passcode_length: u32,

    /// Argon2 hash of the numeric passcode. Empty string until the
    /// first-run wizard writes one — never plaintext on disk.
    #[serde(default)]
    pub passcode_hash: String,

    #[serde(default)]
    pub disabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        // Deserialize from an empty TOML doc so every #[serde(default)]
        // fires — single source of truth for defaults.
        toml::from_str("").expect("empty toml deserializes to Config default")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationEntry {
    /// Relative path under `<exe_dir>/animations/`.
    pub file: String,
    pub enabled: bool,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationMode {
    Random,
    Sequential,
}

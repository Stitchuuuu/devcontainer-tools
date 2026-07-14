// Hardcoded defaults for every user-configurable field of the Config
// schema. If state.dat is missing / corrupted / older-schema, the loader
// falls back to these values via `#[serde(default = "d::…")]` per field.
//
// All French — v1 is French-only per the design constitution.

use std::collections::HashMap;

use super::schema::RotationMode;

pub const DEFAULT_POPUP_WINDOW_TITLE: &str = "Windows Session Health";
pub const DEFAULT_POPUP_TITLE: &str = "C'est l'heure de la pause !";
pub const DEFAULT_POPUP_SUBTITLE: &str =
    "Prends 5 minutes pour t'étirer, boire, regarder au loin.";
pub const DEFAULT_DISMISS_LABEL: &str = "J'ai fait ma pause";
pub const DEFAULT_COUNTDOWN_TEMPLATE: &str = "Pause dans {mm}:{ss}";
pub const DEFAULT_WIZARD_WELCOME: &str =
    "Bienvenue. Configure les rappels de pause avant de continuer.";
pub const DEFAULT_WIZARD_PASSWORD_HINT: &str =
    "Choisis un code numérique entre 4 et 12 chiffres. Il sera demandé pour modifier ou désinstaller.";

pub mod d {
    use super::*;

    pub fn interval_hours() -> f32 {
        2.0
    }
    pub fn duration_minutes() -> u32 {
        5
    }
    pub fn pre_notification_minutes() -> Vec<u32> {
        vec![15, 10, 5, 1]
    }
    pub fn rotation_mode() -> RotationMode {
        RotationMode::Random
    }
    pub fn popup_window_title() -> String {
        DEFAULT_POPUP_WINDOW_TITLE.to_string()
    }
    pub fn popup_title() -> String {
        DEFAULT_POPUP_TITLE.to_string()
    }
    pub fn popup_subtitle() -> String {
        DEFAULT_POPUP_SUBTITLE.to_string()
    }
    pub fn dismiss_button_label() -> String {
        DEFAULT_DISMISS_LABEL.to_string()
    }
    pub fn countdown_template() -> String {
        DEFAULT_COUNTDOWN_TEMPLATE.to_string()
    }
    pub fn pre_notif_messages() -> HashMap<u32, String> {
        let mut m = HashMap::new();
        m.insert(15, "Prochaine pause dans 15 min".to_string());
        m.insert(10, "Plus que 10 min avant la pause".to_string());
        m.insert(5, "Pause dans 5 min !".to_string());
        m.insert(1, "Pause dans 1 min !".to_string());
        m
    }
    pub fn wizard_welcome() -> String {
        DEFAULT_WIZARD_WELCOME.to_string()
    }
    pub fn wizard_password_hint() -> String {
        DEFAULT_WIZARD_PASSWORD_HINT.to_string()
    }
    pub fn passcode_length() -> u32 {
        6
    }
}

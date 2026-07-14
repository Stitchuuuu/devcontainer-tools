pub mod crypto;
pub mod defaults;
pub mod schema;

#[allow(unused_imports)] // AnimationEntry + RotationMode are consumed by the popup / config UI wired later
pub use schema::{AnimationEntry, Config, RotationMode};

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Read state.dat → DPAPI decrypt → toml deserialize. Any error path
/// falls back to `Config::default()` with a `tracing::warn!` log — the
/// popup / config UI can never fail to start on a corrupt file, per
/// design § "Fallback config".
pub fn load_or_default(path: &Path) -> Config {
    match load(path) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "config load failed, using defaults");
            Config::default()
        }
    }
}

/// Fallible variant — separates the "file exists but corrupt" branch from
/// the caller's fallback logic. Used by tests.
pub fn load(path: &Path) -> Result<Config> {
    let ciphertext = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let plaintext = crypto::decrypt(&ciphertext).context("dpapi decrypt")?;
    let text = std::str::from_utf8(&plaintext).context("state.dat is not utf-8")?;
    let cfg: Config = toml::from_str(text).context("toml deserialize")?;
    Ok(cfg)
}

/// Serialize → DPAPI encrypt → atomic write via `<path>.tmp` + rename.
pub fn save(cfg: &Config, path: &Path) -> Result<()> {
    let text = toml::to_string(cfg).context("toml serialize")?;
    let ciphertext = crypto::encrypt(text.as_bytes()).context("dpapi encrypt")?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    let tmp = path.with_extension("dat.tmp");
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(&ciphertext)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_populates_every_field() {
        let cfg = Config::default();
        assert_eq!(cfg.interval_hours, 2.0);
        assert_eq!(cfg.duration_minutes, 5);
        assert_eq!(cfg.pre_notification_minutes, vec![15, 10, 5]);
        assert_eq!(cfg.rotation_mode, RotationMode::Random);
        assert_eq!(cfg.passcode_length, 6);
        assert_eq!(cfg.passcode_hash, "");
        assert!(!cfg.disabled);
        assert!(cfg.pre_notif_messages.contains_key(&15));
        assert!(cfg.pre_notif_messages.contains_key(&10));
        assert!(cfg.pre_notif_messages.contains_key(&5));
    }

    #[test]
    fn roundtrip_through_toml() {
        let mut cfg = Config::default();
        cfg.interval_hours = 1.5;
        cfg.passcode_hash = "$argon2id$v=19$m=19456,t=2,p=1$…".to_string();
        cfg.animations.push(AnimationEntry {
            file: "dance-cat.lottie".to_string(),
            enabled: true,
            display_name: "Chat qui danse".to_string(),
        });

        let text = toml::to_string(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // Only interval_hours set — everything else defaults.
        let partial = "interval_hours = 3.0\n";
        let cfg: Config = toml::from_str(partial).unwrap();
        assert_eq!(cfg.interval_hours, 3.0);
        assert_eq!(cfg.duration_minutes, 5);
        assert_eq!(cfg.passcode_length, 6);
    }

    #[test]
    fn empty_toml_deserializes_to_default() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn save_then_load_full_pipeline() {
        // On Linux the crypto layer is identity, so this exercises
        // serde + file IO end-to-end. On a Windows host, DPAPI joins.
        let tmp = tempdir();
        let path = tmp.join("state.dat");

        let mut cfg = Config::default();
        cfg.popup_title = "Test 123".to_string();
        cfg.passcode_length = 8;

        save(&cfg, &path).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn load_or_default_returns_default_on_missing_file() {
        let cfg = load_or_default(Path::new("/nonexistent/state.dat"));
        assert_eq!(cfg, Config::default());
    }

    fn tempdir() -> std::path::PathBuf {
        use rand::Rng;
        let n: u64 = rand::rng().random();
        let p = std::env::temp_dir().join(format!("purrpause-test-{n}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}

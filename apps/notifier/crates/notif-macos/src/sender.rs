//! Sender-key → `.app` bundle path resolution.
//!
//! The default sender is `key = "default"`; user-registered senders use the
//! validated key syntax from [`notif_core::validate_sender_key`].
//!
//! Bundle root : `$HOME/.local/share/notif/senders/<key>.app`. This deviates
//! from Apple's `Application Support` convention on purpose — the decision
//! locked at ROLLOUT time (see plans/notif-cli/ROLLOUT.md §Decisions) picks a
//! path symmetric with the (future) Linux / Windows backends.

use std::path::PathBuf;

use crate::error::MacosError;

/// Directory containing every materialized sender bundle for the current
/// user : `$HOME/.local/share/notif/senders`.
///
/// # Errors
/// Returns [`MacosError::NoHome`] if `$HOME` is unset (should not happen on a
/// running Mac session).
pub fn senders_root() -> Result<PathBuf, MacosError> {
    let home = std::env::var_os("HOME").ok_or(MacosError::NoHome)?;
    Ok(PathBuf::from(home).join(".local/share/notif/senders"))
}

/// Fallback path for a bundle we have NOT yet materialized.
///
/// Used by callers that need a stable path before the bundle exists (e.g. the
/// first-run auto-setup on `send`). For actual lookup of an existing bundle
/// on disk, prefer [`find_bundle_by_key`] which scans `senders/*.app` by the
/// custom `NotifSenderKey` Info.plist marker — resilient to bundle folder
/// rename (register writes `<display>.app`, not `<key>.app`).
///
/// # Errors
/// Returns [`MacosError::NoHome`] if `$HOME` is unset.
pub fn bundle_path_for(key: &str) -> Result<PathBuf, MacosError> {
    if let Some(p) = find_bundle_by_key(key)? {
        return Ok(p);
    }
    let dirname = if key == DEFAULT_KEY {
        DEFAULT_DISPLAY_NAME
    } else {
        key
    };
    Ok(senders_root()?.join(format!("{dirname}.app")))
}

/// Scan `~/.local/share/notif/senders/*.app` for a bundle whose Info.plist
/// carries `NotifSenderKey == key`, and return its path.
///
/// Returns `None` if the senders directory does not exist or no bundle
/// matches. Ignores IO errors on individual entries (missing / unreadable
/// Info.plist).
///
/// # Errors
/// Only bubbles [`MacosError::NoHome`] from [`senders_root`].
pub fn find_bundle_by_key(key: &str) -> Result<Option<PathBuf>, MacosError> {
    let root = senders_root()?;
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("app") {
            continue;
        }
        let plist_path = path.join("Contents/Info.plist");
        if !plist_path.exists() {
            continue;
        }
        let dict: plist::Dictionary = match plist::from_file(&plist_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if dict.get("NotifSenderKey").and_then(|v| v.as_string()) == Some(key) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Display name for the Tier 0 default sender.
///
/// Doubles as the `.app` folder name (macOS System Settings > Notifications
/// reads that, not `CFBundleName`) so users see "Notify" in the permissions
/// panel and banners. Real project-scoped icon lands in session 6.
pub const DEFAULT_DISPLAY_NAME: &str = "Notify";

/// Reserved sender key.
pub const DEFAULT_KEY: &str = "default";

/// Metadata resolved from a real installed macOS app via Spotlight.
#[derive(Debug, Clone)]
pub struct ResolvedApp {
    /// Absolute path to the `.app` bundle (e.g.
    /// `/Applications/Visual Studio Code.app`).
    pub path: PathBuf,
    /// `CFBundleIdentifier` — the value we spoof.
    pub identifier: String,
    /// `CFBundleDisplayName` (falls back to `CFBundleName`).
    pub display_name: String,
    /// Absolute path to the app's `.icns` icon file, if declared.
    pub icon_path: Option<PathBuf>,
}

/// Resolve an app hint (either a `CFBundleIdentifier` like `com.microsoft.VSCode`
/// or a display name like `Visual Studio Code`) to its installed
/// [`ResolvedApp`] metadata.
///
/// Uses Spotlight (`mdfind`) — index-only, so an unindexed app (recently
/// installed to a non-standard location) may not be found even if it exists.
///
/// # Errors
/// - [`MacosError::Io`] if `mdfind` fails to spawn.
/// - [`MacosError::Objc`] if no app matches the hint (reused for the "user
///   input rejected by the system" surface).
/// - [`MacosError::Plist`] if the matched app's Info.plist is unreadable.
pub fn resolve_app_hint(hint: &str) -> Result<ResolvedApp, MacosError> {
    let path = find_app_path(hint)?;
    let plist_path = path.join("Contents/Info.plist");
    let dict: plist::Dictionary = plist::from_file(&plist_path)?;

    let identifier = dict
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .ok_or_else(|| MacosError::Objc(format!("{path:?} missing CFBundleIdentifier")))?
        .to_string();

    // Prefer the `.app` folder stem — it's what macOS Finder / Dock / Alt-Tab
    // display, so it's the name users recognize. Falls back to
    // `CFBundleDisplayName` then `CFBundleName` only when the folder stem is
    // unusable. VS Code specifically ships `CFBundleName = "Code"` (short
    // marketing) while the folder is `Visual Studio Code.app` — folder-first
    // makes the borrowed display match what the user typed / sees.
    let folder_stem = path.file_stem().and_then(|s| s.to_str());
    let display_name = folder_stem
        .map(str::to_string)
        .or_else(|| {
            dict.get("CFBundleDisplayName")
                .and_then(|v| v.as_string())
                .map(str::to_string)
        })
        .or_else(|| {
            dict.get("CFBundleName")
                .and_then(|v| v.as_string())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "app".to_string());

    // Icon lookup: prefer CFBundleIconFile → same-name .icns in Resources.
    // Fallback: scan `Contents/Resources` for any `.icns` file (VS Code
    // ships `Code.icns` without declaring CFBundleIconFile in its Info.plist
    // for the "Code" build; other apps have similar quirks).
    let resources = path.join("Contents/Resources");
    let icon_path = dict
        .get("CFBundleIconFile")
        .and_then(|v| v.as_string())
        .map(|name| {
            let mut candidate = resources.join(name);
            if candidate.extension().is_none() {
                candidate.set_extension("icns");
            }
            candidate
        })
        .filter(|p| p.exists())
        .or_else(|| scan_icns_in(&resources));

    Ok(ResolvedApp {
        path,
        identifier,
        display_name,
        icon_path,
    })
}

/// Snapshot of a materialized sender bundle. Populated by [`list_senders`]
/// for CLI listing.
#[derive(Debug, Clone)]
pub struct SenderSummary {
    /// Value of the custom `NotifSenderKey` marker in the bundle's Info.plist.
    pub key: String,
    /// `CFBundleName`.
    pub display: String,
    /// `CFBundleIdentifier`.
    pub identifier: String,
    /// `.app` folder basename (e.g. `"Notify.app"`).
    pub folder: String,
}

/// Enumerate every materialized sender bundle under [`senders_root`].
///
/// Reads each `Contents/Info.plist`. Skips entries whose plist is missing
/// or unreadable (best-effort — a partial listing is more useful than an
/// error).
///
/// # Errors
/// Only bubbles [`MacosError::NoHome`] from [`senders_root`].
pub fn list_senders() -> Result<Vec<SenderSummary>, MacosError> {
    let root = senders_root()?;
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("app") {
            continue;
        }
        let plist_path = path.join("Contents/Info.plist");
        let dict: plist::Dictionary = match plist::from_file(&plist_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let key = dict.get("NotifSenderKey").and_then(|v| v.as_string()).unwrap_or("?").to_string();
        let display = dict.get("CFBundleName").and_then(|v| v.as_string()).unwrap_or("?").to_string();
        let identifier = dict.get("CFBundleIdentifier").and_then(|v| v.as_string()).unwrap_or("?").to_string();
        let folder = path.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
        out.push(SenderSummary { key, display, identifier, folder });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// Scan a directory for the first `.icns` file and return its path.
/// Fallback for apps that don't declare `CFBundleIconFile` in their
/// Info.plist even though a `.icns` sits in `Contents/Resources`.
fn scan_icns_in(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("icns") {
            Some(path)
        } else {
            None
        }
    })
}

/// Two-stage lookup — Spotlight first (fast), fallback to scanning the
/// standard `/Applications` roots. Robust when the metadata attribute we
/// query is not indexed for the target app.
fn find_app_path(hint: &str) -> Result<PathBuf, MacosError> {
    let looks_like_id = hint.contains('.') && !hint.contains(' ');

    // Spotlight queries — try both identifier and filesystem-name matches.
    let queries: [String; 2] = if looks_like_id {
        [
            format!("kMDItemCFBundleIdentifier == '{hint}'"),
            format!("kMDItemFSName == '{hint}.app'"),
        ]
    } else {
        [
            format!("kMDItemFSName == '{hint}.app'"),
            format!("kMDItemCFBundleIdentifier == '{hint}'"),
        ]
    };
    for q in queries.iter() {
        let out = std::process::Command::new("mdfind").arg(q).output()?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if let Some(first) = stdout
            .lines()
            .map(str::trim)
            .find(|s| !s.is_empty() && s.ends_with(".app"))
        {
            return Ok(PathBuf::from(first));
        }
    }

    // Fallback — direct scan of the standard app roots (unindexed installs).
    let home = std::env::var_os("HOME").ok_or(MacosError::NoHome)?;
    let roots = [
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from(&home).join("Applications"),
    ];
    for root in roots.iter() {
        if let Some(m) = scan_root_for_app(root, hint, looks_like_id) {
            return Ok(m);
        }
    }

    Err(MacosError::Objc(format!(
        "no installed app matches {hint:?} (searched Spotlight + /Applications, /System/Applications, ~/Applications)"
    )))
}

fn scan_root_for_app(root: &std::path::Path, hint: &str, by_id: bool) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let target_fsname = format!("{hint}.app");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("app") {
            continue;
        }
        // Filesystem-name match — cheapest.
        if path.file_name().and_then(|s| s.to_str()) == Some(&target_fsname) {
            return Some(path);
        }
        // Info.plist match — checks identifier + display name.
        let plist_path = path.join("Contents/Info.plist");
        let dict: plist::Dictionary = match plist::from_file(&plist_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if by_id {
            if dict.get("CFBundleIdentifier").and_then(|v| v.as_string()) == Some(hint) {
                return Some(path);
            }
        } else {
            let matches_display = dict
                .get("CFBundleDisplayName")
                .and_then(|v| v.as_string())
                == Some(hint)
                || dict.get("CFBundleName").and_then(|v| v.as_string()) == Some(hint);
            if matches_display {
                return Some(path);
            }
        }
    }
    None
}

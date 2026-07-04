//! `.app` bundle materialization for a given sender key.
//!
//! Layout written on disk :
//!
//! ```text
//! $HOME/.local/share/notif/senders/<key>.app/
//!     Contents/
//!         Info.plist                (XML)
//!         MacOS/
//!             notif                 (copy of self, mode 0755)
//!         Resources/
//!             code.icns             (only for the Tier 0 default sender)
//! ```
//!
//! The bundle is materialized on-demand from
//! [`crate::dispatch::dispatch_outer`] before `open -W -a` fires — see also
//! the two-mode architecture note in the crate root docs.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::MacosError;
use crate::sender::{DEFAULT_DISPLAY_NAME, DEFAULT_KEY};

/// Icon embedded for the default Tier 0 sender when the caller does not
/// provide an override.
///
/// Currently the OSS `code.icns` from
/// `github.com/microsoft/vscode/resources/darwin/`. Session 6 replaces with a
/// project-scoped icon; a user can also override per-sender at `register`
/// time via `--icon <path>` (see [`ensure_bundle`]).
const DEFAULT_ICON: &[u8] = include_bytes!("../assets/code.icns");

/// `Info.plist` payload serialized via `plist::to_writer_xml`.
///
/// Field renames follow Apple's PascalCase-with-prefix convention
/// (`CFBundle*`, `LS*`).
#[derive(serde::Serialize)]
struct InfoPlist<'a> {
    #[serde(rename = "CFBundleName")]
    bundle_name: &'a str,
    #[serde(rename = "CFBundleDisplayName")]
    bundle_display_name: &'a str,
    #[serde(rename = "CFBundleIdentifier")]
    bundle_identifier: String,
    #[serde(rename = "CFBundleExecutable")]
    bundle_executable: &'a str,
    #[serde(rename = "CFBundleShortVersionString")]
    bundle_short_version_string: &'a str,
    #[serde(rename = "CFBundleVersion")]
    bundle_version: &'a str,
    #[serde(rename = "CFBundlePackageType")]
    bundle_package_type: &'a str,
    #[serde(rename = "CFBundleInfoDictionaryVersion")]
    bundle_info_dict_version: &'a str,
    #[serde(rename = "CFBundleIconFile", skip_serializing_if = "Option::is_none")]
    bundle_icon_file: Option<&'a str>,
    #[serde(rename = "LSUIElement")]
    ls_ui_element: bool,
    #[serde(rename = "LSMinimumSystemVersion")]
    ls_minimum_system_version: &'a str,
    /// Custom key — our own metadata, ignored by macOS. Lets
    /// [`crate::sender::find_bundle_by_key`] scan `senders/*.app` and match
    /// even when the `.app` folder was renamed to the display name.
    #[serde(rename = "NotifSenderKey")]
    notif_sender_key: &'a str,
}

/// Materialize a `.app` bundle for `key` if it does not already exist, or
/// verify the existing one matches the requested display name.
///
/// Returns the absolute path to the `.app` directory.
///
/// The `display` argument is the `CFBundleName` / `CFBundleDisplayName` that
/// macOS shows in notification banners. For the default key it is overridden
/// to [`DEFAULT_DISPLAY_NAME`] regardless of the caller's argument.
///
/// `icon_override` — optional raw `.icns` bytes to write into
/// `Contents/Resources/icon.icns`. If `None`, the default Tier 0 sender
/// falls back to the embedded [`DEFAULT_ICON`]; Tier 2 senders without an
/// override get no icon (macOS generic).
///
/// # Errors
/// - [`MacosError::NoHome`] if `$HOME` is unset.
/// - [`MacosError::Io`] on any filesystem operation failure.
/// - [`MacosError::Plist`] if the Info.plist cannot be serialized.
/// - [`MacosError::BundleConflict`] if the bundle exists with a different
///   `CFBundleName` than requested (Tier 2 `register` conflict).
pub fn ensure_bundle(
    key: &str,
    display: &str,
    icon_override: Option<&[u8]>,
    identifier_override: Option<&str>,
) -> Result<PathBuf, MacosError> {
    // Look for an already-materialized bundle by NotifSenderKey scan.
    // Handles the case where the folder was renamed (register writes
    // `<display>.app`, not `<key>.app`).
    let existing = crate::sender::find_bundle_by_key(key)?;

    // Compute effective display + bundle path for a *new* bundle.
    // Default sender always resolves to the constant "Notify"; custom
    // senders use the caller-provided display verbatim (the CLI layer is
    // responsible for appending the ` · Notify` distinguishing suffix when
    // the sender is a cosmetic clone of an installed app).
    let effective_display = if key == DEFAULT_KEY {
        DEFAULT_DISPLAY_NAME.to_string()
    } else {
        display.to_string()
    };

    let bundle = match existing {
        Some(p) => p,
        None => crate::sender::senders_root()?.join(format!("{effective_display}.app")),
    };
    let contents = bundle.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources_dir = contents.join("Resources");
    let plist_path = contents.join("Info.plist");
    let exe_path = macos_dir.join("notif");

    let is_new = !plist_path.exists() || !exe_path.exists();

    if is_new {
        fs::create_dir_all(&macos_dir)?;
        fs::create_dir_all(&resources_dir)?;

        let icon_bytes: Option<&[u8]> = icon_override.or_else(|| {
            if key == DEFAULT_KEY {
                Some(DEFAULT_ICON)
            } else {
                None
            }
        });
        let has_icon = icon_bytes.is_some();

        write_info_plist(
            &plist_path,
            key,
            &effective_display,
            has_icon,
            identifier_override,
        )?;

        if let Some(bytes) = icon_bytes {
            fs::write(resources_dir.join("icon.icns"), bytes)?;
        }
    }

    // Refresh the bundled executable (mtime-guarded — skipped when already
    // up-to-date). Bundle updates from `cargo build` land here.
    let refreshed = copy_self_into_bundle(&exe_path)?;

    // Only ad-hoc codesign when we actually materialized fresh state or
    // touched the executable. Signing on every send would generate a new
    // `CodeDirectory` hash per invocation and pump LaunchServices' bundle
    // registry (visible as `_LSServer_CopyLocalDatabase (seeded? 0)` spam
    // in the system log at scale). `--force` still makes it idempotent when
    // we do sign.
    if is_new || refreshed {
        ad_hoc_codesign(&bundle)?;
        // `codesign` embeds the signature into the Mach-O `__LINKEDIT`
        // segment — an in-place modification that bumps the executable's
        // mtime to the wall clock. Re-anchor it to the source binary's
        // mtime so the next `copy_self_into_bundle` check short-circuits
        // instead of triggering another copy + sign cycle.
        anchor_exe_mtime_to_source(&exe_path)?;
    }

    Ok(bundle)
}

/// Set `dest`'s mtime to match `current_exe()`'s — the anchor
/// [`copy_self_into_bundle`] uses to detect "no rebuild since last copy".
///
/// Called by [`ensure_bundle`] after [`ad_hoc_codesign`] because codesign
/// modifies the executable in place (bumping its mtime to wall-clock) and
/// would otherwise defeat the mtime-based skip on the next send.
///
/// # Errors
/// [`MacosError::Io`] on `current_exe` resolution or `set_times` failure.
fn anchor_exe_mtime_to_source(dest: &Path) -> Result<(), MacosError> {
    let src = std::env::current_exe()?;
    let src_mtime = fs::metadata(&src)?.modified()?;
    let times = fs::FileTimes::new().set_modified(src_mtime);
    fs::File::options().write(true).open(dest)?.set_times(times)?;
    Ok(())
}

/// Serialize an [`InfoPlist`] to disk as XML.
///
/// `has_icon` controls whether `CFBundleIconFile` is set. When true, the
/// caller must also write `Contents/Resources/icon.icns` (materialization
/// order is handled by [`ensure_bundle`]).
///
/// # Errors
/// [`MacosError::Plist`] or [`MacosError::Io`] on failure.
pub fn write_info_plist(
    dest: &Path,
    key: &str,
    display: &str,
    has_icon: bool,
    identifier_override: Option<&str>,
) -> Result<(), MacosError> {
    let bundle_identifier = identifier_override
        .map(String::from)
        .unwrap_or_else(|| format!("com.notify.{key}"));
    let payload = InfoPlist {
        bundle_name: display,
        bundle_display_name: display,
        bundle_identifier,
        bundle_executable: "notif",
        bundle_short_version_string: "0.1.0",
        bundle_version: "1",
        bundle_package_type: "APPL",
        bundle_info_dict_version: "6.0",
        bundle_icon_file: if has_icon { Some("icon") } else { None },
        ls_ui_element: true,
        ls_minimum_system_version: "11.0",
        notif_sender_key: key,
    };
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &payload)?;
    let mut f = fs::File::create(dest)?;
    f.write_all(&buf)?;
    Ok(())
}

/// Copy the currently-running `notif` executable to `dest`, mode `0o755`.
///
/// Skips the copy if the destination is up-to-date (same mtime as
/// `current_exe`) to keep repeat invocations cheap.
///
/// Returns `true` when a copy was performed, `false` when the destination
/// was already up-to-date and skipped. Callers use this to decide whether
/// [`ad_hoc_codesign`] needs to run — signing on every send generates fresh
/// `CodeDirectory` hashes that churn LaunchServices' bundle registry.
///
/// # Errors
/// [`MacosError::Io`] on `current_exe` resolution or copy failure.
pub fn copy_self_into_bundle(dest: &Path) -> Result<bool, MacosError> {
    let src = std::env::current_exe()?;
    let src_meta = fs::metadata(&src)?;
    let src_mtime = src_meta.modified()?;

    // Fast path: destination already carries the source's mtime — skip.
    if let Ok(dst_meta) = fs::metadata(dest) {
        if let Ok(dst_mtime) = dst_meta.modified() {
            if src_mtime == dst_mtime {
                return Ok(false);
            }
        }
    }

    fs::copy(&src, dest)?;
    fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;

    // `fs::copy` on macOS resets `dest`'s mtime to the current wall clock
    // (`copyfile()` does not preserve times by default). Manually mirror the
    // source's mtime so the next `copy_self_into_bundle` invocation's
    // fast-path check compares apples to apples and short-circuits — this
    // is what makes [`ensure_bundle`] skip the ad-hoc codesign on every
    // subsequent send.
    let times = fs::FileTimes::new().set_modified(src_mtime);
    fs::File::options()
        .write(true)
        .open(dest)?
        .set_times(times)?;

    Ok(true)
}

/// Ad-hoc codesign the given `.app` — `codesign --sign - --deep --force`.
///
/// Called from the outer path's one-shot retry when UN center returns
/// [`MacosError::NotSigned`]. `codesign` ships with Xcode Command Line Tools,
/// present on any Mac that has ever run `xcode-select --install`.
///
/// # Errors
/// [`MacosError::Codesign`] if the child exits non-zero.
pub fn ad_hoc_codesign(bundle: &Path) -> Result<(), MacosError> {
    let out = Command::new("codesign")
        .args(["--sign", "-", "--deep", "--force"])
        .arg(bundle)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(MacosError::Codesign(stderr));
    }
    Ok(())
}

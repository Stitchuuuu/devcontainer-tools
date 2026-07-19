//! `notif.exe --install` / `notif.exe --uninstall` — zero-installer flow.
//!
//! `install_self` copies the running binary into `%LOCALAPPDATA%\notif\`,
//! appends that directory to the user `Path`, broadcasts `WM_SETTINGCHANGE`
//! so new processes pick it up without logoff, chains a `register_sender`
//! for the reserved `default` sender, then fires a warmup toast to confirm
//! the pipeline works under the fresh AUMID.
//!
//! `uninstall_self` reverses everything except the binary itself — that's
//! left in place with a log line pointing at the path (the user is likely
//! running from it right now).

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::aumid::{self, ResolvedIdentity};
use crate::backend::WindowsError;
use crate::dispatch::dispatch_send;
use crate::register;

const INSTALL_DIR_NAME: &str = "notif";
const EXE_NAME: &str = "notif.exe";

/// Install the current executable into `%LOCALAPPDATA%\notif\notif.exe`,
/// wire it into the user `Path`, register the default sender, and warm the
/// toast pipeline.
///
/// Idempotent : re-running is safe. Each step short-circuits when the
/// destination state already matches (binary bytes identical, `Path` entry
/// already present, registration already up to date).
pub fn install_self() -> Result<(), WindowsError> {
    let install_dir = install_dir()?;
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| WindowsError::plain(format!("mkdir {}: {e}", install_dir.display())))?;
    let dest_exe = install_dir.join(EXE_NAME);

    copy_self(&dest_exe)?;
    ensure_path_entry(&install_dir)?;
    broadcast_environment_change();

    let reg = register::register_sender("default", "Notif", None)?;
    if reg.already_registered {
        info!(target: "notif::install", "default sender already registered");
    } else {
        info!(
            target: "notif::install",
            aumid = %reg.aumid,
            clsid = %crate::aumid::clsid_string(&reg.clsid),
            "default sender registered",
        );
    }

    warmup_toast()?;
    info!(target: "notif::install", exe = %dest_exe.display(), "install complete");
    Ok(())
}

/// Reverse `install_self`. Leaves the binary at `%LOCALAPPDATA%\notif\notif.exe`
/// in place (deleting it would fail — we may be running from it) ; the log
/// line points at it so the user can remove it manually.
pub fn uninstall_self() -> Result<(), WindowsError> {
    let install_dir = install_dir()?;
    remove_path_entry(&install_dir)?;
    broadcast_environment_change();

    let senders = aumid::win::list_senders()
        .map_err(|e| WindowsError::with_context("RegEnumKey(Notif/Senders)", e))?;
    for key in senders {
        match aumid::win::read_manifest(&key) {
            Ok(Some(m)) => {
                if let Err(e) = register::remove_lnk(&m) {
                    warn!(target: "notif::uninstall", sender = %key, err = %e, "lnk removal failed");
                }
                if let Err(e) = register::remove_clsid_key(&m.clsid) {
                    warn!(target: "notif::uninstall", sender = %key, err = %e, "CLSID removal failed");
                }
            }
            Ok(None) => {
                debug!(target: "notif::uninstall", sender = %key, "manifest vanished mid-enum");
            }
            Err(e) => {
                warn!(target: "notif::uninstall", sender = %key, err = %e, "manifest read failed");
            }
        }
    }

    aumid::win::delete_root()
        .map_err(|e| WindowsError::with_context("RegDeleteTree(Software/Notif)", e))?;

    let icon_dir = install_dir.join("icons");
    if icon_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&icon_dir) {
            warn!(target: "notif::uninstall", err = %e, dir = %icon_dir.display(), "icons cleanup failed");
        }
    }

    info!(
        target: "notif::uninstall",
        exe = %install_dir.join(EXE_NAME).display(),
        "uninstall complete — binary left in place",
    );
    Ok(())
}

fn install_dir() -> Result<PathBuf, WindowsError> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| WindowsError::plain("LOCALAPPDATA env missing"))?;
    Ok(PathBuf::from(local).join(INSTALL_DIR_NAME))
}

fn copy_self(dest: &std::path::Path) -> Result<(), WindowsError> {
    let src = std::env::current_exe()
        .map_err(|e| WindowsError::plain(format!("current_exe: {e}")))?;

    if dest.exists() {
        let src_bytes = std::fs::read(&src)
            .map_err(|e| WindowsError::plain(format!("read src: {e}")))?;
        let dst_bytes = std::fs::read(dest)
            .map_err(|e| WindowsError::plain(format!("read dest: {e}")))?;
        if Sha256::digest(&src_bytes) == Sha256::digest(&dst_bytes) {
            info!(target: "notif::install", dest = %dest.display(), "binary already up to date");
            return Ok(());
        }
    }
    std::fs::copy(&src, dest)
        .map_err(|e| WindowsError::plain(format!("copy {} → {}: {e}", src.display(), dest.display())))?;
    info!(target: "notif::install", src = %src.display(), dest = %dest.display(), "binary copied");
    Ok(())
}

// ---- HKCU\Environment\Path -------------------------------------------------

fn ensure_path_entry(dir: &std::path::Path) -> Result<(), WindowsError> {
    let dir_str = dir.to_string_lossy().into_owned();
    let current = read_env_path()?;
    if path_contains(&current, &dir_str) {
        info!(target: "notif::install", "PATH already contains install dir");
        return Ok(());
    }
    let new = if current.is_empty() {
        dir_str.clone()
    } else if current.ends_with(';') {
        format!("{current}{dir_str}")
    } else {
        format!("{current};{dir_str}")
    };
    write_env_path(&new)?;
    info!(target: "notif::install", entry = %dir_str, "PATH appended");
    Ok(())
}

fn remove_path_entry(dir: &std::path::Path) -> Result<(), WindowsError> {
    let dir_str = dir.to_string_lossy().into_owned();
    let current = read_env_path()?;
    if !path_contains(&current, &dir_str) {
        info!(target: "notif::uninstall", "PATH already free of install dir");
        return Ok(());
    }
    let new: String = current
        .split(';')
        .filter(|seg| !seg.eq_ignore_ascii_case(&dir_str))
        .collect::<Vec<_>>()
        .join(";");
    write_env_path(&new)?;
    info!(target: "notif::uninstall", entry = %dir_str, "PATH entry removed");
    Ok(())
}

fn path_contains(pathvar: &str, dir: &str) -> bool {
    pathvar
        .split(';')
        .any(|seg| seg.eq_ignore_ascii_case(dir))
}

fn read_env_path() -> Result<String, WindowsError> {
    use windows::core::{Error as WinError, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_VALUE_TYPE,
    };

    let subkey = HSTRING::from("Environment");
    let mut hkey = HKEY::default();
    let rc = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_READ, &mut hkey)
    };
    if rc != ERROR_SUCCESS {
        return Err(WindowsError::with_context("RegOpenKeyEx(Environment)", WinError::from(rc.to_hresult())));
    }
    let name = HSTRING::from("Path");
    let mut ty = REG_VALUE_TYPE(0);
    let mut size: u32 = 0;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut ty),
            None,
            Some(&mut size),
        )
    };
    if rc == ERROR_FILE_NOT_FOUND {
        unsafe { let _ = RegCloseKey(hkey); }
        return Ok(String::new());
    }
    if rc != ERROR_SUCCESS && rc != ERROR_MORE_DATA {
        unsafe { let _ = RegCloseKey(hkey); }
        return Err(WindowsError::with_context("RegQueryValueEx(Path/size)", WinError::from(rc.to_hresult())));
    }
    let elems = (size as usize + 1) / 2;
    let mut buf = vec![0u16; elems.max(1)];
    let mut size2 = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size2),
        )
    };
    unsafe { let _ = RegCloseKey(hkey); }
    if rc != ERROR_SUCCESS {
        return Err(WindowsError::with_context("RegQueryValueEx(Path/read)", WinError::from(rc.to_hresult())));
    }
    let chars = size2 as usize / 2;
    let trimmed = &buf[..chars.saturating_sub(1)];
    Ok(String::from_utf16_lossy(trimmed))
}

fn write_env_path(value: &str) -> Result<(), WindowsError> {
    use windows::core::{Error as WinError, HSTRING, PCWSTR};
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
        REG_EXPAND_SZ,
    };

    let subkey = HSTRING::from("Environment");
    let mut hkey = HKEY::default();
    let rc = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), None, KEY_WRITE, &mut hkey)
    };
    if rc != ERROR_SUCCESS {
        return Err(WindowsError::with_context("RegOpenKeyEx(Environment/write)", WinError::from(rc.to_hresult())));
    }
    let name = HSTRING::from("Path");
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;
    // REG_EXPAND_SZ preserves `%SystemRoot%\…` semantics for entries the user
    // (or Windows) put in earlier — safer than REG_SZ which would freeze them.
    let rc = unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            REG_EXPAND_SZ,
            Some(std::slice::from_raw_parts(wide.as_ptr() as *const u8, bytes)),
        )
    };
    unsafe { let _ = RegCloseKey(hkey); }
    if rc != ERROR_SUCCESS {
        return Err(WindowsError::with_context("RegSetValueEx(Path)", WinError::from(rc.to_hresult())));
    }
    Ok(())
}

// ---- WM_SETTINGCHANGE broadcast --------------------------------------------

fn broadcast_environment_change() {
    use windows::core::w;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    let param = w!("Environment");
    let mut result: usize = 0;
    let ret = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(param.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            Some(&mut result),
        )
    };
    if ret.0 == 0 {
        warn!(target: "notif::install", "WM_SETTINGCHANGE broadcast timed out");
    } else {
        debug!(target: "notif::install", "WM_SETTINGCHANGE broadcast delivered");
    }
}

// ---- Warmup toast ----------------------------------------------------------

fn warmup_toast() -> Result<(), WindowsError> {
    use notif_core::{Notification, Priority, Sender};

    let sender = Sender::new("default")
        .map_err(|e| WindowsError::plain(format!("Sender::new(default): {e}")))?;
    let notif = Notification {
        title: "notif installed".to_string(),
        body: "Toast pipeline is live under the Notif AUMID.".to_string(),
        subtitle: None,
        priority: Priority::Normal,
        sender,
        id: Some("notif-install-warmup".to_string()),
        sound: None,
        image: None,
        on_timeout: None,
    };

    let resolved = aumid::resolve_for_sender("default");
    match &resolved {
        ResolvedIdentity::Registered { aumid, .. } => {
            match dispatch_send(&notif, aumid) {
                Ok(()) => info!(target: "notif::install", "warmup toast fired"),
                Err(e) => warn!(target: "notif::install", err = %e, "warmup toast failed (non-fatal)"),
            }
        }
        ResolvedIdentity::Fallback { .. } => {
            warn!(
                target: "notif::install",
                "warmup skipped — resolve fell back to Tier 1 spoof (registration didn't stick)",
            );
        }
    }
    Ok(())
}

//! Materialize the Tier 0 `.lnk` + CLSID registration + manifest for a sender.
//!
//! Wire diagram (a single `register_sender` call performs the four steps
//! sequentially, from left to right — each side effect is idempotent so
//! re-runs are cheap):
//!
//! ```text
//!  derive AUMID/CLSID ──► write .ico + .lnk ──► HKCU CLSID ──► HKCU manifest
//! ```
//!
//! Session 3 will bring `--activator-serve` online ; the CLSID key we grave
//! here already points at that (not-yet-implemented) subcommand so the
//! registration is complete on day one.

use std::path::{Path, PathBuf};

use tracing::{debug, info};

use crate::aumid::{self, clsid_string, derive_aumid, derive_clsid, Manifest};
use crate::backend::WindowsError;

/// Icon bytes for the reserved `default` sender. Materialized on-disk at
/// register time so the `.lnk` can reference a stable path. Regenerated
/// from `assets/notify.svg` via `cargo run -p notif-icon-gen`.
const DEFAULT_ICON: &[u8] = include_bytes!("../assets/notify.ico");

/// Outcome of a `register_sender` call — surfaced to the CLI so `--install`
/// can chain a warmup toast under the freshly registered AUMID.
#[derive(Debug, Clone)]
pub struct Registration {
    pub sender_key: String,
    pub aumid: String,
    pub clsid: uuid::Uuid,
    pub lnk_path: PathBuf,
    pub already_registered: bool,
}

/// Register `sender_key` as a Windows toast sender.
///
/// - Materializes an icon (`icon` if provided, else the embedded default).
/// - Writes `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Notif\<display>.lnk`
///   with `AppUserModel.ID = derive_aumid(sender_key)` +
///   `AppUserModel.ToastActivatorCLSID = derive_clsid(sender_key)`.
/// - Graves `HKCU\Software\Classes\CLSID\{clsid}\LocalServer32` pointing at
///   the currently running executable (typically `%LOCALAPPDATA%\notif\notif.exe`
///   after `--install`).
/// - Writes the manifest at `HKCU\Software\Notif\Senders\<sender-key>` so
///   `dispatch` and `uninstall` can resolve back to these values.
///
/// Idempotent — a matching manifest + on-disk `.lnk` short-circuits the
/// side effects.
pub fn register_sender(
    sender_key: &str,
    display_name: &str,
    icon: Option<&Path>,
) -> Result<Registration, WindowsError> {
    // Local var is deliberately not called `display` — the tracing macros
    // import `tracing::field::display` unhygienically, and a `%display`
    // shorthand would resolve to the function item, not our string.
    let aumid = derive_aumid(sender_key);
    let clsid = derive_clsid(sender_key);
    let lnk_path = lnk_path_for(display_name)?;
    let clsid_disp = clsid_string(&clsid);
    let lnk_disp = lnk_path.display().to_string();
    debug!(
        target: "notif::register",
        sender_key,
        display_name,
        aumid = %aumid,
        clsid = %clsid_disp,
        lnk_path = %lnk_disp,
        "resolved identity",
    );

    if let Ok(Some(existing)) = aumid::win::read_manifest(sender_key) {
        if existing.aumid == aumid
            && existing.clsid == clsid
            && existing.lnk_path == lnk_path
            && lnk_path.exists()
        {
            info!(
                target: "notif::register",
                sender_key,
                "already up to date",
            );
            return Ok(Registration {
                sender_key: sender_key.to_string(),
                aumid,
                clsid,
                lnk_path,
                already_registered: true,
            });
        }
    }

    let icon_path = materialize_icon(sender_key, icon)?;
    debug!(target: "notif::register", icon = %icon_path.display(), "icon ready");

    let exe = std::env::current_exe()
        .map_err(|e| WindowsError::plain(format!("current_exe: {e}")))?;
    write_lnk(&lnk_path, &exe, &icon_path, &aumid, &clsid)?;
    info!(target: "notif::register", lnk = %lnk_path.display(), "lnk written");

    write_clsid_key(&clsid, &exe)?;
    info!(target: "notif::register", clsid = %clsid_string(&clsid), "CLSID graved");

    let manifest = Manifest {
        display: display_name.to_string(),
        aumid: aumid.clone(),
        clsid,
        lnk_path: lnk_path.clone(),
    };
    aumid::win::write_manifest(sender_key, &manifest)
        .map_err(|e| WindowsError::with_context("write_manifest", e))?;
    info!(target: "notif::register", sender_key, "manifest written");

    Ok(Registration {
        sender_key: sender_key.to_string(),
        aumid,
        clsid,
        lnk_path,
        already_registered: false,
    })
}

/// Delete the `.lnk` for `manifest` if it exists. The registry key + manifest
/// are handled by the caller (`install::uninstall_self`).
pub(crate) fn remove_lnk(manifest: &Manifest) -> Result<(), WindowsError> {
    if manifest.lnk_path.exists() {
        std::fs::remove_file(&manifest.lnk_path)
            .map_err(|e| WindowsError::plain(format!("remove lnk: {e}")))?;
        info!(target: "notif::uninstall", lnk = %manifest.lnk_path.display(), "lnk removed");
    }
    Ok(())
}

/// Delete `HKCU\Software\Classes\CLSID\{clsid}` recursively.
pub(crate) fn remove_clsid_key(clsid: &uuid::Uuid) -> Result<(), WindowsError> {
    let subkey = format!(r"Software\Classes\CLSID\{}", clsid_string(clsid));
    reg::delete_tree_hkcu(&subkey)
        .map_err(|e| WindowsError::with_context("RegDeleteTree(CLSID)", e))?;
    info!(target: "notif::uninstall", clsid = %clsid_string(clsid), "CLSID removed");
    Ok(())
}

// ---- Icon materialization --------------------------------------------------

fn materialize_icon(sender_key: &str, icon: Option<&Path>) -> Result<PathBuf, WindowsError> {
    let dest_dir = icon_dir()?;
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| WindowsError::plain(format!("mkdir icons: {e}")))?;
    let dest = dest_dir.join(format!("{sender_key}.ico"));

    let bytes: Vec<u8> = match icon {
        Some(src) => std::fs::read(src)
            .map_err(|e| WindowsError::plain(format!("read icon {}: {e}", src.display())))?,
        None => DEFAULT_ICON.to_vec(),
    };

    // Idempotent : skip the write if the on-disk copy is byte-identical.
    if dest.exists() {
        if let Ok(existing) = std::fs::read(&dest) {
            if existing == bytes {
                return Ok(dest);
            }
        }
    }
    std::fs::write(&dest, &bytes)
        .map_err(|e| WindowsError::plain(format!("write {}: {e}", dest.display())))?;
    Ok(dest)
}

fn icon_dir() -> Result<PathBuf, WindowsError> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| WindowsError::plain("LOCALAPPDATA env missing"))?;
    Ok(PathBuf::from(local).join("notif").join("icons"))
}

fn lnk_path_for(display: &str) -> Result<PathBuf, WindowsError> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| WindowsError::plain("APPDATA env missing"))?;
    let dir = PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Notif");
    std::fs::create_dir_all(&dir)
        .map_err(|e| WindowsError::plain(format!("mkdir Start Menu: {e}")))?;
    Ok(dir.join(format!("{}.lnk", sanitize_filename(display))))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c })
        .collect()
}

// ---- Shortcut writer -------------------------------------------------------

fn write_lnk(
    path: &Path,
    target: &Path,
    icon: &Path,
    aumid: &str,
    clsid: &uuid::Uuid,
) -> Result<(), WindowsError> {
    use windows::core::{Interface, GUID, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize, IPersistFile,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Variant::VARENUM;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::InitPropVariantFromStringVector;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    // The activator CLSID must live in a PROPVARIANT with `vt = VT_CLSID (72)`
    // and `puuid` pointing to a CoTaskMemAlloc-backed GUID copy. windows-rs
    // exposes `InitPropVariantFromStringVector` but not `InitPropVariantFromCLSID`
    // — we build the PROPVARIANT by hand for this one field.
    const VT_CLSID: u16 = 72;

    // Ferry the RIID for IPersistFile / IPropertyStore in via the crate's
    // `Interface` impl — no manual GUID plumbing needed.

    unsafe {
        let _co = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        // We tolerate S_FALSE (already initialized on this thread) — the
        // guard drops CoUninitialize either way in `_drop`.

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| WindowsError::with_context("CoCreateInstance(ShellLink)", e))?;

        link.SetPath(&HSTRING::from(target.as_os_str()))
            .map_err(|e| WindowsError::with_context("IShellLink::SetPath", e))?;
        link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)
            .map_err(|e| WindowsError::with_context("IShellLink::SetIconLocation", e))?;
        if let Some(dir) = target.parent() {
            let _ = link
                .SetWorkingDirectory(&HSTRING::from(dir.as_os_str()));
        }

        let store: IPropertyStore = link.cast()
            .map_err(|e| WindowsError::with_context("IShellLink::cast<IPropertyStore>", e))?;

        // PKEY_AppUserModel_ID = {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, pid=5
        let pkey_aumid = PROPERTYKEY {
            fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
            pid: 5,
        };
        // PKEY_AppUserModel_ToastActivatorCLSID = same fmtid, pid=26
        let pkey_activator = PROPERTYKEY {
            fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
            pid: 26,
        };

        let aumid_h = HSTRING::from(aumid);
        let aumid_pcwstr = windows::core::PCWSTR(aumid_h.as_ptr());
        let mut aumid_arr = [aumid_pcwstr];
        let pv_aumid = InitPropVariantFromStringVector(Some(&mut aumid_arr))
            .map_err(|e| WindowsError::with_context("InitPropVariantFromStringVector(aumid)", e))?;
        store
            .SetValue(&pkey_aumid, &pv_aumid)
            .map_err(|e| WindowsError::with_context("IPropertyStore::SetValue(aumid)", e))?;

        // Build VT_CLSID PROPVARIANT by hand.
        let guid_copy = CoTaskMemAlloc(std::mem::size_of::<GUID>()) as *mut GUID;
        if guid_copy.is_null() {
            return Err(WindowsError::plain("CoTaskMemAlloc(GUID) returned NULL"));
        }
        *guid_copy = GUID::from_u128(clsid.as_u128());
        let mut pv_clsid = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::default();
        // Access the anonymous union to stamp vt + puuid.
        {
            let inner = &mut pv_clsid.Anonymous.Anonymous;
            inner.vt = VARENUM(VT_CLSID);
            inner.Anonymous.puuid = guid_copy;
        }
        store
            .SetValue(&pkey_activator, &pv_clsid)
            .map_err(|e| WindowsError::with_context("IPropertyStore::SetValue(activator)", e))?;

        store
            .Commit()
            .map_err(|e| WindowsError::with_context("IPropertyStore::Commit", e))?;

        let persist: IPersistFile = link.cast()
            .map_err(|e| WindowsError::with_context("IShellLink::cast<IPersistFile>", e))?;
        persist
            .Save(&HSTRING::from(path.as_os_str()), true)
            .map_err(|e| WindowsError::with_context("IPersistFile::Save", e))?;

        // No `PropVariantClear` — the PROPVARIANTs are dropped at end of
        // scope. Their embedded pointers are owned by CoTaskMemAlloc and
        // leak on process exit, which is fine for a one-shot CLI.

        CoUninitialize();
    }

    Ok(())
}

// ---- CLSID registry --------------------------------------------------------

fn write_clsid_key(clsid: &uuid::Uuid, exe: &Path) -> Result<(), WindowsError> {
    let subkey = format!(r"Software\Classes\CLSID\{}\LocalServer32", clsid_string(clsid));
    let cmdline = format!("\"{}\" --activator-serve", exe.display());
    reg::write_default_sz_hkcu(&subkey, &cmdline)
        .map_err(|e| WindowsError::with_context("RegSetValueEx(CLSID/LocalServer32)", e))?;
    Ok(())
}

// ---- Registry helpers (thin, scoped to CLSID + friends) --------------------

mod reg {
    use windows::core::{Error as WinError, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    /// Create `HKCU\<subkey>` (with any missing parents) and set its default
    /// value (`""`) to `value`.
    pub fn write_default_sz_hkcu(subkey: &str, value: &str) -> Result<(), WinError> {
        let hsub = HSTRING::from(subkey);
        let mut hkey = HKEY::default();
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(hsub.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(WinError::from(rc.to_hresult()));
        }
        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let rc = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR::null(), // NULL name = default value
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(wide.as_ptr() as *const u8, bytes)),
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        if rc != ERROR_SUCCESS {
            return Err(WinError::from(rc.to_hresult()));
        }
        Ok(())
    }

    pub fn delete_tree_hkcu(subkey: &str) -> Result<(), WinError> {
        let hsub = HSTRING::from(subkey);
        let rc = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(hsub.as_ptr())) };
        if rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(WinError::from(rc.to_hresult()))
        }
    }
}

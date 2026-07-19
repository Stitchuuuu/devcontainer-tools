//! Sender identity for the Windows toast pipeline.
//!
//! Two layers:
//! - Pure derivation ([`derive_aumid`], [`derive_clsid`]) — deterministic
//!   functions of a sender key. Compile everywhere so unit tests run on the
//!   Linux devcontainer without touching Windows APIs.
//! - Registry-backed resolution ([`resolve_for_sender`]) — reads the manifest
//!   at `HKCU\Software\Notif\Senders\<sender-key>` written by
//!   [`crate::register::register_sender`]. Absent registration falls back
//!   to the Tier 1 spoof so smoke tests without prior `register` still
//!   surface a toast in Action Center.

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Namespace UUID for CLSID derivation. Generated once and hardcoded so every
/// host derives the same CLSID for the same sender key.
const NOTIF_CLSID_NAMESPACE: Uuid = uuid::uuid!("6e6f7469-6600-5000-8000-000000000000");

/// Fallback attribution used when a sender has no registration. Universal on
/// any host with VS Code installed.
pub const TIER1_FALLBACK_AUMID: &str = "Microsoft.VisualStudioCode";

/// Registry root under HKCU where per-sender manifests live.
#[cfg(target_os = "windows")]
pub(crate) const MANIFEST_ROOT: &str = r"Software\Notif\Senders";

/// Return the canonical AUMID for `sender_key`.
///
/// Shape : `Notif.<sender_key>.<hex16>` where `hex16` is the leading 16 hex
/// characters of `SHA-256(sender_key)`. Deterministic — the same input always
/// produces the same output, so `register` and `dispatch` agree without
/// persisting the derived string.
pub fn derive_aumid(sender_key: &str) -> String {
    let digest = Sha256::digest(sender_key.as_bytes());
    let hex_short = hex::encode(&digest[..8]);
    format!("Notif.{}.{}", sender_key, hex_short)
}

/// Return the canonical CLSID for `sender_key`.
///
/// UUID v5 (SHA-1 + fixed namespace). Deterministic — register / dispatch /
/// uninstall find the same CLSID key without storing a copy.
pub fn derive_clsid(sender_key: &str) -> Uuid {
    Uuid::new_v5(&NOTIF_CLSID_NAMESPACE, sender_key.as_bytes())
}

/// Format a UUID as the Windows CLSID string, `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
pub fn clsid_string(uuid: &Uuid) -> String {
    format!("{{{}}}", uuid.hyphenated())
}

/// Sender identity resolved from either the on-disk manifest or a fallback.
#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub enum ResolvedIdentity {
    /// Sender is registered — dispatch under the derived AUMID and route
    /// activator callbacks through the derived CLSID.
    Registered { aumid: String, clsid: Uuid },
    /// No registration found. Dispatch under the Tier 1 spoof so the
    /// pipeline still shows *something* ; callbacks are not wired.
    Fallback { aumid: String },
}

#[cfg(target_os = "windows")]
impl ResolvedIdentity {
    pub fn aumid(&self) -> &str {
        match self {
            Self::Registered { aumid, .. } | Self::Fallback { aumid } => aumid,
        }
    }
}

/// Manifest entry stored under `HKCU\Software\Notif\Senders\<sender-key>`.
///
/// Written by `register_sender` ; read by `resolve_for_sender` and
/// `uninstall_self`.
#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub struct Manifest {
    pub display: String,
    pub aumid: String,
    pub clsid: Uuid,
    pub lnk_path: std::path::PathBuf,
}

/// Look up the manifest for `sender_key` and return a [`ResolvedIdentity`].
///
/// On any registry error (key missing, value missing, malformed CLSID) we
/// log a warn and return the Tier 1 fallback — a failed lookup must not
/// break dispatch.
#[cfg(target_os = "windows")]
pub fn resolve_for_sender(sender_key: &str) -> ResolvedIdentity {
    match win::read_manifest(sender_key) {
        Ok(Some(m)) => ResolvedIdentity::Registered { aumid: m.aumid, clsid: m.clsid },
        Ok(None) => {
            tracing::warn!(
                target: "notif::aumid",
                sender_key,
                "no registration, using Tier 1 spoof"
            );
            ResolvedIdentity::Fallback { aumid: TIER1_FALLBACK_AUMID.to_string() }
        }
        Err(e) => {
            tracing::warn!(
                target: "notif::aumid",
                sender_key,
                error = %e,
                "manifest read failed, using Tier 1 spoof"
            );
            ResolvedIdentity::Fallback { aumid: TIER1_FALLBACK_AUMID.to_string() }
        }
    }
}

// ---- Windows-only manifest I/O ---------------------------------------------

#[cfg(target_os = "windows")]
pub(crate) mod win {
    use super::{Manifest, MANIFEST_ROOT};
    use std::path::PathBuf;
    use uuid::Uuid;
    use windows::core::{Error as WinError, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegEnumKeyExW, RegGetValueW,
        RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE,
        REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
    };

    pub fn read_manifest(sender_key: &str) -> Result<Option<Manifest>, WinError> {
        let subkey = format!(r"{}\{}", MANIFEST_ROOT, sender_key);
        let hsub = HSTRING::from(subkey.as_str());
        let mut hkey = HKEY::default();
        let open = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(hsub.as_ptr()), None, KEY_READ, &mut hkey)
        };
        if open == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if open != ERROR_SUCCESS {
            return Err(WinError::from(open.to_hresult()));
        }
        let display = read_sz(hkey, "display")?;
        let aumid = read_sz(hkey, "aumid")?;
        let clsid_raw = read_sz(hkey, "clsid")?;
        let lnk_path = read_sz(hkey, "lnk_path")?;
        unsafe { let _ = RegCloseKey(hkey); }

        let clsid = parse_clsid(&clsid_raw)
            .map_err(|_| WinError::from_hresult(windows::core::HRESULT(-2147024809i32)))?; // E_INVALIDARG
        Ok(Some(Manifest {
            display,
            aumid,
            clsid,
            lnk_path: PathBuf::from(lnk_path),
        }))
    }

    pub fn write_manifest(sender_key: &str, m: &Manifest) -> Result<(), WinError> {
        let subkey = format!(r"{}\{}", MANIFEST_ROOT, sender_key);
        let hsub = HSTRING::from(subkey.as_str());
        let mut hkey = HKEY::default();
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(hsub.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                None,
                &mut hkey,
                None,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(WinError::from(rc.to_hresult()));
        }
        let clsid_str = super::clsid_string(&m.clsid);
        write_sz(hkey, "display", &m.display)?;
        write_sz(hkey, "aumid", &m.aumid)?;
        write_sz(hkey, "clsid", &clsid_str)?;
        write_sz(hkey, "lnk_path", &m.lnk_path.to_string_lossy())?;
        unsafe { let _ = RegCloseKey(hkey); }
        Ok(())
    }

    /// Enumerate every registered sender key under `HKCU\Software\Notif\Senders`.
    pub fn list_senders() -> Result<Vec<String>, WinError> {
        let root = HSTRING::from(MANIFEST_ROOT);
        let mut hkey = HKEY::default();
        let open = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(root.as_ptr()), None, KEY_READ, &mut hkey)
        };
        if open == ERROR_FILE_NOT_FOUND {
            return Ok(Vec::new());
        }
        if open != ERROR_SUCCESS {
            return Err(WinError::from(open.to_hresult()));
        }
        let mut out = Vec::new();
        let mut index: u32 = 0;
        loop {
            let mut name_buf = vec![0u16; 512];
            let mut name_len: u32 = name_buf.len() as u32;
            let rc = unsafe {
                RegEnumKeyExW(
                    hkey,
                    index,
                    Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if rc != ERROR_SUCCESS {
                // Any non-success — including ERROR_NO_MORE_ITEMS — terminates enumeration.
                break;
            }
            let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            out.push(name);
            index += 1;
        }
        unsafe { let _ = RegCloseKey(hkey); }
        Ok(out)
    }

    /// Recursively delete `HKCU\Software\Notif` — the manifest root plus any
    /// sibling keys the uninstall path decides to add later.
    pub fn delete_root() -> Result<(), WinError> {
        let root = HSTRING::from(r"Software\Notif");
        let rc = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(root.as_ptr())) };
        if rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(WinError::from(rc.to_hresult()))
        }
    }

    fn read_sz(hkey: HKEY, name: &str) -> Result<String, WinError> {
        let hname = HSTRING::from(name);
        let mut size: u32 = 0;
        let rc = unsafe {
            RegGetValueW(
                hkey,
                PCWSTR::null(),
                PCWSTR(hname.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut size),
            )
        };
        if rc != ERROR_SUCCESS && rc != ERROR_MORE_DATA {
            return Err(WinError::from(rc.to_hresult()));
        }
        let elems = (size as usize + 1) / 2;
        let mut buf = vec![0u16; elems.max(1)];
        let mut size2 = (buf.len() * 2) as u32;
        let rc = unsafe {
            RegGetValueW(
                hkey,
                PCWSTR::null(),
                PCWSTR(hname.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                Some(&mut size2),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(WinError::from(rc.to_hresult()));
        }
        let chars = size2 as usize / 2;
        let trimmed = &buf[..chars.saturating_sub(1)]; // drop trailing NUL
        Ok(String::from_utf16_lossy(trimmed))
    }

    fn write_sz(hkey: HKEY, name: &str, value: &str) -> Result<(), WinError> {
        let hname = HSTRING::from(name);
        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        let rc = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(hname.as_ptr()),
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(wide.as_ptr() as *const u8, bytes)),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(WinError::from(rc.to_hresult()));
        }
        Ok(())
    }

    fn parse_clsid(s: &str) -> Result<Uuid, uuid::Error> {
        let trimmed = s.trim_matches(|c| c == '{' || c == '}');
        Uuid::parse_str(trimmed)
    }
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aumid_is_deterministic() {
        assert_eq!(derive_aumid("claude-code"), derive_aumid("claude-code"));
        assert_ne!(derive_aumid("claude-code"), derive_aumid("other"));
    }

    #[test]
    fn aumid_starts_with_notif_prefix_and_alphanumeric() {
        let a = derive_aumid("claude-code");
        assert!(a.starts_with("Notif."), "AUMID must begin with the Notif. prefix");
        assert!(a.chars().next().unwrap().is_alphanumeric());
    }

    #[test]
    fn aumid_under_windows_length_limit() {
        // Windows AUMID validation rejects strings ≥ 129 chars. A 64-char
        // sender key + our 22-char envelope stays well under.
        let long = "a".repeat(64);
        assert!(derive_aumid(&long).len() < 129);
    }

    #[test]
    fn aumid_carries_sender_key_verbatim() {
        // The sender key appears in the AUMID so a human reading the toast
        // properties can eyeball which sender it maps to.
        let a = derive_aumid("claude-code");
        assert!(a.contains("claude-code"), "AUMID `{a}` should embed the sender key");
    }

    #[test]
    fn aumid_hash_suffix_is_16_hex_chars() {
        let a = derive_aumid("x");
        let suffix = a.rsplit('.').next().unwrap();
        assert_eq!(suffix.len(), 16);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn clsid_is_deterministic() {
        assert_eq!(derive_clsid("claude-code"), derive_clsid("claude-code"));
        assert_ne!(derive_clsid("claude-code"), derive_clsid("other"));
    }

    #[test]
    fn clsid_string_is_curly_brace_wrapped() {
        let s = clsid_string(&derive_clsid("claude-code"));
        assert!(s.starts_with('{') && s.ends_with('}'));
        // 32 hex + 4 dashes + 2 braces = 38
        assert_eq!(s.len(), 38);
    }

    #[test]
    fn clsid_uses_v5_version() {
        let uuid = derive_clsid("anything");
        assert_eq!(uuid.get_version_num(), 5, "must be UUID v5 for stability");
    }
}

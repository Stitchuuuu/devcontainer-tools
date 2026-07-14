// Spawn a child process in the active console user's session from a
// service running as LocalSystem in Session 0.
//
// The recipe (well-worn but easy to get wrong) :
//   1. Discover the active console session via WTSGetActiveConsoleSessionId.
//   2. Query the user's impersonation token for that session.
//   3. DuplicateTokenEx to a primary token — CreateProcessAsUserW rejects
//      impersonation tokens. SecurityIdentification + TokenPrimary are
//      the combination that works ; other pairings return
//      ERROR_BAD_TOKEN_TYPE.
//   4. Build a Unicode environment block from the user's profile so the
//      child sees USERPROFILE / TEMP / PATH as the user would.
//   5. STARTUPINFOW.lpDesktop = "winsta0\\default" so the child renders
//      on the interactive desktop (not Session 0's invisible one).
//   6. CreateProcessAsUserW with CREATE_UNICODE_ENVIRONMENT +
//      CREATE_NO_WINDOW + NORMAL_PRIORITY_CLASS. bInheritHandles = FALSE
//      because cross-session handle inheritance is undefined.
//   7. Close everything — we track spawned children by PID only.

use std::ffi::c_void;
use std::ffi::OsStr;
use std::mem::size_of;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_NO_TOKEN, HANDLE};
use windows::Win32::Security::{
    DuplicateTokenEx, SecurityIdentification, TokenPrimary, SECURITY_ATTRIBUTES,
    TOKEN_ALL_ACCESS,
};
use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, NORMAL_PRIORITY_CLASS,
    PROCESS_INFORMATION, STARTUPINFOW,
};

use crate::platform::argv::build_command_line;

const NO_USER_SESSION: u32 = 0xFFFFFFFF;

pub fn spawn_in_active_user_session(exe: &Path, args: &[&OsStr]) -> Result<u32> {
    let session_id = unsafe { WTSGetActiveConsoleSessionId() };
    if session_id == NO_USER_SESSION {
        anyhow::bail!("no active console session");
    }

    let mut user_token = HANDLE::default();
    let query = unsafe { WTSQueryUserToken(session_id, &mut user_token) };
    if let Err(e) = query {
        if e.code().0 == ERROR_NO_TOKEN.0 as i32 {
            anyhow::bail!(
                "no user token for session {session_id} (logon screen / RDP disconnected)"
            );
        }
        return Err(anyhow!("WTSQueryUserToken: {e}"));
    }

    // From here `user_token` must be closed on every path.
    let primary = match duplicate_to_primary(user_token) {
        Ok(h) => h,
        Err(e) => {
            unsafe { let _ = CloseHandle(user_token); }
            return Err(e);
        }
    };
    unsafe { let _ = CloseHandle(user_token); }

    // From here `primary` must be closed on every path.
    let env_block = match create_env_block(primary) {
        Ok(p) => p,
        Err(e) => {
            unsafe { let _ = CloseHandle(primary); }
            return Err(e);
        }
    };

    let mut cmd_line = build_command_line(exe, args);
    let mut desktop: Vec<u16> = "winsta0\\default\0".encode_utf16().collect();

    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpDesktop: PWSTR(desktop.as_mut_ptr()),
        ..Default::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();

    let null_sa: Option<*const SECURITY_ATTRIBUTES> = None;
    let create = unsafe {
        CreateProcessAsUserW(
            Some(primary),
            PCWSTR::null(),
            Some(PWSTR(cmd_line.as_mut_ptr())),
            null_sa,
            null_sa,
            false,
            CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | NORMAL_PRIORITY_CLASS,
            Some(env_block),
            PCWSTR::null(),
            &startup,
            &mut process_info,
        )
    };

    // Always release the env block + primary token, even on failure.
    unsafe {
        let _ = DestroyEnvironmentBlock(env_block);
        let _ = CloseHandle(primary);
    }

    create.context("CreateProcessAsUserW")?;

    // Close child handles immediately — no tracking.
    unsafe {
        let _ = CloseHandle(process_info.hThread);
        let _ = CloseHandle(process_info.hProcess);
    }

    Ok(process_info.dwProcessId)
}

fn duplicate_to_primary(impersonation: HANDLE) -> Result<HANDLE> {
    let mut primary = HANDLE::default();
    unsafe {
        DuplicateTokenEx(
            impersonation,
            TOKEN_ALL_ACCESS,
            None,
            SecurityIdentification,
            TokenPrimary,
            &mut primary,
        )
    }
    .context("DuplicateTokenEx")?;
    Ok(primary)
}

fn create_env_block(primary: HANDLE) -> Result<*mut c_void> {
    let mut block: *mut c_void = std::ptr::null_mut();
    unsafe { CreateEnvironmentBlock(&mut block, Some(primary), false) }
        .context("CreateEnvironmentBlock")?;
    if block.is_null() {
        anyhow::bail!("CreateEnvironmentBlock returned null block");
    }
    Ok(block)
}

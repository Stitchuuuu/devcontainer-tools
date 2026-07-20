//! Console attachment helper used by the CLI entry point.
//!
//! `notif.exe` is compiled with `windows_subsystem = "windows"` so
//! Explorer's `LocalServer32` cold-spawn doesn't create an ephemeral
//! conhost (the pre-3.5 cold-start flash). CLI subcommands still need
//! stderr, so `main` re-attaches to the parent console early via
//! [`attach_parent_console`] whenever we weren't launched by Windows
//! COM (the `-Embedding` marker gates the call at the caller side).

use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

/// Attach the current process to the parent's console when one exists.
/// Silent no-op on failure — a `ShellExecute` / double-click launch has
/// no parent console, and neither does a COM server activation ; both
/// are expected paths.
pub fn attach_parent_console() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

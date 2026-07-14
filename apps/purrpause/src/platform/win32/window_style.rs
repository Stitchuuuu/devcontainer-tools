// Post-create window styling for the popup. tao's builder options handle
// most of the fullscreen-transparent-borderless-topmost combo, but two
// bits still need SetWindowLongPtrW:
//
//   WS_EX_TOOLWINDOW  — hides the window from Alt+Tab and the taskbar,
//                       so a child can't tab back to their game.
//   WS_EX_TOPMOST     — belt-and-braces on top of tao's with_always_on_top,
//                       since Fullscreen::Borderless + toolwindow has
//                       been observed to drop the topmost bit on some
//                       Windows shell revisions.

use anyhow::{Context, Result};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

pub fn apply_topmost_toolwindow(hwnd: HWND) -> Result<()> {
    unsafe {
        let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new = cur | (WS_EX_TOOLWINDOW.0 as isize) | (WS_EX_TOPMOST.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .context("SetWindowPos HWND_TOPMOST")?;
    }
    Ok(())
}

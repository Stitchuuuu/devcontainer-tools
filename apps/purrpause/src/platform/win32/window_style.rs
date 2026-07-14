// Post-create window styling. tao/winit's builder options handle
// most of the fullscreen-transparent-borderless-topmost combo, but the
// following bits still need SetWindowLongPtrW to be reliable — the
// winit path has been observed to leave the title bar visible on
// certain virtualized graphics stacks (Parallels ARM64) and to drop
// the topmost bit after a Fullscreen::Borderless.

use anyhow::{Context, Result};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
    GWL_STYLE, HWND_TOPMOST, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, WS_CAPTION, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
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

/// Enable DWM-composited whole-window alpha transparency. Works
/// around eframe/glow/glutin failing to expose an alpha pixel format
/// on Parallels ARM64's virtualized ANGLE — the window is created
/// opaque, then DWM blends the whole framebuffer at `alpha/255`
/// opacity against the desktop. Uniform alpha only (no per-pixel
/// alpha), so rounded-corner transparency around a rectangle
/// requires the heavier UpdateLayeredWindow + bitmap approach.
///
/// `alpha = 255` → fully opaque (equivalent to no LWA_ALPHA at all).
/// `alpha = 217` → ~85% opaque, matches the design's rgba(_, _, _, 0.85).
/// `alpha = 128` → 50% opaque, quite see-through.
pub fn apply_layered_alpha(hwnd: HWND, alpha: u8) -> Result<()> {
    unsafe {
        let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new = cur | (WS_EX_LAYERED.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        SetLayeredWindowAttributes(
            hwnd,
            windows::Win32::Foundation::COLORREF(0),
            alpha,
            LWA_ALPHA,
        )
        .context("SetLayeredWindowAttributes")?;
    }
    Ok(())
}

/// Strip every window frame style (title bar, thick frame, min/max
/// boxes, sysmenu). Belt-and-braces on top of winit's with_decorations
/// (false) — that builder option is ignored on some Parallels ARM64
/// builds. `SWP_FRAMECHANGED` forces the DWM to recompute the
/// non-client area immediately so the change is visible without a
/// user resize.
pub fn strip_window_frame(hwnd: HWND) -> Result<()> {
    unsafe {
        let before = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let mask = (WS_CAPTION.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0
            | WS_SYSMENU.0) as isize;
        let new = before & !mask;
        let read_back = SetWindowLongPtrW(hwnd, GWL_STYLE, new);
        let after = GetWindowLongPtrW(hwnd, GWL_STYLE);
        tracing::info!(
            before = format!("0x{:x}", before),
            after = format!("0x{:x}", after),
            requested = format!("0x{:x}", new),
            set_returned = format!("0x{:x}", read_back),
            "GWL_STYLE strip"
        );
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
        .context("SetWindowPos SWP_FRAMECHANGED")?;
    }
    Ok(())
}

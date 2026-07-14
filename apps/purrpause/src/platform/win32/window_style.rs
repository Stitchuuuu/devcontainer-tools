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
    SWP_NOZORDER, WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME,
    WS_EX_LAYERED, WS_EX_STATICEDGE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_WINDOWEDGE,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
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

/// Disable DWM's non-client rendering (title bar chrome, resize
/// handles, drop shadow) AND opt out of Windows 11's auto-rounded
/// corners. Both APIs are needed on Win11 to get a truly bordless
/// transparent window : DWMWA_NCRENDERING_POLICY kills the shadow,
/// DWMWA_WINDOW_CORNER_PREFERENCE prevents the subtle white liseré
/// that Win11 draws around every top-level window by default.
pub fn disable_dwm_nc_rendering(hwnd: HWND) -> Result<()> {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    unsafe {
        let policy = DWMNCRP_DISABLED;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            std::mem::size_of_val(&policy) as u32,
        )
        .context("DwmSetWindowAttribute(NCRENDERING_POLICY, DISABLED)")?;

        // Win11+ : opt out of the auto-rounded corners that come with
        // a subtle white outline. Safe no-op on Win10 (returns
        // E_INVALIDARG which we swallow — the API is 20H1+).
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );
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
        // Strip GWL_STYLE bits : title bar, thick frame, min/max/sysmenu,
        // plus the 1-pixel WS_BORDER and WS_DLGFRAME that survive most
        // borderless requests and show up as a thin outline in the
        // client area after DWM composition.
        let style_before = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let style_mask = (WS_CAPTION.0
            | WS_THICKFRAME.0
            | WS_MINIMIZEBOX.0
            | WS_MAXIMIZEBOX.0
            | WS_SYSMENU.0
            | WS_BORDER.0
            | WS_DLGFRAME.0) as isize;
        let style_new = style_before & !style_mask;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style_new);

        // Also strip GWL_EXSTYLE edges — WS_EX_WINDOWEDGE /
        // CLIENTEDGE / STATICEDGE / DLGMODALFRAME each render a
        // subtle 1-2px 3D border that survives WS_CAPTION removal.
        let ex_before = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let ex_mask = (WS_EX_WINDOWEDGE.0
            | WS_EX_CLIENTEDGE.0
            | WS_EX_STATICEDGE.0
            | WS_EX_DLGMODALFRAME.0) as isize;
        let ex_new = ex_before & !ex_mask;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_new);

        let style_after = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let ex_after = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        tracing::info!(
            style_before = format!("0x{:x}", style_before),
            style_after = format!("0x{:x}", style_after),
            ex_before = format!("0x{:x}", ex_before),
            ex_after = format!("0x{:x}", ex_after),
            "window frame strip"
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

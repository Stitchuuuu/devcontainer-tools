// Countdown widget — small top-right always-on-top notification rendered
// with eframe/egui. Spawned by the service at each palier boundary
// before the popup (T-15 / T-10 / T-5 / T-1). Two lines : palier
// message (from Config::pre_notif_messages[palier]) + live counter
// (from Config::countdown_template, 500ms refresh).
//
// Two pure helpers (format_countdown, resolve_palier_message) live
// outside the #[cfg(windows)] gate so they run under Linux `cargo test`.
// The event loop and window styling are Windows-only.
//
// Dismiss (× click) or auto-close at counter=0 both call
// ViewportCommand::Close. The service tracks the palier as notified in
// its `fired_paliers` set — the next palier boundary spawns a fresh
// widget on its own schedule.

use crate::config::Config;

/// Substitutes `{mm}`, `{ss}`, `{total_min}` in the template. Unknown
/// placeholders pass through unchanged. `{ss}` is zero-padded to two
/// digits ; `{mm}` and `{total_min}` are not padded. `{total_min}` is
/// the ceiling of `remaining_seconds / 60` so a display of "1 min
/// restantes" makes sense at 61 s but not at 60 s (which is "1 min"
/// exactly).
pub fn format_countdown(template: &str, remaining_seconds: u64) -> String {
    let mm = remaining_seconds / 60;
    let ss = remaining_seconds % 60;
    let total_min = remaining_seconds.div_ceil(60);
    template
        .replace("{mm}", &mm.to_string())
        .replace("{ss}", &format!("{ss:02}"))
        .replace("{total_min}", &total_min.to_string())
}

/// Reads `Config::pre_notif_messages[palier]`, falls back to a
/// templated default (`"Pause dans <N> min"`) if the palier is missing
/// from the map. Callers pass the raw palier value (15, 10, 5, 1, or a
/// custom user value) — the config UI (session 6) will guarantee an
/// entry exists for every palier in `pre_notification_minutes`, but
/// state.dat edited by hand can drift.
pub fn resolve_palier_message(cfg: &Config, palier: u32) -> String {
    cfg.pre_notif_messages
        .get(&palier)
        .cloned()
        .unwrap_or_else(|| format!("Pause dans {palier} min"))
}

#[cfg(windows)]
pub fn run(seconds: u64, palier: u32, debug: bool) -> anyhow::Result<()> {
    windows_impl::run(seconds, palier, debug)
}

#[cfg(not(windows))]
pub fn run(_seconds: u64, _palier: u32, _debug: bool) -> anyhow::Result<()> {
    anyhow::bail!("countdown mode is Windows-only")
}

#[cfg(windows)]
mod windows_impl {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use anyhow::{anyhow, Context, Result};
    use eframe::egui;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    use crate::modes::install::STATE_DAT;
    use crate::platform::win32::{fullscreen_detect, window_style};

    use super::{format_countdown, resolve_palier_message};

    const WIDTH: f32 = 320.0;
    const HEIGHT: f32 = 90.0;
    const MARGIN: i32 = 20;

    pub fn run(seconds: u64, palier: u32, debug: bool) -> Result<()> {
        let cfg = crate::config::load_or_default(Path::new(STATE_DAT));

        // If this palier is opted-in for force-minimize, escalate over
        // any foreground fullscreen app BEFORE creating our own window
        // — otherwise our topmost bit competes with the game's and
        // exclusive-fullscreen games win. Debug mode skips the
        // minimize so a developer's current window isn't stolen.
        if !debug && cfg.force_minimize_paliers.contains(&palier) {
            if let Err(e) = fullscreen_detect::force_minimize_foreground_fullscreen() {
                tracing::warn!(error = ?e, palier, "force-minimize check failed");
            }
        }
        if debug {
            tracing::info!("countdown widget: --debug mode (force-minimize skipped)");
        }

        // Position top-right on the primary monitor's work area (i.e.
        // excluding the taskbar). Fallback to a plausible 1920×1080
        // origin when the monitor query fails — the widget will still
        // show, just possibly off-position on multi-monitor setups.
        //
        // eframe/winit's with_position takes LOGICAL points, but
        // GetMonitorInfoW returns PHYSICAL pixels. On Retina / high-DPI
        // displays (Parallels default is often 2x on Apple Silicon)
        // passing physical coords as logical pushes the window
        // off-screen at scale × factor. Query the monitor's DPI and
        // divide before feeding eframe.
        let (mx, my, mw, mh, scale) = match primary_monitor_bounds_and_scale() {
            Ok(v) => {
                tracing::info!(
                    mx = v.0, my = v.1, mw = v.2, mh = v.3, scale = v.4,
                    "primary monitor bounds (physical) + DPI scale"
                );
                v
            }
            Err(e) => {
                tracing::warn!(error = ?e, "monitor/DPI query failed, using 1920x1080 @ 1.0 fallback");
                (0, 0, 1920, 1080, 1.0)
            }
        };

        // Convert physical → logical for eframe.
        let mx_l = mx as f32 / scale;
        let my_l = my as f32 / scale;
        let mw_l = mw as f32 / scale;
        let x_l = mx_l + mw_l - WIDTH - MARGIN as f32;
        let y_l = my_l + MARGIN as f32;
        tracing::info!(
            x_logical = x_l, y_logical = y_l, w = WIDTH, h = HEIGHT,
            "computed widget position (logical points for eframe)"
        );
        let _unused_mh = mh;   // kept for context in the log above

        // wgpu backend + D3D12 direct handles per-pixel alpha
        // correctly on virtualized graphics stacks (validated in
        // sandbox on Parallels ARM64). Enable transparency so the
        // widget's rounded corners show the desktop through.
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([WIDTH, HEIGHT])
                .with_position([x_l, y_l])
                .with_resizable(false)
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top(),
            ..Default::default()
        };

        let deadline = Instant::now() + Duration::from_secs(seconds);
        let message = resolve_palier_message(&cfg, palier);
        let template = cfg.countdown_template.clone();
        let title = cfg.popup_window_title.clone();

        tracing::info!(seconds, palier, monitor_mh = mh, "countdown widget starting");

        eframe::run_native(
            &title,
            options,
            Box::new(move |_cc| {
                Ok(Box::new(CountdownApp::new(deadline, message, template)))
            }),
        )
        .map_err(|e| anyhow!("eframe: {e}"))
    }

    struct CountdownApp {
        deadline: Instant,
        message: String,
        template: String,
        styled_once: bool,
        cached_hwnd: Option<isize>,
    }

    impl CountdownApp {
        fn new(deadline: Instant, message: String, template: String) -> Self {
            Self {
                deadline,
                message,
                template,
                styled_once: false,
                cached_hwnd: None,
            }
        }
    }

    impl eframe::App for CountdownApp {
        // Fully transparent clear so the rounded panel we paint below
        // shows the desktop through the corners.
        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            [0.0, 0.0, 0.0, 0.0]
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            // Cache the HWND on first frame, then re-strip the frame
            // every subsequent frame. winit's WndProc may re-apply the
            // caption/frame styles after our initial strip on some
            // virtualized graphics stacks (Parallels ARM64) — cheap to
            // re-strip and guarantees the styles stay off.
            if self.cached_hwnd.is_none() {
                tracing::info!("countdown widget: first ui() frame");
                if let Ok(handle) = frame.window_handle() {
                    if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                        let hwnd_isize = win32.hwnd.get();
                        tracing::info!(hwnd = format!("0x{:x}", hwnd_isize), "widget HWND acquired");
                        self.cached_hwnd = Some(hwnd_isize);
                    } else {
                        tracing::warn!("widget got a non-Win32 window handle — unexpected");
                    }
                } else {
                    tracing::warn!("widget: no window_handle available yet");
                }
            }

            if let Some(hwnd_isize) = self.cached_hwnd {
                let hwnd = HWND(hwnd_isize as *mut _);
                if !self.styled_once {
                    if let Err(e) = window_style::apply_topmost_toolwindow(hwnd) {
                        tracing::warn!(error = ?e, "apply_topmost_toolwindow failed");
                    }
                    // Disable DWM's non-client shadow rendering so
                    // the rounded transparent corners show the desktop
                    // cleanly (no outline / drop shadow).
                    if let Err(e) = window_style::disable_dwm_nc_rendering(hwnd) {
                        tracing::warn!(error = ?e, "disable_dwm_nc_rendering failed");
                    }
                    self.styled_once = true;
                }
                // Re-strip every frame — brute-force against winit's
                // WndProc re-applying default caption on WM_NCPAINT.
                if let Err(e) = window_style::strip_window_frame(hwnd) {
                    tracing::warn!(error = ?e, "strip_window_frame failed");
                }
            }

            let remaining = self
                .deadline
                .saturating_duration_since(Instant::now())
                .as_secs();

            if remaining == 0 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }

            // Refresh at 500ms so the counter advances visibly even
            // without any mouse/keyboard input.
            ui.ctx().request_repaint_after(Duration::from_millis(500));

            // Rounded semi-opaque dark panel — wgpu handles per-pixel
            // alpha so pixels outside the rounded rect stay alpha 0
            // and show the desktop through cleanly.
            let rect = ui.max_rect();
            ui.painter().rect_filled(
                rect,
                12.0,
                egui::Color32::from_rgba_premultiplied(15, 15, 22, 230),
            );

            // Content laid out inside a small inset from the rounded
            // background so nothing touches the corners.
            let content_rect = rect.shrink(12.0);

            let counter = format_countdown(&self.template, remaining);
            ui.painter().text(
                content_rect.min,
                egui::Align2::LEFT_TOP,
                &self.message,
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgba_premultiplied(240, 240, 245, 255),
            );
            ui.painter().text(
                content_rect.min + egui::vec2(0.0, 28.0),
                egui::Align2::LEFT_TOP,
                counter,
                egui::FontId::monospace(24.0),
                egui::Color32::from_rgb(255, 183, 77),
            );

            // Close button on top of everything, small × top-right.
            let close_size = 22.0;
            let close_rect = egui::Rect::from_min_size(
                egui::pos2(content_rect.right() - close_size, content_rect.top() - 2.0),
                egui::vec2(close_size, close_size),
            );
            if ui.put(close_rect, egui::Button::new("×").frame(false)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn primary_monitor_bounds_and_scale() -> Result<(i32, i32, u32, u32, f32)> {
        unsafe {
            let point = POINT { x: 0, y: 0 };
            let hmon = MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(hmon, &mut info).as_bool() {
                anyhow::bail!("GetMonitorInfoW failed");
            }
            let rc = info.rcWork;
            let w = (rc.right - rc.left) as u32;
            let h = (rc.bottom - rc.top) as u32;
            let scale = monitor_scale(hmon).unwrap_or(1.0);
            Ok((rc.left, rc.top, w, h, scale))
        }
    }

    fn monitor_scale(hmon: HMONITOR) -> Result<f32> {
        unsafe {
            let mut dpix: u32 = 96;
            let mut dpiy: u32 = 96;
            GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpix, &mut dpiy)
                .context("GetDpiForMonitor")?;
            Ok(dpix as f32 / 96.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn format_countdown_mm_ss_15_min() {
        assert_eq!(
            format_countdown("Pause dans {mm}:{ss}", 900),
            "Pause dans 15:00"
        );
    }

    #[test]
    fn format_countdown_zero_padded_seconds() {
        assert_eq!(
            format_countdown("Pause dans {mm}:{ss}", 65),
            "Pause dans 1:05"
        );
    }

    #[test]
    fn format_countdown_zero_remaining() {
        assert_eq!(
            format_countdown("Pause dans {mm}:{ss}", 0),
            "Pause dans 0:00"
        );
    }

    #[test]
    fn format_countdown_total_min_ceils() {
        // 61s → 2 min (ceil), 60s → 1 min exactly.
        assert_eq!(
            format_countdown("{total_min} min restantes", 61),
            "2 min restantes"
        );
        assert_eq!(
            format_countdown("{total_min} min restantes", 60),
            "1 min restantes"
        );
    }

    #[test]
    fn format_countdown_unknown_placeholder_passthrough() {
        assert_eq!(
            format_countdown("{unknown} {mm}", 60),
            "{unknown} 1"
        );
    }

    #[test]
    fn resolve_palier_message_known_palier() {
        let cfg = Config::default();
        // Default HashMap ships 15/10/5/1.
        assert_eq!(
            resolve_palier_message(&cfg, 15),
            "Prochaine pause dans 15 min"
        );
        assert_eq!(resolve_palier_message(&cfg, 1), "Pause dans 1 min !");
    }

    #[test]
    fn resolve_palier_message_unknown_palier_templated_fallback() {
        let cfg = Config::default();
        assert_eq!(resolve_palier_message(&cfg, 7), "Pause dans 7 min");
    }

    #[test]
    fn resolve_palier_message_empty_map_falls_back() {
        let mut cfg = Config::default();
        cfg.pre_notif_messages = HashMap::new();
        assert_eq!(resolve_palier_message(&cfg, 15), "Pause dans 15 min");
        assert_eq!(resolve_palier_message(&cfg, 1), "Pause dans 1 min");
    }
}

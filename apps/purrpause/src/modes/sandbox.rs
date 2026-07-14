// Sandbox mode : minimal isolated diagnostic window. Zero dependency
// on state.dat / IPC / wry / keyboard hook / force-minimize. Each
// preset tunes one dimension of the window creation stack so a smoke
// tester can figure out which combination Parallels' virtualized
// graphics stack accepts.
//
// Usage :
//   SystemHealthAgent.exe --sandbox --try <preset>
//
// Presets (case-insensitive) :
//   plain                       Opaque 500x300, decorated, top-left.
//   transparent                 with_transparent(true).
//   borderless                  with_decorations(false).
//   borderless-transparent      Both.
//   topmost                     with_always_on_top() + borderless + transparent.
//   fullscreen                  Fullscreen borderless opaque.
//   fullscreen-transparent      Fullscreen borderless with_transparent.
//   red-only                    Plain but fill bright red instead of dark.
//   center                      Plain but centered on primary monitor (uses
//                               eframe's default center-of-primary).
//   strip                       Borderless opaque + Win32 strip_window_frame post-create.
//   strip-transparent           Borderless transparent + strip_window_frame.
//
// All presets paint a big red square with a black cross so the window
// is unmistakable. Any preset that shows red = rendering works with
// that combination. Preset that renders as white/blank = we've found
// a combo Parallels doesn't like.

#[cfg(windows)]
pub fn run(preset: &str) -> anyhow::Result<()> {
    windows_impl::run(preset)
}

#[cfg(not(windows))]
pub fn run(_preset: &str) -> anyhow::Result<()> {
    anyhow::bail!("sandbox mode is Windows-only")
}

#[cfg(windows)]
mod windows_impl {
    use anyhow::{anyhow, Result};
    use eframe::egui;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use windows::Win32::Foundation::HWND;

    use crate::platform::win32::window_style;

    struct Cfg {
        title: &'static str,
        size: [f32; 2],
        transparent: bool,
        decorations: bool,
        always_on_top: bool,
        fullscreen: bool,
        center: bool,
        fill: egui::Color32,
        clear: [f32; 4],
        strip_frame_win32: bool,
    }

    fn defaults() -> Cfg {
        Cfg {
            title: "sandbox",
            size: [500.0, 300.0],
            transparent: false,
            decorations: true,
            always_on_top: false,
            fullscreen: false,
            center: false,
            fill: egui::Color32::from_rgb(220, 40, 40),
            clear: [0.85, 0.15, 0.15, 1.0],
            strip_frame_win32: false,
        }
    }

    pub fn run(preset: &str) -> Result<()> {
        let normalized = preset.to_ascii_lowercase();
        let mut cfg = defaults();
        match normalized.as_str() {
            "plain" => { /* baseline */ }
            "transparent" => {
                cfg.transparent = true;
                cfg.clear = [0.0, 0.0, 0.0, 0.0];
            }
            "borderless" => {
                cfg.decorations = false;
            }
            "borderless-transparent" => {
                cfg.decorations = false;
                cfg.transparent = true;
                cfg.clear = [0.0, 0.0, 0.0, 0.0];
            }
            "topmost" => {
                cfg.decorations = false;
                cfg.transparent = true;
                cfg.always_on_top = true;
                cfg.clear = [0.0, 0.0, 0.0, 0.0];
            }
            "fullscreen" => {
                cfg.fullscreen = true;
            }
            "fullscreen-transparent" => {
                cfg.fullscreen = true;
                cfg.transparent = true;
                cfg.clear = [0.0, 0.0, 0.0, 0.0];
            }
            "red-only" => {
                cfg.fill = egui::Color32::from_rgb(255, 0, 0);
                cfg.clear = [1.0, 0.0, 0.0, 1.0];
            }
            "center" => {
                cfg.center = true;
            }
            "strip" => {
                cfg.decorations = false;
                cfg.strip_frame_win32 = true;
            }
            "strip-transparent" => {
                cfg.decorations = false;
                cfg.transparent = true;
                cfg.strip_frame_win32 = true;
                cfg.clear = [0.0, 0.0, 0.0, 0.0];
            }
            _ => {
                tracing::warn!(
                    preset,
                    "sandbox: unknown preset, falling back to 'plain'"
                );
            }
        }

        tracing::info!(
            preset = %normalized,
            transparent = cfg.transparent,
            decorations = cfg.decorations,
            always_on_top = cfg.always_on_top,
            fullscreen = cfg.fullscreen,
            strip_frame_win32 = cfg.strip_frame_win32,
            fill = ?cfg.fill,
            "sandbox: starting"
        );

        let mut viewport = egui::ViewportBuilder::default()
            .with_title(cfg.title)
            .with_inner_size(cfg.size)
            .with_resizable(false)
            .with_decorations(cfg.decorations)
            .with_transparent(cfg.transparent);
        if cfg.always_on_top {
            viewport = viewport.with_always_on_top();
        }
        if cfg.fullscreen {
            viewport = viewport.with_fullscreen(true);
        }
        if !cfg.center && !cfg.fullscreen {
            // Fixed position top-left corner-ish so we know where to look.
            viewport = viewport.with_position([120.0, 120.0]);
        }

        let options = eframe::NativeOptions {
            viewport,
            ..Default::default()
        };

        let sandbox_app_cfg = SandboxAppCfg {
            fill: cfg.fill,
            clear: cfg.clear,
            strip_frame_win32: cfg.strip_frame_win32,
        };

        eframe::run_native(
            cfg.title,
            options,
            Box::new(move |_cc| Ok(Box::new(SandboxApp::new(sandbox_app_cfg)))),
        )
        .map_err(|e| anyhow!("eframe: {e}"))
    }

    struct SandboxAppCfg {
        fill: egui::Color32,
        clear: [f32; 4],
        strip_frame_win32: bool,
    }

    struct SandboxApp {
        cfg: SandboxAppCfg,
        first_frame_logged: bool,
        strip_applied: bool,
    }

    impl SandboxApp {
        fn new(cfg: SandboxAppCfg) -> Self {
            Self {
                cfg,
                first_frame_logged: false,
                strip_applied: false,
            }
        }
    }

    impl eframe::App for SandboxApp {
        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            self.cfg.clear
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            if !self.first_frame_logged {
                tracing::info!("sandbox: first ui() frame");
                self.first_frame_logged = true;
            }

            if self.cfg.strip_frame_win32 && !self.strip_applied {
                if let Ok(handle) = frame.window_handle() {
                    if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                        let hwnd = HWND(win32.hwnd.get() as *mut _);
                        tracing::info!(hwnd = format!("0x{:x}", win32.hwnd.get()), "sandbox: HWND acquired");
                        if let Err(e) = window_style::strip_window_frame(hwnd) {
                            tracing::warn!(error = ?e, "strip_window_frame failed");
                        }
                        self.strip_applied = true;
                    }
                }
            }

            let panel = egui::Frame::new().fill(self.cfg.fill).inner_margin(0);
            panel.show(ui, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter();
                // Solid fill (guaranteed via Frame) + a diagonal black cross
                // for good measure — anywhere red visible = window rendered.
                painter.line_segment(
                    [rect.left_top(), rect.right_bottom()],
                    egui::Stroke::new(4.0, egui::Color32::BLACK),
                );
                painter.line_segment(
                    [rect.right_top(), rect.left_bottom()],
                    egui::Stroke::new(4.0, egui::Color32::BLACK),
                );
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("SANDBOX OK")
                            .size(32.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                });
            });

            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(1000));
        }
    }
}

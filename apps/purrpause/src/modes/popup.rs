// Fullscreen transparent break-reminder popup — tao window + wry WebView.
//
// Two pure helpers (animation_pick, render_html) live outside the
// #[cfg(windows)] gate so they run under Linux `cargo test`. The event
// loop, keyboard hook and window styling are Windows-only.
//
// Resources (popup.html, vendored dotlottie-wc, animations) are served
// via a wry custom protocol (`purrpause://`) rather than file:// so that
// the popup.html's ES module imports and WASM fetches resolve against a
// same-origin scheme — WebView2 restricts several APIs on file:// origins.

use crate::config::{AnimationEntry, Config, RotationMode};

const FALLBACK_ANIM_PATH: &str = "animations/dance-cat.lottie";

/// Pure animation picker.
///
/// Filters `cfg.animations` to enabled entries and returns the pick plus
/// the next rotation index (Sequential mode) or `0` (Random mode, unused
/// by the caller). Returns `None` when no anim is enabled — caller uses
/// the bundled fallback path.
pub fn animation_pick(
    cfg: &Config,
    rotation_state: Option<u32>,
) -> Option<(AnimationEntry, u32)> {
    let enabled: Vec<&AnimationEntry> =
        cfg.animations.iter().filter(|a| a.enabled).collect();
    if enabled.is_empty() {
        return None;
    }
    let len = enabled.len();
    match cfg.rotation_mode {
        RotationMode::Random => {
            use rand::Rng;
            let idx = rand::rng().random_range(0..len);
            Some((enabled[idx].clone(), 0))
        }
        RotationMode::Sequential => {
            let cursor = rotation_state.unwrap_or(0) as usize;
            let idx = cursor % len;
            let next = (cursor as u32).wrapping_add(1);
            Some((enabled[idx].clone(), next))
        }
    }
}

/// Pure HTML template renderer. Applies eight `str::replace` calls in
/// deterministic order.
///
/// **No HTML-escaping.** Config strings originate from the passcode-gated
/// config UI on the same machine — the threat model does not include
/// HTML injection. If a future user-supplied `countdown_template`
/// contains a `"` char it may break the enclosing HTML attribute quote ;
/// session 6's config UI should reject that at input time.
pub fn render_html(
    template: &str,
    cfg: &Config,
    animation_path: &str,
    countdown_seconds: u32,
    preview: bool,
) -> String {
    let mode = if preview { "preview" } else { "live" };
    template
        .replace("{{ WINDOW_TITLE }}", &cfg.popup_window_title)
        .replace("{{ POPUP_TITLE }}", &cfg.popup_title)
        .replace("{{ POPUP_SUBTITLE }}", &cfg.popup_subtitle)
        .replace("{{ DISMISS_LABEL }}", &cfg.dismiss_button_label)
        .replace("{{ COUNTDOWN_TEMPLATE }}", &cfg.countdown_template)
        .replace("{{ ANIMATION_PATH }}", animation_path)
        .replace("{{ COUNTDOWN_SECONDS }}", &countdown_seconds.to_string())
        .replace("{{ POPUP_MODE }}", mode)
}

#[cfg(windows)]
pub fn run(preview: bool, debug: bool) -> anyhow::Result<()> {
    windows_impl::run(preview, debug)
}

#[cfg(not(windows))]
pub fn run(_preview: bool, _debug: bool) -> anyhow::Result<()> {
    anyhow::bail!("popup mode is Windows-only")
}

#[cfg(windows)]
mod windows_impl {
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::WindowExtWindows;
    use tao::window::{Fullscreen, WindowBuilder};

    use windows::Win32::Foundation::HWND;

    use crate::modes::install::{DIAGNOSTICS_CACHE_DIR, STATE_DAT};
    use crate::platform::win32::{fullscreen_detect, keyboard_hook, window_style};

    use super::{animation_pick, render_html, FALLBACK_ANIM_PATH};

    const ROTATION_DAT_FILENAME: &str = "rotation.dat";

    #[derive(Debug, Clone)]
    enum UserEvent {
        Dismiss,
    }

    pub fn run(preview: bool, debug: bool) -> Result<()> {
        let cfg =
            crate::config::load_or_default(Path::new(STATE_DAT));

        // Popup uses palier 0 for the force-minimize opt-in. Skipped
        // in preview mode (parent testing from their desktop) AND in
        // debug mode (developer wants to keep their current window).
        if !preview && !debug && cfg.force_minimize_paliers.contains(&0) {
            if let Err(e) = fullscreen_detect::force_minimize_foreground_fullscreen() {
                tracing::warn!(error = ?e, "force-minimize check failed");
            }
        }
        if debug {
            tracing::info!("popup: --debug mode (no keyboard hook, no force-minimize)");
        }

        // Sequential mode reads the rotation cursor from disk ; Random ignores it.
        let rotation_in = match cfg.rotation_mode {
            crate::config::RotationMode::Sequential => read_rotation_dat().ok(),
            crate::config::RotationMode::Random => None,
        };

        let (anim_path, rotation_out) = match animation_pick(&cfg, rotation_in) {
            Some((entry, next)) => (entry.file, Some(next)),
            None => (FALLBACK_ANIM_PATH.to_string(), None),
        };

        if matches!(cfg.rotation_mode, crate::config::RotationMode::Sequential) {
            if let Some(next) = rotation_out {
                if let Err(e) = write_rotation_dat(next) {
                    tracing::warn!(error = %e, "failed to persist rotation state");
                }
            }
        }

        // Preview keeps a visible countdown so the parent sees the UI in
        // motion, but the JS unlocks dismiss immediately via data-mode.
        let countdown = if preview {
            10
        } else {
            cfg.duration_minutes.saturating_mul(60)
        };

        let resources_dir = resources_dir().context("locate resources/")?;
        let template_path = resources_dir.join("popup.html");
        let template = std::fs::read_to_string(&template_path)
            .with_context(|| format!("read {}", template_path.display()))?;
        let rendered = render_html(&template, &cfg, &anim_path, countdown, preview);

        tracing::info!(
            preview,
            countdown,
            anim = %anim_path,
            "popup mode starting",
        );

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let window = WindowBuilder::new()
            .with_title(&cfg.popup_window_title)
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top(true)
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .build(&event_loop)
            .context("build tao window")?;

        let hwnd = HWND(window.hwnd() as *mut _);
        window_style::apply_topmost_toolwindow(hwnd)
            .context("apply WS_EX_TOOLWINDOW|WS_EX_TOPMOST")?;

        // Debug mode leaves Alt+F4 / Alt+Tab / Win+D / Ctrl+Esc alone
        // so a developer can exit the popup during smoke without
        // killing the process from Task Manager.
        let _kb_guard = if debug {
            None
        } else {
            Some(
                keyboard_hook::install_keyboard_hook()
                    .context("install keyboard hook")?,
            )
        };

        let proxy = event_loop.create_proxy();
        let protocol_resources = resources_dir.clone();
        let protocol_rendered = rendered.clone();

        let webview = wry::WebViewBuilder::new()
            .with_transparent(true)
            .with_custom_protocol(
                "purrpause".to_string(),
                move |_webview_id, request| {
                    serve_custom_protocol(
                        request,
                        &protocol_resources,
                        &protocol_rendered,
                    )
                },
            )
            .with_url("purrpause://localhost/popup.html")
            .with_ipc_handler(move |request| {
                if request.body() == "dismiss" {
                    let _ = proxy.send_event(UserEvent::Dismiss);
                }
            })
            .build(&window)
            .context("build wry webview")?;

        // Silence the unused-var lint : the webview must live for the
        // event loop's duration ; dropping it destroys the child hwnd.
        let _webview = webview;

        event_loop.run(move |event, _target, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::UserEvent(UserEvent::Dismiss) => {
                    tracing::info!("popup dismissed by user");
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }

    fn serve_custom_protocol(
        request: wry::http::Request<Vec<u8>>,
        resources_dir: &Path,
        rendered_popup_html: &str,
    ) -> wry::http::Response<Cow<'static, [u8]>> {
        let uri = request.uri();
        // Trim leading slash — uri().path() returns "/popup.html".
        let path = uri.path().trim_start_matches('/');
        if path == "popup.html" || path.is_empty() {
            return wry::http::Response::builder()
                .status(200)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Cow::Owned(rendered_popup_html.as_bytes().to_vec()))
                .unwrap_or_else(|_| empty_response());
        }
        let target = resources_dir.join(path);
        match std::fs::read(&target) {
            Ok(bytes) => wry::http::Response::builder()
                .status(200)
                .header("Content-Type", mime_for(path))
                .body(Cow::Owned(bytes))
                .unwrap_or_else(|_| empty_response()),
            Err(_) => wry::http::Response::builder()
                .status(404)
                .body(Cow::Owned(Vec::new()))
                .unwrap_or_else(|_| empty_response()),
        }
    }

    fn empty_response() -> wry::http::Response<Cow<'static, [u8]>> {
        wry::http::Response::new(Cow::Owned(Vec::new()))
    }

    fn mime_for(path: &str) -> &'static str {
        let ext = Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "html" => "text/html; charset=utf-8",
            "js" | "mjs" => "application/javascript; charset=utf-8",
            "wasm" => "application/wasm",
            "json" | "lottie" => "application/json",
            "css" => "text/css; charset=utf-8",
            "png" => "image/png",
            "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        }
    }

    fn resources_dir() -> Result<PathBuf> {
        let exe = std::env::current_exe().context("env::current_exe()")?;
        let dir = exe
            .parent()
            .context("exe has no parent")?
            .join("resources");
        Ok(dir)
    }

    fn rotation_dat_path() -> PathBuf {
        Path::new(DIAGNOSTICS_CACHE_DIR).join(ROTATION_DAT_FILENAME)
    }

    fn read_rotation_dat() -> Result<u32> {
        let path = rotation_dat_path();
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let arr: [u8; 4] = bytes
            .as_slice()
            .try_into()
            .context("rotation.dat is not 4 bytes")?;
        Ok(u32::from_le_bytes(arr))
    }

    fn write_rotation_dat(value: u32) -> Result<()> {
        let path = rotation_dat_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let tmp = path.with_extension("dat.tmp");
        std::fs::write(&tmp, value.to_le_bytes())
            .with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("rename to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnimationEntry, Config, RotationMode};

    fn anim(name: &str, enabled: bool) -> AnimationEntry {
        AnimationEntry {
            file: format!("animations/{name}.lottie"),
            enabled,
            display_name: name.to_string(),
        }
    }

    fn cfg_with_anims(anims: Vec<AnimationEntry>, mode: RotationMode) -> Config {
        let mut cfg = Config::default();
        cfg.animations = anims;
        cfg.rotation_mode = mode;
        cfg
    }

    #[test]
    fn sequential_advances() {
        let cfg = cfg_with_anims(
            vec![anim("a", true), anim("b", true), anim("c", true)],
            RotationMode::Sequential,
        );
        let (picked, next) = animation_pick(&cfg, Some(0)).expect("some");
        assert_eq!(picked.file, "animations/a.lottie");
        assert_eq!(next, 1);
    }

    #[test]
    fn sequential_wraps() {
        let cfg = cfg_with_anims(
            vec![anim("a", true), anim("b", true), anim("c", true)],
            RotationMode::Sequential,
        );
        let (picked, next) = animation_pick(&cfg, Some(5)).expect("some");
        // 5 % 3 == 2 → third entry (c)
        assert_eq!(picked.file, "animations/c.lottie");
        assert_eq!(next, 6);
    }

    #[test]
    fn random_picks_enabled_only() {
        let cfg = cfg_with_anims(
            vec![anim("a", true), anim("b", false), anim("c", true)],
            RotationMode::Random,
        );
        for _ in 0..100 {
            let (picked, _) = animation_pick(&cfg, None).expect("some");
            assert!(picked.file.ends_with("a.lottie") || picked.file.ends_with("c.lottie"));
        }
    }

    #[test]
    fn empty_list_returns_none() {
        let cfg = cfg_with_anims(vec![], RotationMode::Random);
        assert!(animation_pick(&cfg, None).is_none());
    }

    #[test]
    fn all_disabled_returns_none() {
        let cfg = cfg_with_anims(
            vec![anim("a", false), anim("b", false)],
            RotationMode::Sequential,
        );
        assert!(animation_pick(&cfg, Some(0)).is_none());
    }

    fn template() -> String {
        // Miniature template mirroring the popup.html placeholder shape.
        r#"<title>{{ WINDOW_TITLE }}</title>
<body data-remaining-seconds="{{ COUNTDOWN_SECONDS }}"
      data-animation-path="{{ ANIMATION_PATH }}"
      data-countdown-template="{{ COUNTDOWN_TEMPLATE }}"
      data-mode="{{ POPUP_MODE }}">
  <h1>{{ POPUP_TITLE }}</h1>
  <p>{{ POPUP_SUBTITLE }}</p>
  <button>{{ DISMISS_LABEL }}</button>
</body>"#
            .to_string()
    }

    #[test]
    fn all_placeholders_replaced() {
        let cfg = Config::default();
        let out = render_html(&template(), &cfg, "animations/foo.lottie", 300, false);
        assert!(!out.contains("{{"));
        assert!(!out.contains("}}"));
    }

    #[test]
    fn preserves_non_placeholder_curlies() {
        // The `{mm}` and `{ss}` markers inside countdown_template are consumed
        // by JS on the client, not by render_html — they must survive.
        let mut cfg = Config::default();
        cfg.countdown_template = "Pause dans {mm}:{ss}".to_string();
        let out = render_html(&template(), &cfg, "animations/foo.lottie", 300, false);
        assert!(out.contains("Pause dans {mm}:{ss}"));
    }

    #[test]
    fn preview_countdown_10s() {
        let cfg = Config::default();
        let out = render_html(&template(), &cfg, "animations/foo.lottie", 10, true);
        assert!(out.contains(r#"data-remaining-seconds="10""#));
        assert!(out.contains(r#"data-mode="preview""#));
    }

    #[test]
    fn window_title_replaced() {
        let mut cfg = Config::default();
        cfg.popup_window_title = "Windows Session Health".to_string();
        let out = render_html(&template(), &cfg, "animations/foo.lottie", 60, false);
        assert!(out.contains("<title>Windows Session Health</title>"));
        // And live mode should mark data-mode accordingly.
        assert!(out.contains(r#"data-mode="live""#));
    }
}

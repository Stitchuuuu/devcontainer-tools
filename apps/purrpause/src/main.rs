// Entry point for SystemHealthAgent (project codename: PurrPause).
//
// Session 1 delivers only the argv dispatcher — every mode is a stub that
// logs its label and exits. Later sessions replace each stub body with the
// real implementation per /workspace/plans/purrpause/ROLLOUT.md.
//
// Argv contract mirrors the "Modes du binaire (dispatch argv)" table in the
// design constitution. Keep this file in sync with STATUS.md as modes get
// fleshed out.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod ipc;
mod modes;
mod password;
mod platform;
mod scheduler;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = classify_mode(&args);
    init_tracing(&mode);

    tracing::info!(?mode, argv = ?args, "SystemHealthAgent starting");

    // Pre-flight : WebView2 runtime must be available before any UI mode.
    // The service / countdown / popup modes all end up needing wry, and
    // installing without WebView2 would leave a broken install. Bail
    // early with a native MessageBoxW dialog if it's missing.
    #[cfg(windows)]
    if mode_needs_webview2(&mode) {
        if let Err(err) = platform::win32::webview2::ensure_available() {
            tracing::error!(error = %err, "WebView2 pre-flight failed");
            return ExitCode::FAILURE;
        }
    }

    match dispatch(mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "mode handler failed");
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn mode_needs_webview2(mode: &Mode) -> bool {
    // Headless / cleanup paths never need WebView2 — the service runs
    // in Session 0, the watchdog exits after checking SCM state, and
    // rollback + uninstall are teardown. Everything else ends up in
    // the WebView2 dep graph one way or another (popup renders Lottie ;
    // the wizard opens an egui window that co-exists with wry).
    !matches!(
        mode,
        Mode::Service
            | Mode::Watchdog
            | Mode::RollbackFromFailedInstall
            | Mode::Uninstall
            | Mode::Countdown { .. }
    )
}

fn init_tracing(mode: &Mode) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    #[cfg(windows)]
    if matches!(mode, Mode::Service | Mode::Watchdog) {
        let log_dir = std::path::Path::new(modes::install::DIAGNOSTICS_CACHE_DIR);
        let _ = std::fs::create_dir_all(log_dir);
        let appender = tracing_appender::rolling::daily(log_dir, "service.log");
        let _ = fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_ansi(false)
            .with_writer(appender)
            .try_init();
        return;
    }

    // User-session UI modes (Countdown, Popup, Config) have no console
    // under windows_subsystem = "windows" — route to a file next to the
    // exe so tracing survives long enough to diagnose crashes. Falls
    // back to stderr on any error (dead pipe, but harmless).
    #[cfg(windows)]
    if matches!(
        mode,
        Mode::Countdown { .. } | Mode::Popup { .. } | Mode::Config { .. }
    ) {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let appender = tracing_appender::rolling::daily(dir, "widget.log");
                let _ = fmt()
                    .with_env_filter(filter)
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(appender)
                    .try_init();
                return;
            }
        }
    }

    let _ = mode; // silence unused-var on non-Windows
    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    /// Default: installer / update-path / launch config UI, decided at runtime.
    InstallOrConfig,
    /// Windows service entry point, invoked by SCM.
    Service,
    /// Fullscreen Lottie break popup, spawned by the service.
    /// `debug` skips the keyboard hook (Alt+F4 / Alt+Tab work) so a
    /// developer can exit the popup during smoke without killing the
    /// process from Task Manager.
    Popup { preview: bool, debug: bool },
    /// Small corner countdown widget for T-15 / T-10 / T-5 / T-1.
    /// `debug` skips force-minimize so the current window isn't lost.
    Countdown { seconds: u64, palier: u32, debug: bool },
    /// Passcode-gated config UI (egui window).
    Config { first_run: bool },
    /// Health-check task run by Scheduled Task every minute.
    Watchdog,
    /// Passcode-gated uninstall flow.
    Uninstall,
    /// Rollback partial install if wizard was cancelled.
    RollbackFromFailedInstall,
    /// Argv did not match any known mode.
    Unknown(Vec<String>),
}

fn classify_mode(args: &[String]) -> Mode {
    match args.first().map(String::as_str) {
        None => Mode::InstallOrConfig,
        Some("--service") => Mode::Service,
        Some("--popup") => Mode::Popup {
            preview: args.iter().any(|a| a == "--preview"),
            debug: args.iter().any(|a| a == "--debug"),
        },
        Some("--countdown") => parse_countdown(args),
        Some("--config") => Mode::Config {
            first_run: args.iter().any(|a| a == "--first-run"),
        },
        Some("--watchdog") => Mode::Watchdog,
        Some("--uninstall") => Mode::Uninstall,
        Some("--rollback-from-failed-install") => Mode::RollbackFromFailedInstall,
        Some(_) => Mode::Unknown(args.to_vec()),
    }
}

fn parse_countdown(args: &[String]) -> Mode {
    // Shape: --countdown <secs> --palier <15|10|5|1> [--debug]
    let seconds = args
        .get(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let palier = args
        .iter()
        .position(|a| a == "--palier")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let debug = args.iter().any(|a| a == "--debug");
    Mode::Countdown { seconds, palier, debug }
}

/// User-facing stopgap for modes not yet implemented. Under
/// `windows_subsystem = "windows"` the exe has no console, so `println!`
/// is invisible — a MessageBox is the only way the user sees anything.
#[cfg(windows)]
fn not_yet_available_dialog(title: &str, body: &str) {
    use std::iter::once;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_OK,
    };
    let title_w: Vec<u16> = title.encode_utf16().chain(once(0)).collect();
    let body_w: Vec<u16> = body.encode_utf16().chain(once(0)).collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[cfg(not(windows))]
fn not_yet_available_dialog(title: &str, body: &str) {
    eprintln!("[{title}] {body}");
}

fn dispatch(mode: Mode) -> anyhow::Result<()> {
    match mode {
        Mode::InstallOrConfig => modes::install::run()?,
        Mode::RollbackFromFailedInstall => modes::rollback::run()?,
        #[cfg(windows)]
        Mode::Config { first_run: true } => modes::config_first_run::run()?,
        Mode::Service => modes::service::run()?,
        Mode::Watchdog => modes::watchdog::run()?,
        Mode::Popup { preview, debug } => modes::popup::run(preview, debug)?,
        Mode::Countdown { seconds, palier, debug } => {
            modes::countdown::run(seconds, palier, debug)?
        }
        Mode::Config { first_run } => {
            not_yet_available_dialog(
                "Paramètres",
                "L'écran de configuration arrive dans une prochaine version.\n\nEn attendant, tu peux modifier le passcode ou désinstaller en supprimant C:\\ProgramData\\DiagnosticsCache\\state.dat puis en relançant l'application.",
            );
            let _ = first_run;
        }
        Mode::Uninstall => {
            not_yet_available_dialog(
                "Désinstallation",
                "La désinstallation intégrée arrive dans une prochaine version.\n\nProcédure manuelle en attendant :\n1. Ouvrir services.msc et arrêter « Windows Session Health Service ».\n2. Ouvrir taskschd.msc et supprimer la tâche \\Microsoft\\Windows\\SystemHealth\\HealthCheck.\n3. Supprimer C:\\ProgramData\\DiagnosticsCache\\ (nécessite droits admin).",
            );
        }
        Mode::Unknown(argv) => {
            eprintln!("Unknown argv: {argv:?}");
            eprintln!("Valid modes: --service | --popup [--preview] | --countdown <secs> --palier <15|10|5> | --config [--first-run] | --watchdog | --uninstall | --rollback-from-failed-install");
            anyhow::bail!("unknown mode");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_args<const N: usize>(a: [&str; N]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_is_install_or_config() {
        assert_eq!(classify_mode(&[]), Mode::InstallOrConfig);
    }

    #[test]
    fn service_flag() {
        assert_eq!(classify_mode(&as_args(["--service"])), Mode::Service);
    }

    #[test]
    fn popup_with_preview() {
        assert_eq!(
            classify_mode(&as_args(["--popup", "--preview"])),
            Mode::Popup { preview: true, debug: false }
        );
    }

    #[test]
    fn popup_without_preview() {
        assert_eq!(
            classify_mode(&as_args(["--popup"])),
            Mode::Popup { preview: false, debug: false }
        );
    }

    #[test]
    fn popup_with_debug() {
        assert_eq!(
            classify_mode(&as_args(["--popup", "--debug"])),
            Mode::Popup { preview: false, debug: true }
        );
    }

    #[test]
    fn countdown_parsed() {
        assert_eq!(
            classify_mode(&as_args(["--countdown", "900", "--palier", "15"])),
            Mode::Countdown { seconds: 900, palier: 15, debug: false }
        );
    }

    #[test]
    fn countdown_with_debug() {
        assert_eq!(
            classify_mode(&as_args(["--countdown", "60", "--palier", "1", "--debug"])),
            Mode::Countdown { seconds: 60, palier: 1, debug: true }
        );
    }

    #[test]
    fn config_first_run() {
        assert_eq!(
            classify_mode(&as_args(["--config", "--first-run"])),
            Mode::Config { first_run: true }
        );
    }

    #[test]
    fn unknown_flag() {
        let m = classify_mode(&as_args(["--garbage"]));
        assert!(matches!(m, Mode::Unknown(_)));
    }
}

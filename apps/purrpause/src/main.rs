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

use std::process::ExitCode;

fn main() -> ExitCode {
    init_tracing();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = classify_mode(&args);

    tracing::info!(?mode, argv = ?args, "SystemHealthAgent starting");

    match dispatch(mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "mode handler failed");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
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
    Popup { preview: bool },
    /// Small corner countdown widget for T-15 / T-10 / T-5.
    Countdown { seconds: u64, palier: u32 },
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
    // Shape: --countdown <secs> --palier <15|10|5>
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
    Mode::Countdown { seconds, palier }
}

fn dispatch(mode: Mode) -> anyhow::Result<()> {
    match mode {
        Mode::InstallOrConfig => {
            println!("[stub] install-or-config mode (session 2 wires this)");
        }
        Mode::Service => {
            println!("[stub] service mode (session 3 wires this)");
        }
        Mode::Popup { preview } => {
            println!(
                "[stub] popup mode (session 4 wires this) — preview={preview}"
            );
        }
        Mode::Countdown { seconds, palier } => {
            println!(
                "[stub] countdown mode (session 5 wires this) — seconds={seconds} palier={palier}"
            );
        }
        Mode::Config { first_run } => {
            println!(
                "[stub] config-ui mode (session 6 wires this) — first_run={first_run}"
            );
        }
        Mode::Watchdog => {
            println!("[stub] watchdog mode (session 7 wires this)");
        }
        Mode::Uninstall => {
            println!("[stub] uninstall mode (session 7 wires this)");
        }
        Mode::RollbackFromFailedInstall => {
            println!("[stub] rollback-from-failed-install mode (session 2 wires this)");
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
            Mode::Popup { preview: true }
        );
    }

    #[test]
    fn popup_without_preview() {
        assert_eq!(
            classify_mode(&as_args(["--popup"])),
            Mode::Popup { preview: false }
        );
    }

    #[test]
    fn countdown_parsed() {
        assert_eq!(
            classify_mode(&as_args(["--countdown", "900", "--palier", "15"])),
            Mode::Countdown { seconds: 900, palier: 15 }
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

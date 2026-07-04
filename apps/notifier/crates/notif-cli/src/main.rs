//! `notif` — cross-platform notification CLI.
//!
//! v0.1 delivers the macOS backend (Tier 0 + Tier 2). Windows and Linux ship
//! stubs that will be filled in v0.3 and v0.4 respectively.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "notif", version, about = "Cross-platform notification CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dispatch a notification. All register-time metadata (`--name`,
    /// `--icon`, `--identifier`) is accepted inline — if the sender bundle
    /// does not exist yet, it is auto-materialized before dispatch. Subsequent
    /// sends ignore these flags (the bundle is already registered).
    Send {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
        #[arg(long)]
        subtitle: Option<String>,
        /// Sender key. Defaults to the reserved `"default"` (Tier 0).
        #[arg(long)]
        sender: Option<String>,
        /// Display name for the sender (used only if the bundle is being
        /// auto-created by this call).
        #[arg(long)]
        name: Option<String>,
        /// Path to a `.icns` icon (used only on first-time auto-create).
        #[arg(long)]
        icon: Option<std::path::PathBuf>,
        /// `CFBundleIdentifier` override — Tier 1 spoof (e.g.
        /// `com.microsoft.VSCode`). Applied only on first-time auto-create.
        #[arg(long)]
        identifier: Option<String>,
        /// Auto-resolve an installed app (Spotlight) and use its identifier,
        /// name, and icon. Hint can be either a `CFBundleIdentifier`
        /// (`com.microsoft.VSCode`) or a display name (`"Visual Studio Code"`).
        /// Explicit `--name` / `--icon` / `--identifier` still override.
        #[arg(long)]
        app: Option<String>,
    },
    /// Register a Tier 2 custom sender. Idempotent unless the display name
    /// differs from a pre-existing registration.
    Register {
        #[arg(long)]
        sender: String,
        #[arg(long)]
        name: String,
        /// Path to a `.icns` icon file to embed in the bundle. Optional —
        /// without it, the sender uses the macOS generic application icon.
        #[arg(long)]
        icon: Option<std::path::PathBuf>,
        /// Tier 1 spoof — override `CFBundleIdentifier` with a real app's
        /// identifier (e.g. `com.microsoft.VSCode`). Defaults to
        /// `com.notify.<sender>`. Behavior with a foreign
        /// identifier is macOS-version-dependent — see LOG for observations.
        #[arg(long)]
        identifier: Option<String>,
    },
    /// Materialize the sender bundle and trigger the macOS notification
    /// authorization prompt.
    Setup {
        #[arg(long)]
        sender: Option<String>,
    },
    /// List every materialized sender bundle with its key, display name,
    /// identifier, and on-disk path.
    Senders,
}

#[cfg(not(target_os = "macos"))]
const HOST: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "linux") {
    "linux"
} else {
    "unknown"
};

fn main() -> Result<()> {
    let cli = Cli::parse();

    #[cfg(target_os = "macos")]
    return run_macos(cli.command);

    #[cfg(not(target_os = "macos"))]
    return run_stub(cli.command);
}

// ---- macOS -----------------------------------------------------------------

#[cfg(target_os = "macos")]
fn run_macos(cmd: Command) -> Result<()> {
    use anyhow::{bail, Context};
    use notif_core::{Backend, Notification, Priority, Sender};
    use notif_macos::dispatch::{dispatch_inner, is_inner_mode, setup_inner, setup_outer};
    use notif_macos::sender::DEFAULT_KEY;
    use notif_macos::{bundle, MacosBackend};

    match cmd {
        Command::Send {
            title,
            body,
            subtitle,
            sender,
            name,
            icon,
            identifier,
            app,
        } => {
            // Resolve --app once so the resolved metadata can seed sender,
            // name, and icon defaults if the caller omitted the explicit
            // flags.
            let (auto_name, auto_icon, suggested_key) =
                resolve_app_metadata(app.as_deref())?;

            // Effective sender key: explicit --sender wins, else derive from
            // the app-resolved name, else fall back to the reserved default.
            let effective_sender_key = sender
                .or(suggested_key)
                .unwrap_or_else(|| "default".to_string());
            let sender_obj = notif_core::Sender::new(effective_sender_key.clone())
                .context("invalid sender key")?;
            let notif = Notification {
                title,
                body,
                subtitle,
                priority: Priority::Normal,
                sender: sender_obj,
            };

            if is_inner_mode() {
                match dispatch_inner(&notif) {
                    Ok(()) => Ok(()),
                    Err(notif_macos::MacosError::NotSigned) => {
                        std::process::exit(42);
                    }
                    Err(e) => Err(e).context("inner dispatch failed"),
                }
            } else {
                let existing = notif_macos::sender::find_bundle_by_key(&notif.sender.key)?;
                let is_first_run = existing.is_none();
                if is_first_run {
                    eprintln!("first run: initializing bundle + requesting permission…");

                    // Tag with ` · Notify` only when the display name is
                    // *borrowed* from an installed app (`--app` resolved a
                    // name, user did not override with `--name`). An explicit
                    // `--name` — even alongside `--app` — is a full override
                    // and ships verbatim, matching the "user always wins"
                    // principle.
                    let borrowed = name.is_none() && auto_name.is_some();
                    let base_name = name
                        .or(auto_name)
                        .unwrap_or_else(|| notif.sender.key.clone());
                    let effective_name = if borrowed && notif.sender.key != "default" {
                        format!("{base_name} \u{00B7} Notify")
                    } else {
                        base_name
                    };

                    let icon_path = icon.or(auto_icon);
                    let icon_bytes = match icon_path.as_deref() {
                        None => None,
                        Some(p) => Some(std::fs::read(p).with_context(|| {
                            format!("read icon {}", p.display())
                        })?),
                    };

                    // Explicit-only spoof gate: `--identifier` propagates,
                    // `--app` never surfaces the resolved app's identifier.
                    let effective_id = identifier;
                    refuse_apple_identifier(effective_id.as_deref())?;

                    bundle::ensure_bundle(
                        &notif.sender.key,
                        &effective_name,
                        icon_bytes.as_deref(),
                        effective_id.as_deref(),
                    )?;
                }

                // Always fire setup before dispatch — `requestAuthorization`
                // is idempotent (no dialog after the first grant/deny) and
                // remains the only way to detect an externally-reset TCC
                // entry (user reset the app's notif permission in System
                // Settings). Without this check, UN center silently drops
                // the send when permission is denied.
                if is_first_run {
                    eprintln!("waiting for permission dialog (click 'Allow' within 60s)…");
                }
                let display_hint = existing
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| notif.sender.key.clone());
                match setup_outer(&notif.sender.key) {
                    Ok(()) => {
                        if is_first_run {
                            eprintln!("permission granted.");
                        }
                    }
                    Err(notif_macos::MacosError::AuthorizationDenied) => {
                        bail!(
                            "notification permission denied for '{display_hint}' — enable it in System Settings > Notifications, then retry"
                        );
                    }
                    Err(e) => return Err(e).context("authorization check"),
                }

                eprintln!("sending notification via '{}'…", notif.sender.key);
                MacosBackend.dispatch(&notif).context("dispatch")?;
                eprintln!("sent.");
                Ok(())
            }
        }
        Command::Register { sender, name, icon, identifier } => {
            if sender == DEFAULT_KEY {
                bail!("'{DEFAULT_KEY}' is reserved and cannot be registered");
            }
            refuse_apple_identifier(identifier.as_deref())?;
            let s = Sender::new(sender.clone()).context("invalid sender key")?;
            let icon_bytes = match icon.as_deref() {
                None => None,
                Some(p) => Some(std::fs::read(p).with_context(|| {
                    format!("read icon {}", p.display())
                })?),
            };
            let path = bundle::ensure_bundle(
                &s.key,
                &name,
                icon_bytes.as_deref(),
                identifier.as_deref(),
            )?;
            // Fire the macOS permission dialog under the newly-registered
            // bundle identity so subsequent `send` calls have a granted auth
            // state — otherwise UN center silently drops.
            setup_outer(&s.key).context("register: request authorization")?;
            println!("registered {} at {}", s.key, path.display());
            Ok(())
        }
        Command::Senders => {
            let rows = notif_macos::sender::list_senders()?;
            if rows.is_empty() {
                println!("(no senders registered)");
            } else {
                println!("{:<24} {:<32} {:<40} FOLDER", "KEY", "DISPLAY", "IDENTIFIER");
                for s in rows {
                    println!(
                        "{key:<24} {display:<32} {id:<40} {folder}",
                        key = s.key,
                        display = s.display,
                        id = s.identifier,
                        folder = s.folder,
                    );
                }
            }
            Ok(())
        }
        Command::Setup { sender } => {
            let sender = build_sender(sender.as_deref())?;
            if is_inner_mode() {
                // Inner-mode exit conventions consumed by
                // `notif-macos::dispatch::run_open`:
                //   42 → MacosError::NotSigned
                //   43 → MacosError::AuthorizationDenied
                match setup_inner() {
                    Ok(()) => Ok(()),
                    Err(notif_macos::MacosError::NotSigned) => std::process::exit(42),
                    Err(notif_macos::MacosError::AuthorizationDenied) => {
                        std::process::exit(43)
                    }
                    Err(e) => Err(e).context("inner setup failed"),
                }
            } else {
                setup_outer(&sender.key).context("setup")
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn build_sender(raw: Option<&str>) -> Result<notif_core::Sender> {
    use anyhow::Context;
    match raw {
        None => Ok(notif_core::Sender::default()),
        Some(k) => notif_core::Sender::new(k.to_string()).context("invalid sender key"),
    }
}

/// Resolve `--app <hint>` to (name, icon-path, suggested-sender-key).
///
/// **Never** surfaces the resolved app's `CFBundleIdentifier` — `--app` is a
/// pure cosmetic shortcut ("look like this app"). Callers who want to
/// actually spoof the identity must pass `--identifier` explicitly. Keeps
/// impersonation opt-in and side-steps the SIP-tier LSError -10664 that
/// macOS triggers on `com.apple.*` spoofs.
#[cfg(target_os = "macos")]
#[allow(clippy::type_complexity)]
fn resolve_app_metadata(
    hint: Option<&str>,
) -> Result<(Option<String>, Option<std::path::PathBuf>, Option<String>)> {
    use anyhow::Context;
    match hint {
        None => Ok((None, None, None)),
        Some(h) => {
            let resolved = notif_macos::sender::resolve_app_hint(h)
                .with_context(|| format!("resolve app {h:?}"))?;
            let suggested_key = sanitize_sender_key(&resolved.display_name);
            Ok((Some(resolved.display_name), resolved.icon_path, Some(suggested_key)))
        }
    }
}

/// Refuse a `com.apple.*` identifier before any bundle materialization.
///
/// macOS treats Apple-owned identifier prefixes as SIP-tier — LaunchServices
/// returns `kLSNoLaunchPermissionErr` (LSError -10664) when we later try to
/// launch the bundle. Failing here avoids the wasted materialization +
/// orphaned bundle folder that would otherwise sit around confusing LSDB.
#[cfg(target_os = "macos")]
fn refuse_apple_identifier(id: Option<&str>) -> Result<()> {
    use anyhow::bail;
    if let Some(id) = id {
        if id.starts_with("com.apple.") {
            bail!(
                "cannot spoof Apple-owned identifier {id:?} — macOS refuses to launch bundles impersonating com.apple.* (LSError -10664). Use a non-Apple identifier."
            );
        }
    }
    Ok(())
}

/// Turn a display name into a valid sender key.
///
/// Lowercases, replaces non-`[a-z0-9_-]` runs with a single `-`, trims
/// leading/trailing hyphens, and truncates to 32 bytes so the result passes
/// [`notif_core::validate_sender_key`].
///
/// Examples: `"Visual Studio Code"` → `"visual-studio-code"`,
/// `"Safari"` → `"safari"`.
#[cfg(target_os = "macos")]
fn sanitize_sender_key(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep && !out.is_empty() {
            out.push('-');
            last_was_sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.chars().take(32).collect()
}

// ---- Non-mac dev stub ------------------------------------------------------

#[cfg(not(target_os = "macos"))]
fn run_stub(cmd: Command) -> Result<()> {
    match cmd {
        Command::Send { title, body, subtitle, sender, name, icon, identifier, app } => {
            let sender = sender.unwrap_or_else(|| "default".to_string());
            let sub = subtitle.unwrap_or_default();
            let n = name.unwrap_or_default();
            let i = icon.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let id = identifier.unwrap_or_default();
            let a = app.unwrap_or_default();
            println!(
                "[stub] would dispatch: title={title}, body={body}, subtitle={sub}, sender={sender}, name={n}, icon={i}, identifier={id}, app={a}, host={HOST}",
            );
        }
        Command::Register { sender, name, icon, identifier } => {
            let icon_str = icon.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let id_str = identifier.unwrap_or_default();
            println!(
                "[stub] would register: sender={sender}, name={name}, icon={icon_str}, identifier={id_str}, host={HOST}"
            );
        }
        Command::Setup { sender } => {
            let sender = sender.unwrap_or_else(|| "default".to_string());
            println!("[stub] would setup: sender={sender}, host={HOST}");
        }
        Command::Senders => {
            println!("[stub] would list senders, host={HOST}");
        }
    }
    Ok(())
}

//! `notif` — cross-platform notification CLI.
//!
//! v0.1 delivers the macOS backend (Tier 0 + Tier 2). Windows and Linux ship
//! stubs that will be filled in v0.3 and v0.4 respectively.

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "notif", version, about = "Cross-platform notification CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Suppress `warning:` lines on stderr. Errors still print. Equivalent to
    /// setting `NOTIF_QUIET=1` — either wins.
    #[arg(long, global = true)]
    quiet: bool,
}

/// Portable priority. Round-trips through `Priority::wire_str` on the
/// outer→inner CLI hop.
#[derive(Copy, Clone, ValueEnum)]
enum PriorityArg {
    Low,
    Normal,
    High,
    Critical,
}

#[cfg(target_os = "macos")]
impl PriorityArg {
    fn to_core(self) -> notif_core::Priority {
        use notif_core::Priority;
        match self {
            Self::Low => Priority::Low,
            Self::Normal => Priority::Normal,
            Self::High => Priority::High,
            Self::Critical => Priority::Critical,
        }
    }
}

/// Portable timeout behavior. Kebab-case (`log-only`) is the CLI form.
#[derive(Copy, Clone, ValueEnum)]
enum OnTimeoutArg {
    LogOnly,
    Dismiss,
    Persist,
}

#[cfg(target_os = "macos")]
impl OnTimeoutArg {
    fn to_core(self) -> notif_core::TimeoutBehavior {
        use notif_core::TimeoutBehavior as T;
        match self {
            Self::LogOnly => T::LogOnly,
            Self::Dismiss => T::Dismiss,
            Self::Persist => T::Persist,
        }
    }
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
        /// Path to a `.icns` icon file (used only on first-time auto-create).
        /// For an installed app's icon, prefer `--app <hint>` which resolves
        /// it via Spotlight without needing to know the path.
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
        /// Interruption level. Maps to `UNNotificationInterruptionLevel` on
        /// macOS. Absent → OS default (`Active`).
        #[arg(long, value_enum)]
        priority: Option<PriorityArg>,
        /// Sound to play on delivery. `default` / `alert` are keywords;
        /// anything else is a bundled sound name or an absolute path to a
        /// sound file. Absent → silent (macOS default for `Active`).
        #[arg(long)]
        sound: Option<String>,
        /// Path to an inline image / attachment. Must exist and end with
        /// `.png`, `.jpg`, `.jpeg`, or `.gif`.
        #[arg(long, value_parser = parse_image)]
        image: Option<std::path::PathBuf>,
        /// Per-notification identifier. Reused as
        /// `UNNotificationRequest.identifier` on macOS. Absent → random
        /// UUID.
        #[arg(long)]
        id: Option<String>,
        /// What to do when the OS auto-dismisses the banner. macOS in v0.1
        /// has no native equivalent — the flag is accepted and a
        /// suppressible `warning:` line is emitted (see `--quiet`).
        #[arg(long, value_enum)]
        on_timeout: Option<OnTimeoutArg>,
        /// Parse and print the resolved notification as human-readable
        /// key: value pairs on stdout, then exit 0. Does not materialize the
        /// sender bundle and does not call the notification center. `--app`
        /// is still resolved via Spotlight so the printed metadata reflects
        /// what a real send would use.
        #[arg(long)]
        dry_run: bool,
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
    /// Unregister and remove sender bundles — resets their notification
    /// permission via `tccutil` and deletes the bundle folder.
    Clean {
        /// Clean a single sender by key.
        #[arg(long, conflicts_with = "all", required_unless_present = "all")]
        sender: Option<String>,
        /// Clean every materialized bundle (prompts unless `--yes`).
        #[arg(long, conflicts_with = "sender", required_unless_present = "sender")]
        all: bool,
        /// Skip the confirmation prompt for `--all`.
        #[arg(long)]
        yes: bool,
    },
    /// Swap the icon on an already-materialized sender bundle. Refuses the
    /// reserved `default` sender — its icon is embedded at compile time and
    /// only changes with a `notif` rebuild.
    SetIcon {
        /// Sender key whose bundle icon should be replaced.
        #[arg(long)]
        sender: String,
        /// Path to a `.icns` file. Read verbatim into
        /// `Contents/Resources/icon.icns` ; the bundle is re-signed
        /// ad-hoc so LaunchServices picks up the change.
        #[arg(long)]
        icon: std::path::PathBuf,
    },
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

    let env_quiet = std::env::var("NOTIF_QUIET").ok().as_deref() == Some("1");
    notif_core::warn::init(notif_core::warn::WarnConfig {
        quiet: cli.quiet || env_quiet,
    });

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
    use notif_macos::dispatch::{
        dispatch_inner, is_inner_mode, setup_inner, setup_outer, setup_outer_bootstrap,
    };
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
            priority,
            sound,
            image,
            id,
            on_timeout,
            dry_run,
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
                priority: priority.map_or(Priority::Normal, PriorityArg::to_core),
                sender: sender_obj,
                id,
                sound: sound.as_deref().map(parse_sound),
                image,
                on_timeout: on_timeout.map(OnTimeoutArg::to_core),
            };

            if dry_run {
                // Print the resolved notification and exit before any bundle
                // materialization or UN center call. `resolve_app_metadata`
                // has already run (Spotlight-only, read-only) so the app
                // hint round-trips into the output.
                print!("{}", format_dry_run(&notif, app.as_deref(), auto_name.as_deref()));
                return Ok(());
            }

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
                //
                // First run uses `setup_outer_bootstrap` which seeds LSDB
                // via `lsregister -f` — required for the permission dialog
                // to actually show on a brand-new sender (session-3's
                // direct-spawn skips LSDB, and UN center refuses unknown
                // identifiers with UNErrorCode 1). Subsequent sends use
                // the fast direct-spawn path via `setup_outer`.
                if is_first_run {
                    eprintln!("waiting for permission dialog (click 'Allow' within 60s)…");
                }
                let display_hint = existing
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| notif.sender.key.clone());
                let setup_result = if is_first_run {
                    setup_outer_bootstrap(&notif.sender.key)
                } else {
                    setup_outer(&notif.sender.key)
                };
                match setup_result {
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
        Command::Clean { sender, all, yes } => run_clean(sender.as_deref(), all, yes),
        Command::SetIcon { sender, icon } => {
            if sender == DEFAULT_KEY {
                bail!(
                    "'{DEFAULT_KEY}' is reserved — its icon is embedded at compile time. Rebuild `notif` after replacing assets/notify.icns to change it."
                );
            }
            let s = Sender::new(sender.clone()).context("invalid sender key")?;
            let path = bundle::set_bundle_icon(&s.key, &icon).with_context(|| {
                format!("set icon for sender {:?} from {}", s.key, icon.display())
            })?;
            println!("icon updated for sender {} at {}", s.key, path.display());
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn run_clean(sender: Option<&str>, all: bool, yes: bool) -> Result<()> {
    use anyhow::{bail, Context};
    match (sender, all) {
        (Some(key), false) => {
            let report = notif_macos::clean::clean_sender(key)
                .with_context(|| format!("clean sender {key:?}"))?;
            print_clean(&report);
            Ok(())
        }
        (None, true) => {
            let reports = notif_macos::clean::clean_all(yes).context("clean --all")?;
            if reports.is_empty() {
                println!("(nothing to clean)");
                return Ok(());
            }
            for r in &reports {
                print_clean(r);
            }
            Ok(())
        }
        _ => bail!("clean requires either --sender <KEY> or --all"),
    }
}

#[cfg(target_os = "macos")]
fn print_clean(r: &notif_macos::clean::CleanReport) {
    let removal = if r.bundle_removed { "removed" } else { "absent" };
    println!(
        "{key:<24} {id:<40} {tcc:<24} {removal:<8} {path}",
        key = r.key,
        id = r.identifier,
        tcc = format!("{}", r.tcc_reset),
        removal = removal,
        path = r.bundle_path.display(),
    );
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

/// Value parser for `--image`. Rejects at parse time to keep the failure
/// path deterministic (no half-materialized bundle, no half-built UN
/// request). Extensions checked case-insensitively.
fn parse_image(s: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(s);
    if !p.exists() {
        return Err(format!("file not found: {}", p.display()));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif") => Ok(p),
        Some(other) => Err(format!("expected .png / .jpg / .gif, got .{other}")),
        None => Err("expected an image file with a .png / .jpg / .gif extension".into()),
    }
}

/// Parse a raw `--sound` value. `default` / `alert` are keywords; anything
/// else is treated as a custom sound (bundled name or filesystem path,
/// disambiguated at dispatch time).
#[cfg(target_os = "macos")]
fn parse_sound(raw: &str) -> notif_core::Sound {
    use notif_core::Sound;
    match raw {
        "default" => Sound::Default,
        "alert" => Sound::Alert,
        other => Sound::Custom(other.to_string()),
    }
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

/// Format the resolved notification as human-readable `key: value` lines for
/// `--dry-run`. Portable — reuses `wire_str()` on `Priority`, `Sound`, and
/// `TimeoutBehavior` so the output matches the outer→inner CLI wire format
/// exactly. Ends with a trailing newline.
///
/// Kept as a pure `String`-returning fn so `cli_tests` can snapshot the
/// output without capturing stdout.
///
/// `cfg(any(target_os = "macos", test))` — the outer entry point at
/// `Command::Send` in `run_macos` is the only production caller ; the test
/// suite exercises the pure form directly. Non-macOS release builds don't
/// need it (the stub prints its own line).
#[cfg(any(target_os = "macos", test))]
fn format_dry_run(
    notif: &notif_core::Notification,
    app_hint: Option<&str>,
    app_resolved_name: Option<&str>,
) -> String {
    fn opt(v: Option<&str>) -> String {
        v.map_or_else(|| "<none>".to_string(), str::to_string)
    }
    let subtitle = opt(notif.subtitle.as_deref());
    let sound = notif
        .sound
        .as_ref()
        .map_or_else(|| "<none>".to_string(), |s| s.wire_str().to_string());
    let image = notif
        .image
        .as_ref()
        .map_or_else(|| "<none>".to_string(), |p| p.display().to_string());
    let id = opt(notif.id.as_deref());
    let on_timeout = notif
        .on_timeout
        .map_or_else(|| "<none>".to_string(), |t| t.wire_str().to_string());
    let app = opt(app_hint);
    let app_resolved_name = opt(app_resolved_name);
    format!(
        "title: {title}\n\
         body: {body}\n\
         subtitle: {subtitle}\n\
         priority: {priority}\n\
         sender: {sender}\n\
         sound: {sound}\n\
         image: {image}\n\
         id: {id}\n\
         on_timeout: {on_timeout}\n\
         app: {app}\n\
         app_resolved_name: {app_resolved_name}\n",
        title = notif.title,
        body = notif.body,
        priority = notif.priority.wire_str(),
        sender = notif.sender.key,
    )
}

// ---- Non-mac dev stub ------------------------------------------------------

#[cfg(not(target_os = "macos"))]
fn run_stub(cmd: Command) -> Result<()> {
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
            priority,
            sound,
            image,
            id,
            on_timeout,
            dry_run,
        } => {
            let sender = sender.unwrap_or_else(|| "default".to_string());
            let sub = subtitle.unwrap_or_default();
            let n = name.unwrap_or_default();
            let i = icon.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let bid = identifier.unwrap_or_default();
            let a = app.unwrap_or_default();
            let prio = priority.map_or("<none>", |p| match p {
                PriorityArg::Low => "low",
                PriorityArg::Normal => "normal",
                PriorityArg::High => "high",
                PriorityArg::Critical => "critical",
            });
            let snd = sound.unwrap_or_default();
            let img = image.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let nid = id.unwrap_or_default();
            let ot = on_timeout.map_or("<none>", |t| match t {
                OnTimeoutArg::LogOnly => "log-only",
                OnTimeoutArg::Dismiss => "dismiss",
                OnTimeoutArg::Persist => "persist",
            });
            let dr = if dry_run { "true" } else { "false" };
            println!(
                "[stub] would dispatch: title={title}, body={body}, subtitle={sub}, sender={sender}, name={n}, icon={i}, identifier={bid}, app={a}, priority={prio}, sound={snd}, image={img}, id={nid}, on_timeout={ot}, dry_run={dr}, host={HOST}",
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
        Command::Clean { sender, all, yes } => {
            let s = sender.unwrap_or_default();
            println!("[stub] would clean sender={s}, all={all}, yes={yes}, host={HOST}");
        }
        Command::SetIcon { sender, icon } => {
            println!(
                "[stub] would set-icon sender={sender}, icon={}, host={HOST}",
                icon.display(),
            );
        }
    }
    Ok(())
}

// ---- CLI parsing tests ------------------------------------------------------
//
// clap catches most argument-shape bugs at compile time via `#[derive(Parser)]`,
// but positional-vs-flag and group-required-mutex constraints are runtime.
// These tests lock the CLI surface : any renamed / dropped / re-shaped flag
// will fail the corresponding case, so regressions land in `cargo test`
// output instead of surfacing to the user at runtime.

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::Parser;

    fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
        let mut full = vec!["notif"];
        full.extend_from_slice(argv);
        Cli::try_parse_from(full)
    }

    #[test]
    fn send_minimal() {
        let cli = parse(&["send", "--title", "T", "--body", "B"]).unwrap();
        assert!(matches!(cli.command, Command::Send { .. }));
    }

    #[test]
    fn send_icon_path_accepted() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--icon", "/Applications/Foo.app/Contents/Resources/Bar.icns",
        ])
        .unwrap();
        match cli.command {
            Command::Send { icon, .. } => assert_eq!(
                icon.as_deref().and_then(|p| p.to_str()),
                Some("/Applications/Foo.app/Contents/Resources/Bar.icns"),
            ),
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn clean_sender_accepted() {
        let cli = parse(&["clean", "--sender", "vscode"]).unwrap();
        match cli.command {
            Command::Clean { sender, all, yes } => {
                assert_eq!(sender.as_deref(), Some("vscode"));
                assert!(!all);
                assert!(!yes);
            }
            _ => panic!("expected Clean"),
        }
    }

    #[test]
    fn clean_all_accepted() {
        let cli = parse(&["clean", "--all"]).unwrap();
        assert!(matches!(cli.command, Command::Clean { all: true, .. }));
    }

    #[test]
    fn clean_all_yes_accepted() {
        let cli = parse(&["clean", "--all", "--yes"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Clean { all: true, yes: true, .. },
        ));
    }

    #[test]
    fn clean_no_args_rejected() {
        // Regression : without `required_unless_present` the empty invocation
        // parsed successfully and failed only downstream in the mac handler.
        assert!(parse(&["clean"]).is_err());
    }

    #[test]
    fn clean_sender_and_all_mutually_exclusive() {
        assert!(parse(&["clean", "--sender", "x", "--all"]).is_err());
    }

    #[test]
    fn set_icon_minimal() {
        let cli = parse(&[
            "set-icon", "--sender", "vscode", "--icon", "/tmp/notify.icns",
        ])
        .unwrap();
        match cli.command {
            Command::SetIcon { sender, icon } => {
                assert_eq!(sender, "vscode");
                assert_eq!(icon.to_str(), Some("/tmp/notify.icns"));
            }
            _ => panic!("expected SetIcon"),
        }
    }

    #[test]
    fn set_icon_requires_sender() {
        assert!(parse(&["set-icon", "--icon", "/tmp/notify.icns"]).is_err());
    }

    #[test]
    fn set_icon_requires_icon() {
        assert!(parse(&["set-icon", "--sender", "vscode"]).is_err());
    }

    #[test]
    fn set_icon_default_sender_parses_but_refused_at_runtime() {
        // clap does not know about the reserved-key contract — parse succeeds
        // and the mac handler bails with a clear message. Locking the parse
        // shape here so the runtime refusal stays reachable.
        let cli = parse(&[
            "set-icon", "--sender", "default", "--icon", "/tmp/notify.icns",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::SetIcon { .. }));
    }

    #[test]
    fn send_priority_variants() {
        for v in ["low", "normal", "high", "critical"] {
            let cli = parse(&["send", "--title", "T", "--body", "B", "--priority", v])
                .unwrap_or_else(|e| panic!("--priority {v} rejected: {e}"));
            match cli.command {
                Command::Send { priority: Some(_), .. } => {}
                _ => panic!("expected Send with priority set"),
            }
        }
    }

    #[test]
    fn send_priority_invalid_rejected() {
        assert!(parse(&["send", "--title", "T", "--body", "B", "--priority", "urgent"]).is_err());
    }

    #[test]
    fn send_sound_accepted() {
        for v in ["default", "alert", "Ping", "/System/Library/Sounds/Glass.aiff"] {
            let cli = parse(&["send", "--title", "T", "--body", "B", "--sound", v]).unwrap();
            assert!(matches!(cli.command, Command::Send { sound: Some(_), .. }));
        }
    }

    #[test]
    fn send_id_accepted() {
        let cli = parse(&["send", "--title", "T", "--body", "B", "--id", "abc-123"]).unwrap();
        match cli.command {
            Command::Send { id, .. } => assert_eq!(id.as_deref(), Some("abc-123")),
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn send_on_timeout_variants() {
        for v in ["log-only", "dismiss", "persist"] {
            let cli =
                parse(&["send", "--title", "T", "--body", "B", "--on-timeout", v]).unwrap();
            assert!(matches!(cli.command, Command::Send { on_timeout: Some(_), .. }));
        }
    }

    #[test]
    fn send_image_missing_file_rejected() {
        // parse_image runs at CLI-parse time — the failure surfaces before
        // any dispatch code executes.
        assert!(
            parse(&[
                "send", "--title", "T", "--body", "B", "--image",
                "/tmp/does-not-exist-nope.png",
            ])
            .is_err()
        );
    }

    #[test]
    fn send_image_bad_ext_rejected() {
        // Manufacture a file with a non-image extension so the "file exists"
        // check passes but the extension check fails.
        let tmp = std::env::temp_dir().join("notif-cli-test-image-ext.txt");
        std::fs::write(&tmp, b"x").unwrap();
        let err = parse(&[
            "send",
            "--title",
            "T",
            "--body",
            "B",
            "--image",
            tmp.to_str().unwrap(),
        ])
        .err()
        .expect("expected parse error for .txt extension");
        let _ = std::fs::remove_file(&tmp);
        assert!(err.to_string().contains(".png"), "unexpected error: {err}");
    }

    #[test]
    fn send_image_png_accepted() {
        let tmp = std::env::temp_dir().join("notif-cli-test-image.png");
        std::fs::write(&tmp, b"stub").unwrap();
        let cli = parse(&[
            "send",
            "--title",
            "T",
            "--body",
            "B",
            "--image",
            tmp.to_str().unwrap(),
        ])
        .unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert!(matches!(cli.command, Command::Send { image: Some(_), .. }));
    }

    #[test]
    fn quiet_global_before_subcommand() {
        let cli = parse(&["--quiet", "send", "--title", "T", "--body", "B"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn quiet_global_after_subcommand() {
        // `global = true` allows the flag on either side of the subcommand.
        let cli = parse(&["send", "--title", "T", "--body", "B", "--quiet"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn quiet_default_false() {
        let cli = parse(&["send", "--title", "T", "--body", "B"]).unwrap();
        assert!(!cli.quiet);
    }

    #[test]
    fn dry_run_accepted() {
        let cli = parse(&["send", "--title", "T", "--body", "B", "--dry-run"]).unwrap();
        assert!(matches!(cli.command, Command::Send { dry_run: true, .. }));
    }

    #[test]
    fn dry_run_default_false() {
        let cli = parse(&["send", "--title", "T", "--body", "B"]).unwrap();
        assert!(matches!(cli.command, Command::Send { dry_run: false, .. }));
    }

    #[test]
    fn dry_run_format_minimal() {
        // Only title + body set; everything else defaults / stays None.
        let notif = notif_core::Notification {
            title: "Deploy done".to_string(),
            body: "staging \u{2192} prod".to_string(),
            subtitle: None,
            priority: notif_core::Priority::Normal,
            sender: notif_core::Sender::default(),
            id: None,
            sound: None,
            image: None,
            on_timeout: None,
        };
        let expected = "\
title: Deploy done
body: staging \u{2192} prod
subtitle: <none>
priority: normal
sender: default
sound: <none>
image: <none>
id: <none>
on_timeout: <none>
app: <none>
app_resolved_name: <none>
";
        assert_eq!(format_dry_run(&notif, None, None), expected);
    }

    #[test]
    fn dry_run_format_full() {
        let notif = notif_core::Notification {
            title: "Deploy done".to_string(),
            body: "staging -> prod".to_string(),
            subtitle: Some("CI".to_string()),
            priority: notif_core::Priority::Critical,
            sender: notif_core::Sender::new("vscode").unwrap(),
            id: Some("abc-123".to_string()),
            sound: Some(notif_core::Sound::Alert),
            image: Some(std::path::PathBuf::from("/tmp/x.png")),
            on_timeout: Some(notif_core::TimeoutBehavior::Dismiss),
        };
        let expected = "\
title: Deploy done
body: staging -> prod
subtitle: CI
priority: critical
sender: vscode
sound: alert
image: /tmp/x.png
id: abc-123
on_timeout: dismiss
app: Visual Studio Code
app_resolved_name: Visual Studio Code
";
        assert_eq!(
            format_dry_run(&notif, Some("Visual Studio Code"), Some("Visual Studio Code")),
            expected,
        );
    }
}


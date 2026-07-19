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
    /// Increase verbosity on the Windows backend (raise the tracing level
    /// from `info` to `debug`). Ignored on macOS/Linux where the existing
    /// `warn`/`info`/`stderr` helpers already cover the diagnostic surface.
    /// Precedence: `NOTIF_LOG_LEVEL` env > `--quiet` > `--verbose` > default.
    #[arg(long, global = true)]
    verbose: bool,
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
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

// `--on-timeout` accepts a callback target string, parsed through
// `notif_core::callback::parse_target`. `notif_core::TimeoutBehavior`
// stays in the portable crate for Windows/Linux backends that may still
// honor an enum-shaped auto-dismiss behavior spec.

#[derive(Subcommand)]
// The `Send` variant is 5× bigger than the others thanks to the 20+ optional
// flags. Boxing would just add pointer indirection on every access without
// any perf benefit — the CLI parses the enum once per invocation and drops
// it. Suppress the pedantic lint.
#[allow(clippy::large_enum_variant)]
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
        // ---- Tier 3 raw macOS overrides ----------------------------------
        /// `UNNotificationSound::soundNamed(<name>)`. Overrides `--sound`.
        /// Native beats portable with an `info:` log line when both are set.
        #[arg(long)]
        macos_sound_name: Option<String>,
        /// Adds a `UNNotificationAttachment` at this path. Overrides
        /// `--image` with an `info:` log line when both are set.
        #[arg(long)]
        macos_attachment: Option<std::path::PathBuf>,
        /// Raw `UNNotificationInterruptionLevel` value: `passive`, `active`,
        /// `timeSensitive`, `critical`. Overrides `--priority`.
        #[arg(long, value_parser = parse_macos_interruption_level)]
        macos_interruption_level: Option<notif_macos::overrides::InterruptionLevel>,
        /// Grouping key on the banner (`content.setThreadIdentifier`).
        /// No portable counterpart.
        #[arg(long)]
        macos_thread_identifier: Option<String>,
        /// Registered `UNNotificationCategory` identifier
        /// (`content.setCategoryIdentifier`). No portable counterpart.
        #[arg(long)]
        macos_category_identifier: Option<String>,
        // ---- Callback targets --------------------------------------------
        /// Fire on notification body-click. Accepts `hook:<argv>`,
        /// `url:<https?://…>`, `file:<abs-path>`, or a bare target with
        /// auto-detect (URL scheme → url, absolute path → file, else hook).
        #[arg(long, value_parser = parse_callback_target)]
        on_click: Option<notif_core::callback::CallbackTarget>,
        /// Fire on a custom action click. Format `<label>:<target>`.
        /// Repeatable — multiple `--on-action` flags accumulate in the
        /// order the user typed them (macOS renders actions in
        /// registration order).
        #[arg(long, value_parser = parse_callback_action)]
        on_action: Vec<(String, notif_core::callback::CallbackTarget)>,
        /// Fire when the user dismisses the banner. Same target shape as
        /// `--on-click`.
        #[arg(long, value_parser = parse_callback_target)]
        on_dismiss: Option<notif_core::callback::CallbackTarget>,
        /// Fire when the OS auto-dismisses the banner. Callback target
        /// with the same shape as `--on-click`. (The daemon owns the
        /// timer that decides when a banner counts as "timed out".)
        #[arg(long, value_parser = parse_callback_target)]
        on_timeout: Option<notif_core::callback::CallbackTarget>,
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
    /// Run (or shut down) the callback daemon that owns the UN center
    /// delegate for a sender. Auto-spawned on demand by `notif send` when
    /// any `--on-*` flag is present ; can also be launched manually to
    /// verify the socket protocol or tune the idle timeout.
    Listen {
        /// Sender key whose bundle owns this daemon. Absent → `default`.
        #[arg(long)]
        sender: Option<String>,
        /// Duration after which the daemon self-exits when the callback
        /// registry is empty. Accepts `<n>s`, `<n>m`, `<n>h`, `<n>d`, or
        /// a bare integer (seconds).
        #[arg(long, default_value = "24h")]
        idle_timeout: String,
        /// Ask a running daemon to shut down cleanly, then exit.
        /// Mutually exclusive with the run mode — no accept loop is
        /// started.
        #[arg(long)]
        exit: bool,
    },
    /// Remove a delivered notification from Notification Center by id.
    ///
    /// Fire-and-forget — the underlying
    /// `UNUserNotificationCenter.removeDeliveredNotifications:` API
    /// has no synchronous error channel, so an unknown id silently
    /// no-ops. Runs through the sender's bundle so the removal call
    /// carries the right identity.
    Remove {
        /// Sender key whose bundle holds the notification.
        #[arg(long)]
        sender: String,
        /// Notification identifier previously passed to `notif send --id`.
        #[arg(long)]
        id: String,
    },
    /// Windows-only zero-installer flow. Copy the running binary into
    /// `%LOCALAPPDATA%\notif\`, add that dir to the user `Path` (broadcasts
    /// `WM_SETTINGCHANGE` so new shells pick it up without logoff), register
    /// the reserved `default` sender, then fire a warmup toast to prove the
    /// pipeline works under the freshly minted AUMID. Idempotent — safe to
    /// re-run.
    Install,
    /// Windows-only. Reverse `install` : remove the PATH entry, purge every
    /// registered sender (`.lnk` + CLSID + manifest), delete the icon cache.
    /// The binary at `%LOCALAPPDATA%\notif\notif.exe` is left in place — an
    /// in-use file can't be deleted from itself. The log line points at the
    /// path for manual cleanup.
    Uninstall,
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
    // NOTIF_LOG=<absolute-path> duplicates every stderr line into an
    // append-only audit file, timestamped ISO-8601 UTC. Empty / unset =
    // disabled. Relative paths are refused (silently — we can't be sure
    // what the caller's cwd is when e.g. the notify-queue daemon spawns
    // us).
    let log_file = std::env::var("NOTIF_LOG")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute());

    notif_core::warn::init(notif_core::warn::WarnConfig {
        quiet: cli.quiet || env_quiet,
        log_file,
    });

    #[cfg(target_os = "macos")]
    return run_macos(cli.command);

    #[cfg(target_os = "windows")]
    return run_windows(cli.command, cli.verbose, cli.quiet || env_quiet);

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    return run_stub(cli.command);
}

// ---- macOS -----------------------------------------------------------------

#[cfg(target_os = "macos")]
fn run_macos(cmd: Command) -> Result<()> {
    use anyhow::{bail, Context};
    use notif_core::{Notification, Priority, Sender};
    use notif_macos::bundle;
    use notif_macos::dispatch::{
        dispatch_inner, is_inner_mode, setup_inner, setup_outer, setup_outer_bootstrap,
    };
    use notif_macos::sender::DEFAULT_KEY;

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
            macos_sound_name,
            macos_attachment,
            macos_interruption_level,
            macos_thread_identifier,
            macos_category_identifier,
            on_click,
            on_action,
            on_dismiss,
            on_timeout,
            dry_run,
        } => {
            // Strict Tier 1 gate — captured up front because `sender` and
            // `app` get consumed further down (sender.or(...) at the
            // effective-key step, app moves into resolve_app_metadata).
            let tier1 = tier1_activates(
                identifier.as_deref(),
                sender.as_deref(),
                app.as_deref(),
            );

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
                // The CLI routes `--on-timeout` through `CallbackConfig`
                // (target string) so the enum-shaped `Notification.on_timeout`
                // stays `None`. The field is kept on the portable struct for
                // Windows/Linux backends that may still consume an
                // enum-shaped auto-dismiss behavior spec.
                on_timeout: None,
            };

            let overrides = notif_macos::overrides::MacosOverrides {
                sound_name: macos_sound_name,
                attachment: macos_attachment,
                interruption_level: macos_interruption_level,
                thread_identifier: macos_thread_identifier,
                category_identifier: macos_category_identifier,
            };
            let callbacks = notif_core::callback::CallbackConfig {
                on_click,
                on_actions: on_action,
                on_dismiss,
                on_timeout,
            };

            if dry_run {
                // Print the resolved notification and exit before any bundle
                // materialization or UN center call. `resolve_app_metadata`
                // has already run (Spotlight-only, read-only) so the app
                // hint round-trips into the output.
                print!("{}", format_dry_run(&notif, app.as_deref(), auto_name.as_deref()));
                return Ok(());
            }

            if tier1 {
                // Strict-gate path: --identifier alone, no --sender, no --app.
                // Skip the outer / inner two-mode flow entirely — no bundle
                // to materialize, no UNUserNotificationCenter to configure.
                // The gate captured the tier1 boolean before `identifier`
                // could be moved, so the unwrap is safe.
                let id_ref = identifier
                    .as_deref()
                    .expect("tier1 gate guarantees identifier.is_some()");

                // Guard 0 — macOS version. NSUserNotification was deprecated
                // in 10.14 and, from macOS 15 (Sequoia) onward, silently
                // drops delivery for bare CLI processes that have no LSDB
                // `.app` registration. Refuse-early so users see the reason
                // instead of a mystery no-op ; point them at Tier 2.
                if let Some(major) = macos_major_version() {
                    if major >= 15 {
                        bail!(
                            "Tier 1 (NSUserNotification) does not deliver on macOS {major} — the deprecated NS API silently drops delivery for non-bundled CLI processes on macOS 15+ (Sequoia+). Add --sender <key> to switch to Tier 2 (materialized bundle, delivers reliably)."
                        );
                    }
                    if major >= 14 {
                        notif_core::warn::emit(
                            "tier1_ns_may_not_deliver",
                            "NSUserNotification was deprecated in macOS 10.14 and may silently drop delivery for non-bundled CLI processes. If no banner appears, add --sender <key> to switch to Tier 2.",
                        );
                    }
                }

                // Guard 1 — SIP-tier: refuse `com.apple.*` (same reason as
                // the Tier 2 bundle-materialization path).
                refuse_apple_identifier(Some(id_ref))?;

                // Guard 2 — verify the identifier resolves to a real
                // installed app via Spotlight ; without this NC would silently
                // render a blank name for a typo'd identifier.
                notif_macos::sender::resolve_app_hint(id_ref)
                    .with_context(|| {
                        format!(
                            "--identifier {id_ref:?} does not resolve to an installed app via Spotlight"
                        )
                    })?;

                // Guard 3 — refuse flags NSUserNotification can't honor.
                refuse_tier1_incompatible_flags(&notif, &overrides, &callbacks)?;

                return notif_macos::nsun::dispatch_via_nsun(&notif, id_ref)
                    .context("Tier 1 NS spoof dispatch");
            }

            if is_inner_mode() {
                match dispatch_inner(&notif, &overrides, &callbacks) {
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
                    notif_core::warn::stderr("first run: initializing bundle + requesting permission…");

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
                    notif_core::warn::stderr("waiting for permission dialog (click 'Allow' within 60s)…");
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
                            notif_core::warn::stderr("permission granted.");
                        }
                    }
                    Err(notif_macos::MacosError::AuthorizationDenied) => {
                        bail!(
                            "notification permission denied for '{display_hint}' — enable it in System Settings > Notifications, then retry"
                        );
                    }
                    Err(e) => return Err(e).context("authorization check"),
                }

                notif_core::warn::stderr(&format!(
                    "sending notification via '{}'…", notif.sender.key,
                ));
                if callbacks.is_empty() {
                    // Fast path — no callbacks, no daemon. Same
                    // direct-spawn inner-mode dispatch as v0.1.
                    notif_macos::dispatch::dispatch_outer(&notif, &overrides, &callbacks)
                        .context("dispatch")?;
                } else {
                    // Callback path — route via the per-sender daemon
                    // (session 7b). Daemon owns both the UN center
                    // delegate AND the actual `addNotificationRequest`
                    // call, so `notif send` returns as soon as the
                    // daemon acks (typically <10 ms after the socket
                    // connects).
                    let send = build_send_payload(&notif);
                    let payload_context = build_payload_context(&notif);
                    notif_macos::daemon::ensure_running(&notif.sender.key)
                        .context("ensure callback daemon running")?;
                    let sock = notif_macos::daemon::paths::socket_path(&notif.sender.key)?;
                    let resp = notif_macos::daemon::client::send_notif(
                        &sock,
                        send,
                        overrides.clone(),
                        callbacks.clone(),
                        payload_context,
                    )
                    .context("send to callback daemon")?;
                    match resp {
                        notif_macos::daemon::proto::Response::Ack { dispatched_id }
                        | notif_macos::daemon::proto::Response::PendingAuth { dispatched_id } => {
                            notif_core::warn::stderr(&format!(
                                "dispatched via daemon (id={dispatched_id})",
                            ));
                        }
                        notif_macos::daemon::proto::Response::Err { msg } => {
                            bail!("daemon rejected send: {msg}");
                        }
                    }
                }
                notif_core::warn::stderr("sent.");
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
            //
            // Use `setup_outer_bootstrap` (not `setup_outer`) so `lsregister -f`
            // seeds the bundle into LSDB BEFORE `requestAuthorization` fires.
            // On macOS 14+ UN center refuses auth (`UNErrorCode 1 =
            // "Notifications are not allowed for this application"`) for
            // bundles it doesn't know about — see session 3.5 LOG. Register
            // always hits a brand-new bundle by definition.
            setup_outer_bootstrap(&s.key).context("register: request authorization")?;
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
        Command::Listen { sender, idle_timeout, exit } => {
            let sender_key = sender.unwrap_or_else(|| DEFAULT_KEY.to_string());
            if exit {
                notif_macos::daemon::shutdown(&sender_key)
                    .with_context(|| format!("shutdown daemon for sender {sender_key:?}"))?;
                notif_core::warn::stderr(&format!("shutdown signal sent to sender {sender_key:?}"));
                return Ok(());
            }
            let idle = notif_core::duration::parse_duration(&idle_timeout)
                .map_err(|e| anyhow::anyhow!("--idle-timeout: {e}"))?;
            if is_inner_mode() {
                notif_macos::daemon::run_daemon(&sender_key, idle)
                    .context("run callback daemon")
            } else {
                notif_macos::daemon::listen_outer(&sender_key, &idle_timeout)
                    .context("launch inner-mode daemon")
            }
        }
        Command::Remove { sender, id } => {
            let s = Sender::new(sender.clone()).context("invalid sender key")?;
            if is_inner_mode() {
                notif_macos::dispatch::remove_inner(&id).context("inner remove failed")
            } else {
                notif_macos::dispatch::remove_outer(&s.key, &id)
                    .with_context(|| format!("remove notification id={id:?} sender={:?}", s.key))?;
                notif_core::warn::stderr("removed.");
                Ok(())
            }
        }
        Command::Install | Command::Uninstall => {
            bail!("`install` / `uninstall` are Windows-only subcommands")
        }
    }
}

/// Serialize a resolved [`notif_core::Notification`] into the daemon
/// socket's [`SendPayload`] shape. Keeps the wire layer at arm's length
/// from the CLI enum so changes to either side don't cascade.
///
/// [`SendPayload`]: notif_macos::daemon::proto::SendPayload
#[cfg(target_os = "macos")]
fn build_send_payload(notif: &notif_core::Notification) -> notif_macos::daemon::proto::SendPayload {
    notif_macos::daemon::proto::SendPayload {
        title: notif.title.clone(),
        body: notif.body.clone(),
        subtitle: notif.subtitle.clone(),
        priority: notif.priority.wire_str().to_string(),
        sender_key: notif.sender.key.clone(),
        id: notif.id.clone(),
        sound: notif.sound.as_ref().map(|s| s.wire_str().to_string()),
        image: notif.image.as_ref().map(|p| p.to_string_lossy().to_string()),
    }
}

/// Seed a [`CallbackPayload`] with everything except `notif_id` (the
/// daemon fills that in with the resolved id after minting) and `event`
/// (the delegate fills per response kind at fire time).
#[cfg(target_os = "macos")]
fn build_payload_context(
    notif: &notif_core::Notification,
) -> notif_core::callback::CallbackPayload {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    let ts = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default();
    notif_core::callback::CallbackPayload {
        notif_id: notif.id.clone().unwrap_or_default(),
        event: String::new(),
        sender: notif.sender.key.clone(),
        title: notif.title.clone(),
        body: notif.body.clone(),
        ts,
    }
}

#[cfg(target_os = "macos")]
fn run_clean(sender: Option<&str>, all: bool, yes: bool) -> Result<()> {
    use anyhow::{bail, Context};
    let touched_tcc = match (sender, all) {
        (Some(key), false) => {
            let report = notif_macos::clean::clean_sender(key)
                .with_context(|| format!("clean sender {key:?}"))?;
            print_clean(&report);
            true
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
            !reports.is_empty()
        }
        _ => bail!("clean requires either --sender <KEY> or --all"),
    };

    if touched_tcc {
        eprintln!();
        eprintln!("Note — `tccutil` on macOS Sonoma+ often reports success without actually");
        eprintln!("resetting the TCC grant on unnotarized bundles. If the permission dialog");
        eprintln!("does NOT reappear on the next `notif send`, reset manually via :");
        eprintln!();
        eprintln!("    open 'x-apple.systempreferences:com.apple.Notifications-Settings.extension'");
        eprintln!();
        eprintln!("Then toggle Off/On (or Remove via the '-' button) for the sender's entry.");
    }
    Ok(())
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
/// Best-effort read of `kern.osproductversion` returning the major version
/// (e.g. `Some(26)` on Tahoe, `Some(15)` on Sequoia, `Some(14)` on Sonoma).
/// `None` if the sysctl call fails or the value doesn't parse — callers
/// must treat that as "unknown, proceed as if the API might work".
///
/// Used to gate Tier 1 : from macOS 15 (Sequoia) onward the deprecated
/// NSUserNotification API silently drops delivery for bare CLI processes
/// (no LSDB `.app` registration), so we refuse-early there and point users
/// at Tier 2. On 10.14–14 we warn but still attempt (the API historically
/// worked best-effort on those releases).
#[cfg(target_os = "macos")]
fn macos_major_version() -> Option<u32> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "kern.osproductversion"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().split('.').next()?.parse::<u32>().ok()
}

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

/// Strict Tier 1 gate — activates only when `--identifier` is set alone
/// (no `--sender`, no `--app`). Any local-bundle intent falls through to
/// the existing Tier 0 / Tier 2 flow: `--sender` picks a materialized
/// bundle explicitly ; `--app` materializes a bundle whose display name +
/// icon are borrowed from the resolved app.
///
/// Compiled on macOS (called from `run_macos`) and under `cfg(test)` (called
/// from the CLI-shape unit tests on Linux). Never referenced from the Linux
/// bin build, so it stays out of that compilation to avoid a dead_code
/// warning.
#[cfg(any(target_os = "macos", test))]
fn tier1_activates(
    identifier: Option<&str>,
    sender: Option<&str>,
    app: Option<&str>,
) -> bool {
    identifier.is_some() && sender.is_none() && app.is_none()
}

/// Refuse Tier 1 activation when the caller passed flags that the
/// deprecated `NSUserNotification` API can't honor.
///
/// The API predates attachments, interruption levels, and delegate
/// callbacks. Rather than silently drop those semantics we surface a
/// single error listing every offending flag, with a suggested
/// remediation (drop the flags OR add `--sender <key>` to switch to
/// Tier 2). `--priority` is counted only when it differs from `Normal`
/// (the parser default), so a bare `--identifier X` invocation doesn't
/// trip the guard.
///
/// Same cfg strategy as [`tier1_activates`] — compiled on macOS + under
/// `cfg(test)` so the CLI-shape tests can reach it from Linux, but not
/// compiled into the plain Linux bin build (would emit dead_code otherwise).
#[cfg(any(target_os = "macos", test))]
fn refuse_tier1_incompatible_flags(
    notif: &notif_core::Notification,
    overrides: &notif_macos::overrides::MacosOverrides,
    callbacks: &notif_core::callback::CallbackConfig,
) -> Result<()> {
    use anyhow::bail;
    use notif_core::Priority;

    let mut offenders: Vec<&'static str> = Vec::new();
    if notif.priority != Priority::Normal {
        offenders.push("--priority");
    }
    if notif.image.is_some() {
        offenders.push("--image");
    }
    if overrides.sound_name.is_some() {
        offenders.push("--macos-sound-name");
    }
    if overrides.attachment.is_some() {
        offenders.push("--macos-attachment");
    }
    if overrides.interruption_level.is_some() {
        offenders.push("--macos-interruption-level");
    }
    if overrides.thread_identifier.is_some() {
        offenders.push("--macos-thread-identifier");
    }
    if overrides.category_identifier.is_some() {
        offenders.push("--macos-category-identifier");
    }
    if callbacks.on_click.is_some() {
        offenders.push("--on-click");
    }
    if !callbacks.on_actions.is_empty() {
        offenders.push("--on-action");
    }
    if callbacks.on_dismiss.is_some() {
        offenders.push("--on-dismiss");
    }
    if callbacks.on_timeout.is_some() {
        offenders.push("--on-timeout");
    }

    if offenders.is_empty() {
        return Ok(());
    }

    bail!(
        "--identifier activates Tier 1 (NSUserNotification) which does not support: {}. Either drop those flags, or add --sender <key> to switch to Tier 2 (materialized bundle, supports all of the above).",
        offenders.join(", "),
    );
}

/// Value parser for `--macos-interruption-level`. Case-sensitive per
/// [`notif_macos::overrides::InterruptionLevel::parse`] to prevent drift
/// against Apple's documented spelling.
fn parse_macos_interruption_level(
    s: &str,
) -> Result<notif_macos::overrides::InterruptionLevel, String> {
    notif_macos::overrides::InterruptionLevel::parse(s)
        .ok_or_else(|| {
            format!(
                "expected one of passive / active / timeSensitive / critical (got {s:?})",
            )
        })
}

/// Value parser for the single-target `--on-*` flags. Delegates to
/// [`notif_core::callback::parse_target`] and stringifies the error.
///
/// Also rejects the historical `--on-timeout` enum keywords
/// (`log-only`, `dismiss`, `persist`) with a migration message. Without
/// this guard they would silently auto-detect as [`Hook`][notif_core::callback::CallbackKind::Hook]
/// targets (bare argv → exec `log-only` / `dismiss` / `persist` as a
/// program), producing a "command not found" at fire time — much worse
/// than a clean parse error at CLI time.
fn parse_callback_target(
    s: &str,
) -> Result<notif_core::callback::CallbackTarget, String> {
    if matches!(s.trim(), "log-only" | "dismiss" | "persist") {
        return Err(format!(
            "{s:?} was an earlier `--on-timeout` keyword; v0.2 expects a callback target string \
             (`hook:<argv>` / `url:<https?://…>` / `file:<abs-path>` / auto-detect)",
        ));
    }
    notif_core::callback::parse_target(s).map_err(|e| e.to_string())
}

/// Value parser for `--on-action <label>:<target>`. Splits on the FIRST
/// colon to preserve `hook:` / `url:` / `file:` prefixes in the target.
///
/// Label must be non-empty and contain no colon. Target parses through
/// [`notif_core::callback::parse_target`].
fn parse_callback_action(
    s: &str,
) -> Result<(String, notif_core::callback::CallbackTarget), String> {
    let (label, target) = s
        .split_once(':')
        .ok_or_else(|| "expected <label>:<target> (missing colon)".to_string())?;
    if label.is_empty() {
        return Err("--on-action label must be non-empty".into());
    }
    let target = notif_core::callback::parse_target(target).map_err(|e| e.to_string())?;
    Ok((label.to_string(), target))
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

// ---- Windows arm (session 1 : send + remove) -------------------------------

#[cfg(target_os = "windows")]
fn run_windows(cmd: Command, verbose: bool, quiet: bool) -> Result<()> {
    notif_windows::init_logging(verbose, quiet);

    match cmd {
        Command::Send {
            title,
            body,
            subtitle,
            sender,
            priority,
            id,
            ..
        } => {
            let sender_key = sender.unwrap_or_else(|| "default".to_string());
            let sender_obj = notif_core::Sender::new(sender_key)
                .map_err(|e| anyhow::anyhow!("invalid --sender: {e}"))?;
            let resolved = notif_windows::aumid::resolve_for_sender(sender_obj.key.as_str());
            let notif = notif_core::Notification {
                title,
                body,
                subtitle,
                priority: priority.map_or(notif_core::Priority::Normal, PriorityArg::to_core),
                sender: sender_obj,
                id,
                sound: None,
                image: None,
                on_timeout: None,
            };
            notif_windows::dispatch_send(&notif, resolved.aumid())
                .map_err(|e| anyhow::anyhow!("send failed: {e}"))?;
            Ok(())
        }
        Command::Remove { sender, id } => {
            let resolved = notif_windows::aumid::resolve_for_sender(&sender);
            notif_windows::dispatch_remove(&sender, &id, resolved.aumid())
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("remove failed: {e}"))
        }
        Command::Register { sender, name, icon, identifier: _ } => {
            let sender_obj = notif_core::Sender::new(sender)
                .map_err(|e| anyhow::anyhow!("invalid --sender: {e}"))?;
            let reg = notif_windows::register_sender(
                sender_obj.key.as_str(),
                &name,
                icon.as_deref(),
            )
            .map_err(|e| anyhow::anyhow!("register failed: {e}"))?;
            println!(
                "registered: sender={} aumid={} clsid={} lnk={}",
                reg.sender_key,
                reg.aumid,
                notif_windows::aumid::clsid_string(&reg.clsid),
                reg.lnk_path.display(),
            );
            Ok(())
        }
        Command::Install => notif_windows::install_self()
            .map_err(|e| anyhow::anyhow!("install failed: {e}")),
        Command::Uninstall => notif_windows::uninstall_self()
            .map_err(|e| anyhow::anyhow!("uninstall failed: {e}")),
        other => run_stub(other),
    }
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
            macos_sound_name,
            macos_attachment,
            macos_interruption_level,
            macos_thread_identifier,
            macos_category_identifier,
            on_click,
            on_action,
            on_dismiss,
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
            let msn = macos_sound_name.unwrap_or_default();
            let mat = macos_attachment.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            let mil = macos_interruption_level.map(|v| v.wire_str()).unwrap_or_default();
            let mti = macos_thread_identifier.unwrap_or_default();
            let mci = macos_category_identifier.unwrap_or_default();
            let oc = on_click.as_ref().map(|t| t.to_wire()).unwrap_or_default();
            let oa = on_action
                .iter()
                .map(|(l, t)| format!("{l}:{}", t.to_wire()))
                .collect::<Vec<_>>()
                .join(";");
            let od = on_dismiss.as_ref().map(|t| t.to_wire()).unwrap_or_default();
            let ot = on_timeout.as_ref().map(|t| t.to_wire()).unwrap_or_default();
            let dr = if dry_run { "true" } else { "false" };
            println!(
                "[stub] would dispatch: title={title}, body={body}, subtitle={sub}, sender={sender}, name={n}, icon={i}, identifier={bid}, app={a}, priority={prio}, sound={snd}, image={img}, id={nid}, macos_sound_name={msn}, macos_attachment={mat}, macos_interruption_level={mil}, macos_thread_identifier={mti}, macos_category_identifier={mci}, on_click={oc}, on_action={oa}, on_dismiss={od}, on_timeout={ot}, dry_run={dr}, host={HOST}",
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
        Command::Listen { sender, idle_timeout, exit } => {
            let sender = sender.unwrap_or_else(|| "default".to_string());
            println!(
                "[stub] would listen: sender={sender}, idle_timeout={idle_timeout}, exit={exit}, host={HOST}"
            );
        }
        Command::Remove { sender, id } => {
            println!("[stub] would remove: sender={sender}, id={id}, host={HOST}");
        }
        Command::Install | Command::Uninstall => {
            anyhow::bail!("`install` / `uninstall` are Windows-only subcommands");
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
    fn send_on_timeout_legacy_keywords_rejected() {
        // `log-only` / `dismiss` / `persist` used to be the enum values —
        // without the guard, auto-detect would silently classify them as
        // Hook targets (exec `dismiss` as a program) and fail at fire
        // time instead of parse time.
        for v in ["log-only", "dismiss", "persist"] {
            let err = parse(&["send", "--title", "T", "--body", "B", "--on-timeout", v])
                .err()
                .unwrap_or_else(|| panic!("--on-timeout {v} should be rejected"));
            let msg = err.to_string();
            assert!(
                msg.contains("earlier `--on-timeout` keyword"),
                "expected migration message for {v}, got: {msg}",
            );
        }
    }

    #[test]
    fn send_on_timeout_target_accepted() {
        // `--on-timeout` accepts a callback target string, same shape as
        // the other `--on-*` flags.
        for v in [
            "hook:/bin/echo timeout",
            "url:https://example.com/timeout",
            "file:/tmp/timeouts.jsonl",
            "/tmp/timeouts.jsonl",
        ] {
            let cli = parse(&["send", "--title", "T", "--body", "B", "--on-timeout", v]).unwrap();
            assert!(matches!(cli.command, Command::Send { on_timeout: Some(_), .. }), "for {v:?}");
        }
    }

    // ---- Tier 3 --macos-* flags -----------------------------------------

    #[test]
    fn send_macos_sound_name_accepted() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B", "--macos-sound-name", "Ping",
        ]).unwrap();
        match cli.command {
            Command::Send { macos_sound_name, .. } => assert_eq!(macos_sound_name.as_deref(), Some("Ping")),
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn send_macos_attachment_accepted() {
        // No filesystem existence check on this flag — Tier 3 is a raw
        // override, the CLI trusts the caller.
        let cli = parse(&[
            "send", "--title", "T", "--body", "B", "--macos-attachment", "/tmp/x.png",
        ]).unwrap();
        match cli.command {
            Command::Send { macos_attachment: Some(p), .. } => {
                assert_eq!(p.to_str(), Some("/tmp/x.png"));
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn send_macos_interruption_level_accepts_documented_values() {
        for v in ["passive", "active", "timeSensitive", "critical"] {
            let cli = parse(&[
                "send", "--title", "T", "--body", "B", "--macos-interruption-level", v,
            ])
            .unwrap_or_else(|e| panic!("--macos-interruption-level {v} rejected: {e}"));
            assert!(matches!(
                cli.command,
                Command::Send { macos_interruption_level: Some(_), .. },
            ));
        }
    }

    #[test]
    fn send_macos_interruption_level_case_sensitive() {
        // Documented spelling is camelCase — no snake_case tolerance to
        // prevent silent drift against Apple's docs.
        for v in ["Critical", "time_sensitive", "urgent", "TIME_SENSITIVE"] {
            assert!(
                parse(&[
                    "send", "--title", "T", "--body", "B", "--macos-interruption-level", v,
                ])
                .is_err(),
                "expected reject for {v}",
            );
        }
    }

    #[test]
    fn send_macos_thread_and_category_identifiers_accepted() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--macos-thread-identifier", "deploys",
            "--macos-category-identifier", "REPLY",
        ]).unwrap();
        match cli.command {
            Command::Send {
                macos_thread_identifier: Some(tid),
                macos_category_identifier: Some(cid),
                ..
            } => {
                assert_eq!(tid, "deploys");
                assert_eq!(cid, "REPLY");
            }
            _ => panic!("expected both identifiers set"),
        }
    }

    // ---- Callback target flags ------------------------------------------

    #[test]
    fn send_on_click_all_prefix_forms_accepted() {
        for v in [
            "hook:/bin/echo hi",
            "url:https://example.com/click",
            "file:/tmp/clicks.jsonl",
            "/tmp/clicks.jsonl",      // auto-detect → File
            "https://example.com/x",  // auto-detect → Url
            "myscript arg",           // auto-detect → Hook
        ] {
            let cli = parse(&["send", "--title", "T", "--body", "B", "--on-click", v]).unwrap();
            assert!(matches!(cli.command, Command::Send { on_click: Some(_), .. }), "for {v:?}");
        }
    }

    #[test]
    fn send_on_click_bad_prefix_rejected() {
        assert!(parse(&[
            "send", "--title", "T", "--body", "B", "--on-click", "url:ftp://x",
        ]).is_err());
        assert!(parse(&[
            "send", "--title", "T", "--body", "B", "--on-click", "file:relative/path",
        ]).is_err());
    }

    #[test]
    fn send_on_action_multi_flag_accumulates_in_order() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--on-action", "reply:hook:/bin/reply",
            "--on-action", "ignore:file:/tmp/ignored",
        ]).unwrap();
        match cli.command {
            Command::Send { on_action, .. } => {
                assert_eq!(on_action.len(), 2);
                assert_eq!(on_action[0].0, "reply");
                assert_eq!(on_action[0].1.kind, notif_core::callback::CallbackKind::Hook);
                assert_eq!(on_action[1].0, "ignore");
                assert_eq!(on_action[1].1.kind, notif_core::callback::CallbackKind::File);
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn send_on_action_bad_shapes_rejected() {
        // Missing colon.
        assert!(parse(&[
            "send", "--title", "T", "--body", "B", "--on-action", "no-colon-here",
        ]).is_err());
        // Empty label.
        assert!(parse(&[
            "send", "--title", "T", "--body", "B", "--on-action", ":hook:/x",
        ]).is_err());
    }

    #[test]
    fn send_on_dismiss_accepted() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B", "--on-dismiss", "hook:/bin/echo",
        ]).unwrap();
        assert!(matches!(cli.command, Command::Send { on_dismiss: Some(_), .. }));
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

    // ---- notif listen ----------------------------------------------------

    #[test]
    fn listen_no_args_parses() {
        let cli = Cli::try_parse_from(["notif", "listen"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Listen { sender: None, exit: false, .. },
        ));
    }

    #[test]
    fn listen_with_sender() {
        let cli = Cli::try_parse_from(["notif", "listen", "--sender", "vscode"]).unwrap();
        match cli.command {
            Command::Listen { sender: Some(s), .. } => assert_eq!(s, "vscode"),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn listen_with_idle_timeout() {
        let cli = Cli::try_parse_from([
            "notif",
            "listen",
            "--idle-timeout",
            "2h",
        ])
        .unwrap();
        match cli.command {
            Command::Listen { idle_timeout, .. } => assert_eq!(idle_timeout, "2h"),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn listen_default_idle_timeout_is_24h() {
        let cli = Cli::try_parse_from(["notif", "listen"]).unwrap();
        match cli.command {
            Command::Listen { idle_timeout, .. } => assert_eq!(idle_timeout, "24h"),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn listen_exit_flag() {
        let cli = Cli::try_parse_from([
            "notif",
            "listen",
            "--exit",
            "--sender",
            "default",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Listen { exit: true, .. },
        ));
    }

    // ---- Remove subcommand ---------------------------------------------------
    //
    // v0.2 added `notif remove --sender <key> --id <notif-id>` so the notify
    // daemon can dismiss delivered banners on cancel events (user_replied /
    // tool_cancelled). Both flags are required — no default sender, no id
    // guess. Absence of either must fail parsing so a misconfigured caller
    // gets an error instead of a silent no-op.

    #[test]
    fn remove_valid_form_parses() {
        let cli = Cli::try_parse_from([
            "notif",
            "remove",
            "--sender",
            "default",
            "--id",
            "stop-abcd1234-1783000000",
        ])
        .unwrap();
        match cli.command {
            Command::Remove { sender, id } => {
                assert_eq!(sender, "default");
                assert_eq!(id, "stop-abcd1234-1783000000");
            }
            _ => panic!("expected Command::Remove"),
        }
    }

    #[test]
    fn remove_requires_id() {
        let result = Cli::try_parse_from(["notif", "remove", "--sender", "default"]);
        let Err(err) = result else { panic!("expected parse error, got Ok") };
        assert!(
            err.to_string().contains("--id"),
            "expected --id in error: {err}"
        );
    }

    #[test]
    fn remove_requires_sender() {
        let result = Cli::try_parse_from(["notif", "remove", "--id", "x"]);
        let Err(err) = result else { panic!("expected parse error, got Ok") };
        assert!(
            err.to_string().contains("--sender"),
            "expected --sender in error: {err}"
        );
    }

    // ---- Tier 1 identity spoof gate + refuse-loudly guard --------------------

    /// Build a bare `Notification` with default Priority and no image/sound —
    /// used by the refuse-loudly tests so we can flip one offender at a time
    /// without touching the rest of the struct.
    fn bare_notif() -> notif_core::Notification {
        notif_core::Notification {
            title: "T".into(),
            body: "B".into(),
            subtitle: None,
            priority: notif_core::Priority::Normal,
            sender: notif_core::Sender::default(),
            id: None,
            sound: None,
            image: None,
            on_timeout: None,
        }
    }

    fn bare_overrides() -> notif_macos::overrides::MacosOverrides {
        notif_macos::overrides::MacosOverrides {
            sound_name: None,
            attachment: None,
            interruption_level: None,
            thread_identifier: None,
            category_identifier: None,
        }
    }

    fn bare_callbacks() -> notif_core::callback::CallbackConfig {
        notif_core::callback::CallbackConfig {
            on_click: None,
            on_actions: Vec::new(),
            on_dismiss: None,
            on_timeout: None,
        }
    }

    #[test]
    fn tier1_gate_fires_on_identifier_alone() {
        // Parser round-trip — the flag surface accepts the invocation shape.
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--identifier", "com.microsoft.VSCode",
        ])
        .unwrap();
        let (id, sender, app) = match &cli.command {
            Command::Send { identifier, sender, app, .. } => {
                (identifier.as_deref(), sender.as_deref(), app.as_deref())
            }
            _ => unreachable!("parsed as Send"),
        };
        assert!(tier1_activates(id, sender, app));
    }

    #[test]
    fn tier1_gate_disqualified_by_sender() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--identifier", "com.microsoft.VSCode",
            "--sender", "vscode",
        ])
        .unwrap();
        let (id, sender, app) = match &cli.command {
            Command::Send { identifier, sender, app, .. } => {
                (identifier.as_deref(), sender.as_deref(), app.as_deref())
            }
            _ => unreachable!("parsed as Send"),
        };
        assert!(!tier1_activates(id, sender, app));
    }

    #[test]
    fn tier1_gate_disqualified_by_app() {
        let cli = parse(&[
            "send", "--title", "T", "--body", "B",
            "--identifier", "com.microsoft.VSCode",
            "--app", "Visual Studio Code",
        ])
        .unwrap();
        let (id, sender, app) = match &cli.command {
            Command::Send { identifier, sender, app, .. } => {
                (identifier.as_deref(), sender.as_deref(), app.as_deref())
            }
            _ => unreachable!("parsed as Send"),
        };
        assert!(!tier1_activates(id, sender, app));
    }

    #[test]
    fn tier1_gate_bare_send_does_not_activate() {
        // Sanity check — no --identifier means no Tier 1 regardless of the
        // rest of the flags.
        assert!(!tier1_activates(None, None, None));
    }

    #[test]
    fn tier1_refuse_accepts_bare_send() {
        // No offenders — refuse returns Ok.
        refuse_tier1_incompatible_flags(&bare_notif(), &bare_overrides(), &bare_callbacks())
            .expect("bare send must not trip the refuse-loudly guard");
    }

    #[test]
    fn tier1_refuse_priority_normal_is_ok() {
        // Explicit --priority normal is indistinguishable from the parser
        // default, so it must NOT count as an offender.
        let mut n = bare_notif();
        n.priority = notif_core::Priority::Normal;
        refuse_tier1_incompatible_flags(&n, &bare_overrides(), &bare_callbacks())
            .expect("--priority normal must not be refused");
    }

    #[test]
    fn tier1_refuse_priority_high_bails() {
        let mut n = bare_notif();
        n.priority = notif_core::Priority::High;
        let err = refuse_tier1_incompatible_flags(&n, &bare_overrides(), &bare_callbacks())
            .expect_err("--priority high must be refused");
        let msg = format!("{err:#}");
        assert!(msg.contains("--priority"), "err missing --priority: {msg}");
    }

    #[test]
    fn tier1_refuse_image_bails() {
        let mut n = bare_notif();
        n.image = Some(std::path::PathBuf::from("/tmp/x.png"));
        let err = refuse_tier1_incompatible_flags(&n, &bare_overrides(), &bare_callbacks())
            .expect_err("--image must be refused");
        let msg = format!("{err:#}");
        assert!(msg.contains("--image"), "err missing --image: {msg}");
    }

    #[test]
    fn tier1_refuse_macos_attachment_bails() {
        let mut o = bare_overrides();
        o.attachment = Some(std::path::PathBuf::from("/tmp/x.png"));
        let err = refuse_tier1_incompatible_flags(&bare_notif(), &o, &bare_callbacks())
            .expect_err("--macos-attachment must be refused");
        let msg = format!("{err:#}");
        assert!(msg.contains("--macos-attachment"), "err missing --macos-attachment: {msg}");
    }

    #[test]
    fn tier1_refuse_on_click_bails() {
        let mut c = bare_callbacks();
        c.on_click = Some(notif_core::callback::CallbackTarget {
            kind: notif_core::callback::CallbackKind::Hook,
            payload: "/tmp/x.sh".into(),
        });
        let err = refuse_tier1_incompatible_flags(&bare_notif(), &bare_overrides(), &c)
            .expect_err("--on-click must be refused");
        let msg = format!("{err:#}");
        assert!(msg.contains("--on-click"), "err missing --on-click: {msg}");
    }

    #[test]
    fn tier1_refuse_error_names_tier2_escape_hatch() {
        // The message must point users at `--sender <key>` as the remediation
        // path so the error is actionable, not just a wall of "you can't".
        let mut c = bare_callbacks();
        c.on_click = Some(notif_core::callback::CallbackTarget {
            kind: notif_core::callback::CallbackKind::Hook,
            payload: "/tmp/x.sh".into(),
        });
        let err = refuse_tier1_incompatible_flags(&bare_notif(), &bare_overrides(), &c)
            .expect_err("must bail");
        let msg = format!("{err:#}");
        assert!(msg.contains("--sender"), "err missing --sender remediation hint: {msg}");
    }
}


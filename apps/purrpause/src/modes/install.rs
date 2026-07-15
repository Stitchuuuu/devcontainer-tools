// First-run install flow — 10 ordered steps per design § "Flow d'install".
//
// Entry is `run()` — called from main() when the binary is invoked with no
// argv. It queries the SCM for the `WindowsSystemHealth` service, then
// dispatches on one of three actions :
//
//   FreshInstall        — no prior service. Run the full 10-step install.
//   SamePathRelaunch    — service already registered pointing at THIS exe.
//                         Just open the config UI (session 6 fleshes out).
//   PathUpdate          — service points at a different path (user moved
//                         the binary). Stop → change ImagePath → restart.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const SERVICE_NAME: &str = "WindowsSystemHealth";
pub const SERVICE_DISPLAY_NAME: &str = "Windows Session Health Service";
pub const SERVICE_DESCRIPTION: &str =
    "Monitors user session health metrics for ergonomic notifications.";
pub const DIAGNOSTICS_CACHE_DIR: &str = r"C:\ProgramData\DiagnosticsCache";
pub const STATE_DAT: &str = r"C:\ProgramData\DiagnosticsCache\state.dat";

#[derive(Debug, PartialEq, Eq)]
pub enum InstallAction {
    FreshInstall,
    SamePathRelaunch,
    PathUpdate { old_path: PathBuf },
}

/// Pure classifier — decides which action to take given the current
/// binary path and the ImagePath registered in SCM (or `None` if the
/// service doesn't exist). Case-insensitive comparison, matching
/// Windows filesystem semantics.
pub fn classify(current: &Path, existing_image_path: Option<&Path>) -> InstallAction {
    match existing_image_path {
        None => InstallAction::FreshInstall,
        Some(existing) => {
            if paths_equal_ci(current, existing) {
                InstallAction::SamePathRelaunch
            } else {
                InstallAction::PathUpdate {
                    old_path: existing.to_path_buf(),
                }
            }
        }
    }
}

fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

#[cfg(windows)]
pub fn run() -> Result<()> {
    use std::env;

    let current_exe = env::current_exe().context("env::current_exe()")?;
    tracing::info!(path = %current_exe.display(), "install flow starting");

    let existing = query_service_image_path();
    let action = classify(&current_exe, existing.as_deref());
    tracing::info!(?action, "install action decided");

    match action {
        InstallAction::FreshInstall => fresh_install(&current_exe),
        InstallAction::SamePathRelaunch => launch_config_ui(&current_exe),
        InstallAction::PathUpdate { old_path } => path_update(&current_exe, &old_path),
    }
}

#[cfg(not(windows))]
pub fn run() -> Result<()> {
    anyhow::bail!("install flow is Windows-only");
}

#[cfg(windows)]
fn query_service_image_path() -> Option<PathBuf> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;
    let svc = scm.open_service(SERVICE_NAME, ServiceAccess::QUERY_CONFIG).ok()?;
    let cfg = svc.query_config().ok()?;
    Some(cfg.executable_path)
}

#[cfg(windows)]
fn fresh_install(current_exe: &Path) -> Result<()> {
    use crate::platform::win32::{acl, hidden_attrs, itaskservice};

    // Step 4 : animations/ next to the exe (writable, no restrictive DACL).
    let animations_dir = current_exe
        .parent()
        .context("current_exe has no parent")?
        .join("animations");
    std::fs::create_dir_all(&animations_dir)
        .with_context(|| format!("mkdir {}", animations_dir.display()))?;
    tracing::info!(path = %animations_dir.display(), "step 4: animations/ ready");

    // Step 5 : DiagnosticsCache with DACL + hidden+system.
    let cache_dir = Path::new(DIAGNOSTICS_CACHE_DIR);
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("mkdir {}", cache_dir.display()))?;
    acl::apply_diagnostics_cache_dacl(cache_dir).context("step 5: apply DACL")?;
    hidden_attrs::set_hidden_system(cache_dir).context("step 5: hidden+system attrs")?;
    tracing::info!(path = %cache_dir.display(), "step 5: DiagnosticsCache ready");

    // Step 6 : default state.dat — but ONLY if none exists. A prior
    // failed path_update (SCM in "marked for deletion" state) triggers
    // a re-run of fresh_install ; overwriting the existing state.dat
    // here would wipe the parent's passcode + all their tuning.
    let state_path = Path::new(STATE_DAT);
    let state_existed = state_path.exists();
    if !state_existed {
        let default_cfg = crate::config::Config::default();
        crate::config::save(&default_cfg, state_path)
            .context("step 6: default state.dat")?;
        tracing::info!("step 6: default state.dat written (fresh)");
    } else {
        tracing::info!("step 6: state.dat already exists, preserving passcode + config");
    }

    // Step 7 : Windows service.
    register_service(current_exe).context("step 7: register service")?;
    tracing::info!("step 7: service registered");

    // Step 8 : Scheduled Task watchdog.
    itaskservice::register_watchdog(current_exe).context("step 8: register watchdog")?;
    tracing::info!("step 8: watchdog task registered");

    // Step 9 : block on first-run wizard — SKIP if state.dat already
    // has a passcode. This handles the "PathUpdate failed → FreshInstall
    // fallback" recovery path : the parent already went through the
    // wizard on a previous version, no need to ask again.
    let wizard_needed = match crate::config::load(state_path) {
        Ok(cfg) => cfg.passcode_hash.is_empty(),
        Err(_) => true,
    };
    if wizard_needed {
        let status = std::process::Command::new(current_exe)
            .arg("--config")
            .arg("--first-run")
            .status()
            .context("step 9: spawn wizard")?;
        if !status.success() {
            tracing::warn!(?status, "step 9: wizard cancelled — rolling back");
            crate::modes::rollback::run().ok();
            anyhow::bail!("first-run wizard cancelled");
        }
        tracing::info!("step 9: first-run wizard completed");
    } else {
        tracing::info!("step 9: skipping wizard, passcode already set (recovery path)");
    }

    // Step 10 : start the service.
    start_service().context("step 10: start service")?;
    tracing::info!("step 10: service started");

    // Bonus : open the config UI right after the wizard succeeded, so
    // the parent gets a visible "you're set" moment instead of a
    // silent exit — the user typically wants to customize intervals /
    // add animations right after first install.
    launch_config_ui(current_exe).context("open config UI after fresh install")?;

    Ok(())
}

/// Register the service, retrying a few times if SCM refuses because
/// the name is still "marked for deletion" (all handles must close +
/// short kernel window). Used only in the path-update flow.
#[cfg(windows)]
fn retry_register_service(current_exe: &Path) -> Result<()> {
    const MAX_ATTEMPTS: u32 = 6;
    const BACKOFF_MS: u64 = 500;
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match register_service(current_exe) {
            Ok(()) => {
                if attempt > 1 {
                    tracing::info!(attempt, "re-register service succeeded after retry");
                }
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    attempt,
                    error = %e,
                    "re-register service failed (SCM likely still marked-for-deletion) — retrying"
                );
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(BACKOFF_MS * attempt as u64));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("retry_register_service: exhausted attempts")))
}

#[cfg(windows)]
fn register_service(current_exe: &Path) -> Result<()> {
    use std::ffi::OsString;
    use windows_service::service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let scm = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .context("open SCM")?;

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: current_exe.to_path_buf(),
        launch_arguments: vec![OsString::from("--service")],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    let svc = scm
        .create_service(&info, ServiceAccess::CHANGE_CONFIG)
        .context("create_service")?;
    svc.set_description(SERVICE_DESCRIPTION)
        .context("set_description")?;
    Ok(())
}

#[cfg(windows)]
fn start_service() -> Result<()> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .context("open SCM")?;
    let svc = scm
        .open_service(SERVICE_NAME, ServiceAccess::START)
        .context("open service")?;
    svc.start(&[] as &[&str]).context("start service")?;
    Ok(())
}

#[cfg(windows)]
fn launch_config_ui(current_exe: &Path) -> Result<()> {
    tracing::info!(exe = %current_exe.display(), "launching config UI child (fire-and-forget)");
    // Fire-and-forget : the child owns the window loop, the parent
    // (this install-flow process) exits immediately. Using .status()
    // here would block until the child dies and leave a phantom parent
    // process in Task Manager.
    std::process::Command::new(current_exe)
        .arg("--config")
        .spawn()
        .context("spawn config UI child")?;
    Ok(())
}

#[cfg(windows)]
fn path_update(current_exe: &Path, old_path: &Path) -> Result<()> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    tracing::info!(
        old = %old_path.display(),
        new = %current_exe.display(),
        "user moved the binary — updating install paths",
    );

    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .context("open SCM")?;
    let svc = scm
        .open_service(
            SERVICE_NAME,
            ServiceAccess::CHANGE_CONFIG
                | ServiceAccess::STOP
                | ServiceAccess::START
                | ServiceAccess::QUERY_STATUS
                | ServiceAccess::DELETE,
        )
        .context("open service for update")?;

    // Best-effort stop before rewiring ImagePath. Poll a short window
    // so a running service actually reaches Stopped before we delete
    // it (SCM refuses delete on a running service on some hosts).
    let _ = svc.stop();
    for _ in 0..20 {
        match svc.query_status() {
            Ok(status)
                if status.current_state == windows_service::service::ServiceState::Stopped =>
            {
                break;
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(250)),
        }
    }

    // Update the service action to point at the new exe. The
    // windows-service crate re-exports ChangeServiceConfigW via
    // Service::change_config in recent versions ; if unavailable, we
    // fall back to delete + create (heavier but portable).
    // For MVP we delete + recreate — task update follows the same path.
    svc.delete().context("delete old service entry")?;
    // SCM defers actual removal until every handle is closed AND every
    // process using the name has exited. Drop ours and any transitively
    // held SCM handle, then retry register with backoff — "marked for
    // deletion" typically clears within a few seconds.
    drop(svc);
    retry_register_service(current_exe)
        .context("re-register service at new path (SCM marked-for-deletion contention)")?;

    // Same treatment for the scheduled task.
    crate::platform::win32::itaskservice::update_watchdog_action(current_exe)
        .context("update watchdog action")?;

    // Copy animations/ from the old dir if the new one doesn't have any.
    migrate_animations_folder(old_path, current_exe).ok();

    // Restart the service.
    start_service().context("start service after path update")?;

    // Open the config UI so the path-swap gives visual confirmation to
    // the user instead of a silent parent exit. Same fire-and-forget
    // shape as SamePathRelaunch.
    launch_config_ui(current_exe).context("open config UI after path update")?;
    Ok(())
}

#[cfg(windows)]
fn migrate_animations_folder(old_exe: &Path, new_exe: &Path) -> Result<()> {
    let old_dir = old_exe.parent().context("old_exe parent")?.join("animations");
    let new_dir = new_exe.parent().context("new_exe parent")?.join("animations");
    if !old_dir.exists() {
        return Ok(());
    }
    // Only copy if the new one has no user-added anims (skip if present
    // to avoid clobbering user work).
    if new_dir.exists() {
        let has_files = std::fs::read_dir(&new_dir)
            .map(|d| d.filter_map(|e| e.ok()).any(|_| true))
            .unwrap_or(false);
        if has_files {
            return Ok(());
        }
    }
    std::fs::create_dir_all(&new_dir).ok();
    for entry in std::fs::read_dir(&old_dir)? {
        let entry = entry?;
        let dst = new_dir.join(entry.file_name());
        std::fs::copy(entry.path(), dst).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn no_existing_service_yields_fresh_install() {
        let current = PathBuf::from(r"C:\Users\alice\Desktop\SystemHealthAgent.exe");
        assert_eq!(classify(&current, None), InstallAction::FreshInstall);
    }

    #[test]
    fn same_path_yields_same_path_relaunch() {
        let current = PathBuf::from(r"C:\Users\alice\Desktop\SystemHealthAgent.exe");
        assert_eq!(
            classify(&current, Some(&current)),
            InstallAction::SamePathRelaunch
        );
    }

    #[test]
    fn same_path_case_insensitive() {
        let current = PathBuf::from(r"C:\Users\alice\Desktop\SystemHealthAgent.exe");
        let existing = PathBuf::from(r"c:\users\ALICE\desktop\systemhealthagent.exe");
        assert_eq!(
            classify(&current, Some(&existing)),
            InstallAction::SamePathRelaunch
        );
    }

    #[test]
    fn different_path_yields_path_update() {
        let current = PathBuf::from(r"D:\Tools\SystemHealthAgent.exe");
        let existing = PathBuf::from(r"C:\Users\alice\Desktop\SystemHealthAgent.exe");
        match classify(&current, Some(&existing)) {
            InstallAction::PathUpdate { old_path } => assert_eq!(old_path, existing),
            other => panic!("expected PathUpdate, got {other:?}"),
        }
    }
}

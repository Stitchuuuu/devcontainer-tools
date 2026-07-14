// Minimal watchdog mode. Invoked every minute by the Scheduled Task
// (registered by the install flow). Job for this session : if the
// service isn't running, start it. Anything more elaborate (self-heal,
// crash-log rotation, path-drift detection) belongs to a later session.

use anyhow::Result;

#[cfg(windows)]
pub fn run() -> Result<()> {
    use anyhow::Context;
    use windows_service::service::{ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use crate::modes::install::SERVICE_NAME;

    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .context("open SCM")?;

    let svc = match scm.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::START,
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "watchdog: service not registered");
            return Ok(());
        }
    };

    let status = svc.query_status().context("query_status")?;
    if matches!(
        status.current_state,
        ServiceState::Running | ServiceState::StartPending
    ) {
        tracing::debug!(?status.current_state, "watchdog: service healthy");
        return Ok(());
    }

    tracing::info!(?status.current_state, "watchdog: starting service");
    let empty: [&str; 0] = [];
    if let Err(e) = svc.start(&empty) {
        tracing::warn!(error = %e, "watchdog: failed to start service");
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn run() -> Result<()> {
    anyhow::bail!("watchdog is Windows-only")
}

use tracing::{debug, info};
use windows::{
    core::HSTRING,
    UI::Notifications::ToastNotificationManager,
};

use crate::backend::WindowsError;

/// Remove the toast identified by `(sender, id)` under `aumid` from Action
/// Center. Returns `Ok(true)` when a matching toast was found and removed,
/// `Ok(false)` when nothing matched (no-op, exit code stays 0).
///
/// The pre-check via `GetHistoryWithId` is what lets the CLI log
/// "removed" vs. "not found" — the underlying `RemoveGroupedTagWithId` API
/// itself does not report whether it actually deleted anything.
pub fn dispatch_remove(sender: &str, id: &str, aumid: &str) -> Result<bool, WindowsError> {
    let history = ToastNotificationManager::History()
        .map_err(|e| WindowsError::with_context("ToastNotificationManager::History", e))?;

    let haumid = HSTRING::from(aumid);
    let hid = HSTRING::from(id);
    let hsender = HSTRING::from(sender);

    let existing = history.GetHistoryWithId(&haumid).map_err(|e| {
        WindowsError::with_context("ToastNotificationHistory::GetHistoryWithId", e)
    })?;

    let mut found = false;
    for toast in existing {
        let tag_ok = toast
            .Tag()
            .map(|h| h == hid)
            .unwrap_or(false);
        let group_ok = toast
            .Group()
            .map(|h| h == hsender)
            .unwrap_or(false);
        if tag_ok && group_ok {
            found = true;
            break;
        }
    }
    debug!(target: "notif::remove", sender, id, aumid, found, "history pre-check");

    if !found {
        info!(target: "notif::remove", sender, id, "not found — no-op");
        return Ok(false);
    }

    history
        .RemoveGroupedTagWithId(&hid, &hsender, &haumid)
        .map_err(|e| {
            WindowsError::with_context("ToastNotificationHistory::RemoveGroupedTagWithId", e)
        })?;
    info!(target: "notif::remove", sender, id, "removed");
    Ok(true)
}

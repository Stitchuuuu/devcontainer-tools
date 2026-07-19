//! COM `INotificationActivationCallback` server for toast click routing.
//!
//! When the user clicks a toast body or an action button, Explorer looks up
//! the sender's CLSID in `HKCU\Software\Classes\CLSID\{...}\LocalServer32`,
//! spawns `notif.exe --activator-serve`, and calls `Activate` on the
//! registered class factory. This module owns that class factory + the
//! `Activate` impl that routes the event back to the sidecar the sender
//! wrote at dispatch time.
//!
//! Registered CLSIDs enumerate from `aumid::win::list_senders()` — every
//! sender that ever ran `notif register` gets a class factory, so a single
//! `--activator-serve` process handles callbacks for every sender.
//!
//! Apartment : STA (`COINIT_APARTMENTTHREADED`). Rationale : cohérent avec
//! [`crate::register::write_lnk`] and required for the `GetMessageW` +
//! `DispatchMessageW` loop that keeps class factories alive.

use notif_core::callback::{fire, CallbackEvent, CallbackPayload};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::{debug, error, info, warn};
use windows::core::{implement, IUnknown, Interface, Ref, GUID, PCWSTR};
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_OK};
use windows::Win32::System::Com::{
    CoInitializeEx, CoRegisterClassObject, CoResumeClassObjects, CoRevokeClassObject,
    CoUninitialize, IClassFactory, IClassFactory_Impl, CLSCTX_LOCAL_SERVER,
    COINIT_APARTMENTTHREADED, REGCLS_MULTIPLEUSE, REGCLS_SUSPENDED,
};
use windows::Win32::UI::Notifications::{
    INotificationActivationCallback, INotificationActivationCallback_Impl,
    NOTIFICATION_USER_INPUT_DATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

use crate::aumid;
use crate::backend::WindowsError;
use crate::callbacks::{append_inbox, delete_sidecar, parse_invoked_args, read_sidecar, Sidecar};

/// COM object exposing `INotificationActivationCallback` for one CLSID.
/// The `sender_key` captured here lets `Activate` write to the correct
/// inbox file even though the OS only passes back the AUMID string.
#[implement(INotificationActivationCallback)]
struct NotifActivator {
    sender_key: String,
}

impl INotificationActivationCallback_Impl for NotifActivator_Impl {
    fn Activate(
        &self,
        _app_user_model_id: &PCWSTR,
        invoked_args: &PCWSTR,
        _data: *const NOTIFICATION_USER_INPUT_DATA,
        _count: u32,
    ) -> windows::core::Result<()> {
        let args = unsafe { pcwstr_to_string(invoked_args) };
        info!(
            target: "notif::activator",
            sender = %self.sender_key,
            args = %args,
            "Activate",
        );
        handle_activation(&self.sender_key, &args);
        Ok(())
    }
}

/// Trivial `IClassFactory` — `CreateInstance` returns a fresh
/// `NotifActivator` bound to a captured `sender_key`; `LockServer` no-ops.
#[implement(IClassFactory)]
struct NotifActivatorFactory {
    sender_key: String,
}

impl IClassFactory_Impl for NotifActivatorFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        if !punkouter.is_null() {
            return Err(windows::Win32::Foundation::CLASS_E_NOAGGREGATION.into());
        }
        let activator: INotificationActivationCallback =
            NotifActivator { sender_key: self.sender_key.clone() }.into();
        unsafe { activator.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, _flock: windows::core::BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Run the COM local-server loop : init STA, register a class factory per
/// registered sender, resume class objects, pump messages until `WM_QUIT`,
/// then revoke + uninitialize. Blocks until the message loop exits (Explorer
/// sending `WM_QUIT`, the user killing the process, or a shutdown signal).
pub fn run_activator_serve() -> Result<(), WindowsError> {
    let senders = enumerate_senders();
    if senders.is_empty() {
        warn!(
            target: "notif::activator",
            "no senders registered — nothing to serve; exiting cleanly",
        );
        return Ok(());
    }
    info!(
        target: "notif::activator",
        count = senders.len(),
        "starting COM server",
    );

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // S_FALSE and RPC_E_CHANGED_MODE both mean "COM already initialized on
    // this thread" — accept, do NOT call CoUninitialize at exit in those
    // cases. Fresh init (S_OK) is what we own.
    let owns_com = if hr == S_OK {
        true
    } else if hr == RPC_E_CHANGED_MODE {
        warn!(
            target: "notif::activator",
            "CoInitializeEx returned RPC_E_CHANGED_MODE — reusing prior apartment",
        );
        false
    } else if hr.is_ok() {
        false
    } else {
        return Err(WindowsError::with_context(
            "CoInitializeEx",
            windows::core::Error::from(hr),
        ));
    };

    let mut cookies: Vec<u32> = Vec::with_capacity(senders.len());
    for (sender_key, clsid) in &senders {
        let factory: IClassFactory = NotifActivatorFactory {
            sender_key: sender_key.clone(),
        }
        .into();
        let cookie = unsafe {
            CoRegisterClassObject(
                &guid_from_uuid(clsid),
                &factory,
                CLSCTX_LOCAL_SERVER,
                REGCLS_MULTIPLEUSE | REGCLS_SUSPENDED,
            )
        }
        .map_err(|e| WindowsError::with_context("CoRegisterClassObject", e))?;
        debug!(
            target: "notif::activator",
            sender = %sender_key,
            clsid = %aumid::clsid_string(clsid),
            cookie,
            "class factory registered",
        );
        cookies.push(cookie);
    }

    unsafe { CoResumeClassObjects() }
        .map_err(|e| WindowsError::with_context("CoResumeClassObjects", e))?;
    info!(
        target: "notif::activator",
        registered = cookies.len(),
        "class objects resumed; entering message loop",
    );

    // STA message loop — required for COM callbacks to marshal into this
    // thread. Explorer sends `WM_QUIT` when the server should exit.
    let mut msg = MSG::default();
    loop {
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if got.0 == 0 {
            // WM_QUIT received.
            break;
        }
        if got.0 == -1 {
            error!(target: "notif::activator", "GetMessageW error; exiting");
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    for cookie in cookies {
        if let Err(e) = unsafe { CoRevokeClassObject(cookie) } {
            warn!(target: "notif::activator", cookie, error = %e, "CoRevokeClassObject failed");
        }
    }
    if owns_com {
        unsafe { CoUninitialize() };
    }
    info!(target: "notif::activator", "server exited");
    Ok(())
}

fn enumerate_senders() -> Vec<(String, uuid::Uuid)> {
    match aumid::win::list_senders() {
        Ok(keys) => keys
            .into_iter()
            .filter_map(|key| match aumid::win::read_manifest(&key) {
                Ok(Some(m)) => Some((key, m.clsid)),
                Ok(None) => None,
                Err(e) => {
                    warn!(
                        target: "notif::activator",
                        sender = %key,
                        error = %e,
                        "read_manifest failed; skipping",
                    );
                    None
                }
            })
            .collect(),
        Err(e) => {
            error!(target: "notif::activator", error = %e, "list_senders failed");
            Vec::new()
        }
    }
}

fn handle_activation(sender_key: &str, invoked_args: &str) {
    let Some((event, notif_id)) = parse_invoked_args(invoked_args) else {
        warn!(
            target: "notif::activator",
            args = %invoked_args,
            "unknown invoked_args shape; no-op",
        );
        return;
    };
    let sidecar = match read_sidecar(&notif_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!(
                target: "notif::activator",
                notif_id = %notif_id,
                "sidecar missing; no-op",
            );
            return;
        }
        Err(e) => {
            warn!(target: "notif::activator", error = %e, "read_sidecar failed; no-op");
            return;
        }
    };
    if sidecar.sender != sender_key {
        // Defensive : shouldn't happen (each CLSID's factory only registers
        // for one sender), but log if the sidecar's sender disagrees with
        // the factory's captured key.
        warn!(
            target: "notif::activator",
            factory_sender = %sender_key,
            sidecar_sender = %sidecar.sender,
            "sender mismatch between factory and sidecar",
        );
    }
    let payload = build_payload(&notif_id, &event, &sidecar);
    if let Some(target) = pick_target(&sidecar, &event) {
        let _ = fire(&target, &payload);
    } else {
        debug!(
            target: "notif::activator",
            event = %event.to_wire(),
            "no callback target registered for this event; skipping fire",
        );
    }
    if let Err(e) = append_inbox(&sidecar.sender, &payload) {
        warn!(target: "notif::activator", error = %e, "append_inbox failed");
    }
    delete_sidecar(&notif_id);
}

fn build_payload(notif_id: &str, event: &CallbackEvent, sidecar: &Sidecar) -> CallbackPayload {
    let ts = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));
    CallbackPayload {
        notif_id: notif_id.to_string(),
        event: event.to_wire(),
        sender: sidecar.sender.clone(),
        title: sidecar.title.clone(),
        body: sidecar.body.clone(),
        ts,
    }
}

fn pick_target(
    sidecar: &Sidecar,
    event: &CallbackEvent,
) -> Option<notif_core::callback::CallbackTarget> {
    match event {
        CallbackEvent::Click => sidecar.callbacks.on_click.clone(),
        CallbackEvent::Action(label) => sidecar
            .callbacks
            .on_actions
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, t)| t.clone()),
        CallbackEvent::Dismiss => sidecar.callbacks.on_dismiss.clone(),
        CallbackEvent::Timeout => sidecar.callbacks.on_timeout.clone(),
    }
}

fn guid_from_uuid(u: &uuid::Uuid) -> GUID {
    GUID::from_u128(u.as_u128())
}

/// # Safety
///
/// `p` must point to a valid NUL-terminated UTF-16 string owned by the caller
/// for the duration of the call. The `PCWSTR` values Windows passes into
/// `Activate` satisfy this — they live for the length of the invocation.
unsafe fn pcwstr_to_string(p: &PCWSTR) -> String {
    if p.0.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *p.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(p.0, len);
        String::from_utf16_lossy(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notif_core::callback::{CallbackConfig, CallbackKind, CallbackTarget};

    fn sidecar_with(callbacks: CallbackConfig) -> Sidecar {
        Sidecar {
            sender: "default".into(),
            title: "T".into(),
            body: "B".into(),
            ts: "2026-07-19T00:00:00Z".into(),
            callbacks,
        }
    }

    fn file_target(p: &str) -> CallbackTarget {
        CallbackTarget { kind: CallbackKind::File, payload: p.into() }
    }

    #[test]
    fn pick_target_click() {
        let sc = sidecar_with(CallbackConfig {
            on_click: Some(file_target("/tmp/c")),
            ..CallbackConfig::default()
        });
        let t = pick_target(&sc, &CallbackEvent::Click).unwrap();
        assert_eq!(t.payload, "/tmp/c");
    }

    #[test]
    fn pick_target_action_matches_label() {
        let sc = sidecar_with(CallbackConfig {
            on_actions: vec![
                ("Allow".into(), file_target("/tmp/a")),
                ("Deny".into(), file_target("/tmp/d")),
            ],
            ..CallbackConfig::default()
        });
        let t = pick_target(&sc, &CallbackEvent::Action("Deny".into())).unwrap();
        assert_eq!(t.payload, "/tmp/d");
    }

    #[test]
    fn pick_target_missing_returns_none() {
        let sc = sidecar_with(CallbackConfig::default());
        assert!(pick_target(&sc, &CallbackEvent::Click).is_none());
        assert!(pick_target(&sc, &CallbackEvent::Action("x".into())).is_none());
    }

    #[test]
    fn build_payload_shape() {
        let sc = sidecar_with(CallbackConfig::default());
        let p = build_payload("abc-123", &CallbackEvent::Click, &sc);
        assert_eq!(p.notif_id, "abc-123");
        assert_eq!(p.event, "click");
        assert_eq!(p.sender, "default");
        assert_eq!(p.title, "T");
        assert_eq!(p.body, "B");
        assert!(!p.ts.is_empty());
    }
}

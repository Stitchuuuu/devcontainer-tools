//! Unix-socket server thread. Accepts incoming connections from
//! `notif send` (outer), dispatches `SendNotif` to
//! [`crate::dispatch::inner::dispatch_inner`], and registers the callback
//! binding so the delegate can fire on user response.
//!
//! The server runs in a background thread launched by
//! [`crate::daemon::run_daemon`]. UN center's `addNotificationRequest` is
//! thread-safe ; the delegate itself runs on the main thread (per Apple's
//! delivery model), where it dispatches callbacks via
//! [`notif_core::callback::fire`].

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use notif_core::callback::CallbackPayload;
use notif_core::{Notification, Priority, Sender, Sound};
use objc2_foundation::NSUUID;

use crate::daemon::proto::{self, Request, Response, SendPayload};
use crate::daemon::registry::{Binding, Registry};
use crate::error::MacosError;
use crate::overrides::MacosOverrides;

/// Shared state between the server thread, delegate, and the main
/// runloop. Cloneable — every consumer clones the shared handles.
#[derive(Clone)]
pub struct ServerState {
    pub registry: Registry,
    pub shutdown: Arc<AtomicBool>,
    pub last_activity: Arc<Mutex<Instant>>,
}

impl ServerState {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            shutdown: Arc::new(AtomicBool::new(false)),
            last_activity: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }
}

/// Bind the Unix socket and spawn the accept-loop thread. Returns the
/// listener so the caller can hold it alive ; dropping the listener
/// removes the bound socket path.
///
/// The accept loop terminates when [`ServerState::shutdown`] is set —
/// checked between each `accept()` call. To break out of a blocking
/// `accept()` we set `set_nonblocking(true)` and poll.
///
/// # Errors
/// [`MacosError::Io`] if the socket cannot be bound or configured.
pub fn spawn_accept_loop(socket_path: &std::path::Path, state: ServerState) -> Result<UnixListener, MacosError> {
    // Best-effort remove — a stale socket from a crashed previous
    // instance blocks bind. Ignore NotFound.
    if let Err(e) = std::fs::remove_file(socket_path) {
        if e.kind() != io::ErrorKind::NotFound {
            return Err(MacosError::Io(e));
        }
    }
    let listener = UnixListener::bind(socket_path)?;
    listener.set_nonblocking(true)?;
    // Restrict to owner — see § socket permissions in the daemon module
    // docs.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o700))?;

    let listener_clone = listener.try_clone()?;
    let state_clone = state.clone();
    std::thread::Builder::new()
        .name("notif-daemon-accept".into())
        .spawn(move || accept_loop(listener_clone, state_clone))
        .map_err(|e| MacosError::DaemonStartFailed(format!("accept thread spawn: {e}")))?;
    Ok(listener)
}

fn accept_loop(listener: UnixListener, state: ServerState) {
    let poll_interval = std::time::Duration::from_millis(50);
    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            return;
        }
        match listener.accept() {
            Ok((stream, _addr)) => {
                let state = state.clone();
                std::thread::spawn(move || handle_connection(stream, state));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                notif_core::warn::emit(
                    "daemon_accept_error",
                    &format!("accept() failed: {e}"),
                );
                std::thread::sleep(poll_interval);
            }
        }
    }
}

fn handle_connection(mut stream: UnixStream, state: ServerState) {
    state.touch();
    let req: Request = match proto::read_frame(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            let _ = proto::write_frame(
                &mut stream,
                &Response::Err { msg: format!("read_frame: {e}") },
            );
            return;
        }
    };
    let resp = match req {
        Request::SendNotif { notif, overrides, callbacks, payload_context } => {
            handle_send_notif(&state, notif, overrides, callbacks, payload_context)
        }
        Request::Shutdown => {
            state.shutdown.store(true, Ordering::Relaxed);
            Response::Ack { dispatched_id: String::new() }
        }
    };
    let _ = proto::write_frame(&mut stream, &resp);
}

fn handle_send_notif(
    state: &ServerState,
    send: SendPayload,
    overrides: MacosOverrides,
    callbacks: notif_core::callback::CallbackConfig,
    mut payload_context: CallbackPayload,
) -> Response {
    // Mint id if the caller didn't provide one — daemon owns the
    // resolved id so the ack can carry it back.
    let dispatched_id = send.id.clone().unwrap_or_else(|| NSUUID::UUID().UUIDString().to_string());

    // Log the `--on-timeout is a no-op on macOS` info line once per
    // sender-lifetime. warn::info dedup slot handles the "once" part.
    if callbacks.on_timeout.is_some() {
        notif_core::warn::info(
            "callback_timeout_no_op_macos",
            "--on-timeout is a no-op on macOS (UN center emits no timeout event); the flag is accepted for portable CLI shape",
        );
    }

    // Reify into a portable Notification. Daemon lives inside its
    // sender's bundle so `Sender::new` is guaranteed to hold the same
    // key that spawned the daemon.
    let notif = match build_notification(&send, dispatched_id.clone()) {
        Ok(n) => n,
        Err(e) => return Response::Err { msg: format!("build notification: {e}") },
    };

    // Register the binding BEFORE dispatch so a synchronous click (very
    // unlikely on macOS, but the ordering matters) sees the entry.
    payload_context.notif_id = dispatched_id.clone();
    state.registry.insert(
        dispatched_id.clone(),
        Binding { callbacks: callbacks.clone(), payload: payload_context },
    );

    match crate::dispatch::dispatch_inner(&notif, &overrides, &callbacks) {
        Ok(()) => Response::Ack { dispatched_id },
        Err(e) => {
            // Roll back the registration — no delivered notification =
            // no possible callback.
            state.registry.take_on_response(&dispatched_id, crate::daemon::registry::ResponseKind::Click);
            Response::Err { msg: format!("dispatch_inner: {e}") }
        }
    }
}

fn build_notification(send: &SendPayload, dispatched_id: String) -> Result<Notification, MacosError> {
    let sender = Sender::new(send.sender_key.clone())?;
    let priority = match send.priority.as_str() {
        "low" => Priority::Low,
        "normal" => Priority::Normal,
        "high" => Priority::High,
        "critical" => Priority::Critical,
        other => {
            return Err(MacosError::DaemonProtocol(format!(
                "unknown priority wire form {other:?}",
            )));
        }
    };
    let sound = send.sound.as_deref().map(|raw| match raw {
        "default" => Sound::Default,
        "alert" => Sound::Alert,
        other => Sound::Custom(other.to_string()),
    });
    Ok(Notification {
        title: send.title.clone(),
        body: send.body.clone(),
        subtitle: send.subtitle.clone(),
        priority,
        sender,
        id: Some(dispatched_id),
        sound,
        image: send.image.as_ref().map(std::path::PathBuf::from),
        on_timeout: None,
    })
}

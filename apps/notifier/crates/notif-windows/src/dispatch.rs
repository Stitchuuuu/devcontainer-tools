use notif_core::callback::CallbackConfig;
use notif_core::Notification;
use tracing::{debug, info};
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};

use crate::backend::WindowsError;
use crate::callbacks::{action_arguments, click_launch_attr, write_sidecar};
use crate::priority::resolve_scenario;

pub fn dispatch_send(
    notif: &Notification,
    aumid: &str,
    callbacks: &CallbackConfig,
) -> Result<(), WindowsError> {
    let start = std::time::Instant::now();
    let scenario = resolve_scenario(notif.priority);
    debug!(target: "notif::send", ?scenario, "resolved scenario");

    // Mint an id when the caller left it blank AND at least one callback is
    // registered — the activator needs the id as the correlation key back to
    // the sidecar. Without callbacks, id-less sends stay id-less (matches
    // pre-session-3 behaviour).
    let effective_id = notif
        .id
        .clone()
        .unwrap_or_else(|| {
            if callbacks.is_empty() {
                String::new()
            } else {
                uuid::Uuid::new_v4().to_string()
            }
        });

    let xml = build_toast_xml(notif, scenario, callbacks, &effective_id);
    debug!(target: "notif::send", xml = %xml, "toast xml built");

    if !callbacks.is_empty() {
        write_sidecar(&effective_id, notif.sender.key.as_str(), notif, callbacks)?;
    }

    let doc = XmlDocument::new()
        .map_err(|e| WindowsError::with_context("XmlDocument::new", e))?;
    let hxml = HSTRING::from(xml.as_str());
    doc.LoadXml(&hxml)
        .map_err(|e| WindowsError::with_context("XmlDocument::LoadXml", e))?;

    let toast = ToastNotification::CreateToastNotification(&doc)
        .map_err(|e| WindowsError::with_context("ToastNotification::CreateToastNotification", e))?;

    if !effective_id.is_empty() {
        let htag = HSTRING::from(effective_id.as_str());
        toast
            .SetTag(&htag)
            .map_err(|e| WindowsError::with_context("ToastNotification::SetTag", e))?;
    }
    let hgroup = HSTRING::from(notif.sender.key.as_str());
    toast
        .SetGroup(&hgroup)
        .map_err(|e| WindowsError::with_context("ToastNotification::SetGroup", e))?;

    let id_for_log = if effective_id.is_empty() {
        "<none>"
    } else {
        effective_id.as_str()
    };
    info!(
        target: "notif::send",
        sender = %notif.sender.key,
        id = id_for_log,
        aumid,
        callback_bindings = callbacks.len(),
        "dispatching",
    );

    let haumid = HSTRING::from(aumid);
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&haumid).map_err(|e| {
        WindowsError::with_context("ToastNotificationManager::CreateToastNotifierWithId", e)
    })?;
    debug!(target: "notif::send", aumid, "CreateToastNotifier OK");

    notifier
        .Show(&toast)
        .map_err(|e| WindowsError::with_context("ToastNotifier::Show", e))?;
    info!(
        target: "notif::send",
        elapsed_ms = start.elapsed().as_millis() as u64,
        "dispatched",
    );
    Ok(())
}

fn build_toast_xml(
    notif: &Notification,
    scenario: Option<&str>,
    callbacks: &CallbackConfig,
    notif_id: &str,
) -> String {
    let title = escape_xml_text(&notif.title);
    let body = escape_xml_text(&notif.body);
    let subtitle_line = notif
        .subtitle
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<text>{}</text>", escape_xml_text(s)))
        .unwrap_or_default();
    let scenario_attr = scenario
        .map(|s| format!(r#" scenario="{s}""#))
        .unwrap_or_default();
    let launch_attr = if callbacks.on_click.is_some() && !notif_id.is_empty() {
        format!(
            r#" launch="{}" activationType="foreground""#,
            escape_xml_text(&click_launch_attr(notif_id)),
        )
    } else {
        String::new()
    };
    let actions_block = if callbacks.on_actions.is_empty() || notif_id.is_empty() {
        String::new()
    } else {
        let mut inner = String::new();
        for (label, _target) in &callbacks.on_actions {
            let args = action_arguments(label, notif_id);
            inner.push_str(&format!(
                r#"<action content="{}" arguments="{}" activationType="foreground"/>"#,
                escape_xml_text(label),
                escape_xml_text(&args),
            ));
        }
        format!("<actions>{inner}</actions>")
    };
    format!(
        "<toast{scenario_attr}{launch_attr}><visual><binding template=\"ToastGeneric\"><text>{title}</text>{subtitle_line}<text>{body}</text></binding></visual>{actions_block}<audio silent=\"true\"/></toast>"
    )
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use notif_core::callback::{CallbackKind, CallbackTarget};
    use notif_core::{Notification, Priority, Sender};

    fn notif(title: &str, body: &str, subtitle: Option<&str>) -> Notification {
        Notification {
            title: title.to_string(),
            body: body.to_string(),
            subtitle: subtitle.map(str::to_string),
            priority: Priority::Normal,
            sender: Sender::default(),
            id: None,
            sound: None,
            image: None,
            on_timeout: None,
        }
    }

    fn empty_callbacks() -> CallbackConfig {
        CallbackConfig::default()
    }

    fn file_target(path: &str) -> CallbackTarget {
        CallbackTarget { kind: CallbackKind::File, payload: path.to_string() }
    }

    #[test]
    fn basic_toast_shape() {
        let n = notif("hello", "world", None);
        let xml = build_toast_xml(&n, None, &empty_callbacks(), "");
        assert!(xml.starts_with("<toast>"));
        assert!(xml.contains("<text>hello</text>"));
        assert!(xml.contains("<text>world</text>"));
        assert!(xml.contains("<audio silent=\"true\"/>"));
        assert!(!xml.contains("scenario="));
        assert!(!xml.contains("<actions>"));
        assert!(!xml.contains("launch="));
    }

    #[test]
    fn subtitle_inserted_between_title_and_body() {
        let n = notif("T", "B", Some("S"));
        let xml = build_toast_xml(&n, None, &empty_callbacks(), "");
        let t_idx = xml.find("<text>T</text>").unwrap();
        let s_idx = xml.find("<text>S</text>").unwrap();
        let b_idx = xml.find("<text>B</text>").unwrap();
        assert!(t_idx < s_idx && s_idx < b_idx, "expected title < subtitle < body ordering");
    }

    #[test]
    fn scenario_attribute_when_set() {
        let n = notif("t", "b", None);
        let xml = build_toast_xml(&n, Some("urgent"), &empty_callbacks(), "");
        assert!(xml.starts_with("<toast scenario=\"urgent\">"));
    }

    #[test]
    fn xml_escapes_special_chars() {
        let n = notif("A & B", "<script>", None);
        let xml = build_toast_xml(&n, None, &empty_callbacks(), "");
        assert!(xml.contains("A &amp; B"));
        assert!(xml.contains("&lt;script&gt;"));
        assert!(!xml.contains("<script>"));
    }

    #[test]
    fn empty_subtitle_omitted() {
        let n = notif("T", "B", Some(""));
        let xml = build_toast_xml(&n, None, &empty_callbacks(), "");
        assert!(!xml.contains("<text></text>"));
    }

    #[test]
    fn on_click_emits_launch_attribute() {
        let n = notif("T", "B", None);
        let cb = CallbackConfig {
            on_click: Some(file_target("/tmp/x")),
            ..CallbackConfig::default()
        };
        let xml = build_toast_xml(&n, None, &cb, "abc-123");
        assert!(
            xml.contains(r#"launch="body::abc-123""#),
            "expected launch attr, got {xml}",
        );
        assert!(xml.contains(r#"activationType="foreground""#));
    }

    #[test]
    fn on_click_without_id_skips_launch_attribute() {
        // Defensive : the dispatcher mints an id when callbacks are set, but
        // guard against a caller that somehow reaches build_toast_xml with an
        // empty id AND on_click — better a no-launch toast than a broken
        // `launch="body::"`.
        let n = notif("T", "B", None);
        let cb = CallbackConfig {
            on_click: Some(file_target("/tmp/x")),
            ..CallbackConfig::default()
        };
        let xml = build_toast_xml(&n, None, &cb, "");
        assert!(!xml.contains("launch="));
    }

    #[test]
    fn on_actions_emit_actions_block_in_order() {
        let n = notif("T", "B", None);
        let cb = CallbackConfig {
            on_actions: vec![
                ("Allow".into(), file_target("/tmp/a")),
                ("Deny".into(), file_target("/tmp/d")),
            ],
            ..CallbackConfig::default()
        };
        let xml = build_toast_xml(&n, None, &cb, "id-1");
        let allow_idx = xml.find(r#"content="Allow""#).unwrap();
        let deny_idx = xml.find(r#"content="Deny""#).unwrap();
        assert!(allow_idx < deny_idx, "expected registration order preserved");
        assert!(xml.contains(r#"arguments="action:Allow::id-1""#));
        assert!(xml.contains(r#"arguments="action:Deny::id-1""#));
        assert!(xml.contains("<actions>"));
        assert!(xml.contains("</actions>"));
    }

    #[test]
    fn action_labels_are_xml_escaped() {
        let n = notif("T", "B", None);
        let cb = CallbackConfig {
            on_actions: vec![("Yes & No".into(), file_target("/tmp/x"))],
            ..CallbackConfig::default()
        };
        let xml = build_toast_xml(&n, None, &cb, "id-1");
        assert!(xml.contains(r#"content="Yes &amp; No""#));
    }
}

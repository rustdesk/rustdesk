use crate::{
    server::{service::Subscriber, ConnInner},
    virtual_display_manager,
};
use base::message_proto::{
    DisplayInfo, Message, MessageBox, PeerInfo, Resolution, ToggleVirtualDisplay,
};
use hbb_common::{log, tokio, ResultType};
use std::sync::Arc;

pub(in crate::server) fn get_platform_additions() -> serde_json::Map<String, serde_json::Value> {
    let mut additions = serde_json::Map::new();
    if virtual_display_manager::is_virtual_display_supported() {
        let displays = virtual_display_manager::get_virtual_displays();
        additions.insert("macos_virtual_display_supported".into(), true.into());
        additions.insert(
            "virtual_display_native_scale".into(),
            serde_json::json!([1, 2]),
        );
        additions.insert("macos_virtual_displays".into(), serde_json::json!(displays));
    }
    additions
}

// Called on the display service thread, or via spawn_blocking during login.
// Capture scale is deliberately not used: multi-capture may disable Retina capture.
pub(super) fn display_modes(displays: &[DisplayInfo]) -> serde_json::Value {
    let mut modes = serde_json::Map::new();
    for (index, display) in displays.iter().enumerate() {
        let Ok(id) = display.name.parse::<u32>() else {
            continue;
        };
        if let Some(mode) = virtual_display_manager::display_mode(id) {
            modes.insert(
                index.to_string(),
                serde_json::json!([mode.width, mode.height, mode.scale, id]),
            );
        }
    }
    modes.into()
}

pub(super) struct DisplayModeMonitor {
    modes: serde_json::Value,
}

impl DisplayModeMonitor {
    pub(super) fn new() -> Self {
        Self {
            modes: serde_json::json!({}),
        }
    }

    pub(super) fn check_changed(&mut self, displays: &[DisplayInfo]) -> bool {
        self.update(display_modes(displays))
    }

    fn update(&mut self, modes: serde_json::Value) -> bool {
        if self.modes == modes {
            return false;
        }
        self.modes = modes;
        true
    }
}

pub(in crate::server) async fn sync_display_modes(peer: &mut PeerInfo) {
    let displays = peer.displays.clone();
    match tokio::task::spawn_blocking(move || display_modes(&displays)).await {
        Ok(modes) => {
            let mut additions = match serde_json::from_str::<
                serde_json::Map<String, serde_json::Value>,
            >(&peer.platform_additions)
            {
                Ok(additions) => additions,
                Err(error) => {
                    log::error!("Failed to decode local platform additions: {error}");
                    return;
                }
            };
            additions.insert("macos_virtual_display_modes".into(), modes);
            peer.platform_additions = serde_json::Value::Object(additions).to_string();
        }
        Err(error) => log::error!("Failed to query native virtual display modes: {error}"),
    }
}

pub(in crate::server) async fn toggle(
    conn_id: i32,
    request: ToggleVirtualDisplay,
) -> Option<Message> {
    let result = virtual_display_manager::toggle(conn_id, request.display, request.on).await;
    super::refresh_virtual_displays();
    result
        .err()
        .map(|error| error_message("Virtual display", error))
}

pub(in crate::server) fn resize(
    mut connection: ConnInner,
    name: &str,
    resolution: &Resolution,
    scale: Option<u32>,
) {
    // Queue before spawning so disconnect cleanup cannot overtake this request.
    let pending = match scale {
        None => virtual_display_manager::resize(
            connection.id(),
            name,
            resolution.width,
            resolution.height,
        ),
        Some(scale) => virtual_display_manager::configure(
            connection.id(),
            name,
            resolution.width,
            resolution.height,
            scale,
        ),
    };
    let pending = match pending {
        Ok(pending) => pending,
        Err(error) => {
            connection.send(Arc::new(error_message("Resolution", error)));
            return;
        }
    };
    tokio::spawn(async move {
        let result: ResultType<()> = async { pending.await? }.await;
        super::refresh_virtual_displays();
        if let Err(error) = result {
            connection.send(Arc::new(error_message("Resolution", error)));
        }
    });
}

fn error_message(title: &str, error: hbb_common::anyhow::Error) -> Message {
    log::error!("{error}");
    let mut message = Message::new();
    message.set_message_box(MessageBox {
        msgtype: "nook-nocancel-hasclose".into(),
        title: title.into(),
        text: error.to_string(),
        ..Default::default()
    });
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::message;
    use std::time::Duration;

    #[test]
    fn mode_monitor_detects_native_changes_without_capture_changes() {
        let mut monitor = DisplayModeMonitor::new();
        let hidpi = serde_json::json!({"0": [2560, 1600, 2, 42]});
        let low_resolution = serde_json::json!({"0": [1280, 800, 1, 42]});
        assert!(!monitor.update(serde_json::json!({})));
        assert!(monitor.update(hidpi.clone()));
        assert!(!monitor.update(hidpi));
        assert!(monitor.update(low_resolution.clone()));
        assert!(!monitor.update(low_resolution));
        assert!(monitor.update(serde_json::json!({})));
        assert!(!monitor.update(serde_json::json!({})));
        assert!(monitor.update(serde_json::json!({"1": [1280, 800, 1, 43]})));
    }

    #[tokio::test]
    async fn native_mode_metadata_preserves_other_capabilities() {
        let mut peer = PeerInfo {
            platform_additions: r#"{"other_capability":true}"#.into(),
            displays: vec![DisplayInfo {
                name: "0".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        sync_display_modes(&mut peer).await;
        let additions: serde_json::Value = serde_json::from_str(&peer.platform_additions).unwrap();
        assert_eq!(additions["other_capability"], true);
        assert_eq!(
            additions["macos_virtual_display_modes"],
            serde_json::json!({})
        );
    }

    fn assert_error(message: &Message, title: &str, text: &str) {
        let Some(message::Union::MessageBox(error)) = message.union.as_ref() else {
            panic!("Expected a display settings error response");
        };
        assert_eq!(error.title, title);
        assert_eq!(error.text, text);
        assert_eq!(error.msgtype, "nook-nocancel-hasclose");
    }

    #[tokio::test]
    async fn resize_validation_and_worker_errors_reach_the_requesting_connection() {
        for (width, scale, expected) in [
            (
                0,
                Some(1),
                "Virtual display dimensions must be between 320 and 4096",
            ),
            (1920, Some(1), "Remote connection has closed"),
            (1920, None, "Remote connection has closed"),
        ] {
            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
            resize(
                ConnInner::new(i32::MIN, Some(sender), None),
                "0",
                &Resolution {
                    width,
                    height: 1080,
                    ..Default::default()
                },
                scale,
            );
            let (_, message) = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            assert_error(&message, "Resolution", expected);
        }
    }

    #[tokio::test]
    async fn closed_connections_cannot_toggle_displays() {
        let message = toggle(
            i32::MIN,
            ToggleVirtualDisplay {
                display: 1,
                on: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_error(&message, "Virtual display", "Remote connection has closed");
    }
}

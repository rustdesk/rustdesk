use crate::{
    server::{service::Subscriber, ConnInner},
    virtual_display_manager,
};
use base::message_proto::{
    DisplayInfo, Message, MessageBox, Misc, PeerInfo, Resolution, ToggleVirtualDisplay,
    VirtualDisplayMode,
};
use hbb_common::{bail, log, tokio, ResultType};
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
        let modes = display_modes(displays);
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
) {
    // Queue before spawning so disconnect cleanup cannot overtake this request.
    let pending = virtual_display_manager::resize(
        connection.id(),
        name,
        resolution.width,
        resolution.height,
    );
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

pub(in crate::server) fn configure(
    mut connection: ConnInner,
    request: VirtualDisplayMode,
    allowed: bool,
) {
    let display_id = request.display_id;
    let pending = (|| {
        if !allowed {
            bail!("No permission to change display settings.");
        }
        if request.request_id.is_empty() || request.request_id.len() > 64 {
            bail!("Failed to resize macOS virtual display");
        }
        virtual_display_manager::configure(
            connection.id(),
            &display_id.to_string(),
            request.width,
            request.height,
            request.scale,
        )
    })();
    tokio::spawn(async move {
        let result: ResultType<_> = async {
            let result = pending?.await?;
            super::refresh_virtual_displays();
            result?;
            tokio::task::spawn_blocking(move || virtual_display_manager::display_mode(display_id))
                .await?
                .ok_or_else(|| hbb_common::anyhow::anyhow!("Display settings changed. Reopen the resolution menu and try again."))
        }
        .await;
        let response = match result {
            Ok(mode) => serde_json::json!({
                "request_id": request.request_id,
                "display_id": display_id,
                "width": mode.width,
                "height": mode.height,
                "scale": mode.scale,
            }),
            Err(error) => {
                log::debug!("Virtual display mode: {error}");
                serde_json::json!({"request_id": request.request_id, "error": error.to_string()})
            }
        };
        let mut misc = Misc::new();
        misc.set_virtual_display_mode_response(response.to_string());
        let mut message = Message::new();
        message.set_misc(misc);
        connection.send(Arc::new(message));
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
        for (width, expected) in [
            (
                0,
                "Virtual display dimensions must be between 1 and 4096 logical pixels",
            ),
            (1920, "Remote connection has closed"),
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
            );
            let (_, message) = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            assert_error(&message, "Resolution", expected);
        }
    }

    #[tokio::test]
    async fn configure_errors_return_correlated_results() {
        for (allowed, expected) in [
            (false, "No permission to change display settings."),
            (true, "Remote connection has closed"),
        ] {
            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
            configure(
                ConnInner::new(i32::MIN, Some(sender), None),
                VirtualDisplayMode {
                    request_id: "mode-1".into(),
                    width: 1920,
                    height: 1080,
                    scale: 1,
                    ..Default::default()
                },
                allowed,
            );
            let (_, message) = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
                .await
                .unwrap()
                .unwrap();
            let response: serde_json::Value =
                serde_json::from_str(message.misc().virtual_display_mode_response()).unwrap();
            assert_eq!(response["request_id"], "mode-1");
            assert_eq!(response["error"], expected);
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

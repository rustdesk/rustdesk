use super::Connection;
use crate::{
    ipc::Data,
    server::{display_service, service::Subscriber, video_service::VideoSource},
};

impl Connection {
    pub(super) fn send_screen_frame_displays(&self) {
        if !self.authorized || !self.is_remote() {
            return;
        }
        // This option is off by default; avoid polling display subscriptions and sending
        // IPC until enabled. The existing second timer picks up changes to the option.
        if !hbb_common::config::Config::get_bool_option(
            base::config::keys::OPTION_ALLOW_SCREEN_FRAME,
        ) {
            return;
        }
        let Some(server) = self.server.upgrade() else {
            return;
        };
        // Use actual subscriptions, including CaptureDisplays multi-window sessions. The existing
        // second timer also refreshes names after display re-enumeration without a new lifecycle.
        let displays = display_service::get_sync_displays();
        let names = server
            .read()
            .unwrap()
            .services
            .iter()
            .filter(|(_, service)| service.is_subed(self.inner.id()))
            .filter_map(|(name, _)| name.strip_prefix(VideoSource::Monitor.service_name_prefix()))
            .filter_map(|index| index.parse::<usize>().ok())
            .map(|index| displays.get(index).map(|display| display.name.clone()))
            .collect::<Option<Vec<_>>>()
            .map(|mut names| {
                names.sort_unstable();
                names.dedup();
                names
            });
        // A missing display mapping is unknown, not proof that a monitor is unviewed.
        if let Err(err) = self.tx_to_cm.send(Data::ScreenFrameDisplays(names)) {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(5),
                warn,
                "Failed to report viewed screen frame displays: {err}"
            );
        }
    }
}

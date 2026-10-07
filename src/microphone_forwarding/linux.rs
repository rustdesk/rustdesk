use hbb_common::{anyhow::anyhow, ResultType};
use std::process::Command;
fn pactl(args: &[&str]) -> ResultType<String> {
    let result = Command::new("pactl").args(args).output()?;
    if !result.status.success() {
        return Err(anyhow!(
            "pactl: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}
pub struct Route {
    previous: String,
    sink: String,
    source: String,
    restored: bool,
}
impl Route {
    pub fn open() -> ResultType<Self> {
        let previous = pactl(&["get-default-source"])?;
        let sink = pactl(&[
            "load-module",
            "module-null-sink",
            "sink_name=rustdesk_microphone",
            "rate=48000",
            "channels=2",
        ])?;
        let source = match pactl(&[
            "load-module",
            "module-remap-source",
            "master=rustdesk_microphone.monitor",
            "source_name=rustdesk_microphone_input",
            "source_properties=device.description=RustDesk_Microphone",
        ]) {
            Ok(value) => value,
            Err(error) => {
                if let Err(cleanup) = pactl(&["unload-module", &sink]) {
                    hbb_common::log::warn!("{cleanup}");
                }
                return Err(error);
            }
        };
        let mut route = Self {
            previous,
            sink,
            source,
            restored: false,
        };
        if let Err(error) = pactl(&["set-default-source", "rustdesk_microphone_input"]) {
            route.restore();
            return Err(error);
        }
        Ok(route)
    }
    pub fn output(&self) -> &str {
        "rustdesk_microphone"
    }
    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        if pactl(&["get-default-source"]).ok().as_deref() == Some("rustdesk_microphone_input") {
            if let Err(error) = pactl(&["set-default-source", &self.previous]) {
                hbb_common::log::warn!("{error}");
            }
        }
        for module in [&self.source, &self.sink] {
            if let Err(error) = pactl(&["unload-module", module]) {
                hbb_common::log::warn!("{error}");
            }
        }
        self.restored = true;
    }
}

use super::super::{HIGH_RESOLUTION_SCROLL_DEVICE_NAME, SCROLL_DEVICE_NAME_KEY};
use crate::ipc::{Connection, Data, DataMouse};
use hbb_common::{log, ResultType};
use std::sync::atomic::{AtomicU64, Ordering};

static DEVICE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn device_name() -> String {
    format!(
        "{} {}-{}",
        HIGH_RESOLUTION_SCROLL_DEVICE_NAME,
        std::process::id(),
        DEVICE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) async fn next_scroll(
    stream: &mut Connection,
    device_name: &str,
) -> ResultType<Option<(i32, i32)>> {
    loop {
        match stream.next().await? {
            Some(Data::Mouse(DataMouse::ScrollHighResolution(x, y))) => return Ok(Some((x, y))),
            // Old clients skip this query and can send scroll events immediately.
            Some(Data::Config((key, None))) if key == SCROLL_DEVICE_NAME_KEY => {
                stream
                    .send(&Data::Config((key, Some(device_name.to_owned()))))
                    .await?;
            }
            Some(data) => log::warn!("Unexpected high-resolution uinput data: {data:?}"),
            None => return Ok(None),
        }
    }
}

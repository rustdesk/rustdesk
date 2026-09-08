use super::{
    smooth_scroll::SmoothScrollDevice, HIGH_RESOLUTION_SCROLL_DEVICE_READY_DELAY_MS,
    SCROLL_SERVICE_READY_KEY, SMOOTH_SCROLL_DEVICE_NAME_PREFIX, SMOOTH_SCROLL_READY_POLL_MS,
    SMOOTH_SCROLL_READY_TIMEOUT_MS,
};
use crate::ipc::{self, Data, DataMouse};
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent,
};
use hbb_common::{anyhow::anyhow, bail, log, tokio, ResultType};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

mod high_resolution_connection;

static SMOOTH_SCROLL_DEVICE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn split_high_resolution_scroll(remainder: i32, delta: i32) -> (i32, i32) {
    let total = i64::from(remainder) + i64::from(delta);
    let step = i64::from(enigo::HIGH_RESOLUTION_SCROLL_UNITS_PER_STEP);
    ((total / step) as i32, (total % step) as i32)
}

const HIGH_RESOLUTION_SCROLL_BUTTON: evdev::Key = evdev::Key::BTN_LEFT;
const HIGH_RESOLUTION_SCROLL_AXIS_COUNT: usize = 2;
const HIGH_RESOLUTION_SCROLL_AXES: [evdev::RelativeAxisType; 6] = [
    evdev::RelativeAxisType::REL_X,
    evdev::RelativeAxisType::REL_Y,
    evdev::RelativeAxisType::REL_WHEEL,
    evdev::RelativeAxisType::REL_HWHEEL,
    evdev::RelativeAxisType::REL_WHEEL_HI_RES,
    evdev::RelativeAxisType::REL_HWHEEL_HI_RES,
];

struct HighResolutionScrollDevice {
    device: VirtualDevice,
    remainders: [i32; HIGH_RESOLUTION_SCROLL_AXIS_COUNT],
}

fn high_resolution_scroll_axis_events(
    horizontal: bool,
    length: i32,
    remainder: i32,
) -> ResultType<(Vec<InputEvent>, i32)> {
    if length == 0 {
        return Ok((Vec::new(), remainder));
    }
    let (high_axis, legacy_axis, delta) = if horizontal {
        (
            evdev::RelativeAxisType::REL_HWHEEL_HI_RES,
            evdev::RelativeAxisType::REL_HWHEEL,
            length,
        )
    } else {
        (
            evdev::RelativeAxisType::REL_WHEEL_HI_RES,
            evdev::RelativeAxisType::REL_WHEEL,
            length
                .checked_neg()
                .ok_or_else(|| anyhow!("vertical scroll delta overflow"))?,
        )
    };
    let (legacy_delta, remainder) = split_high_resolution_scroll(remainder, delta);
    let mut events = vec![InputEvent::new(EventType::RELATIVE, high_axis.0, delta)];
    if legacy_delta != 0 {
        events.push(InputEvent::new(
            EventType::RELATIVE,
            legacy_axis.0,
            legacy_delta,
        ));
    }
    Ok((events, remainder))
}

fn high_resolution_scroll_events(
    x: i32,
    y: i32,
    remainders: [i32; HIGH_RESOLUTION_SCROLL_AXIS_COUNT],
) -> ResultType<(Vec<InputEvent>, [i32; HIGH_RESOLUTION_SCROLL_AXIS_COUNT])> {
    let mut events = Vec::new();
    let mut next_remainders = remainders;
    for (horizontal, length) in [(false, y), (true, x)] {
        let index = usize::from(horizontal);
        let (axis_events, remainder) =
            high_resolution_scroll_axis_events(horizontal, length, remainders[index])?;
        events.extend(axis_events);
        next_remainders[index] = remainder;
    }
    Ok((events, next_remainders))
}

impl HighResolutionScrollDevice {
    fn new(device_name: &str) -> ResultType<Self> {
        // udev needs mouse buttons and relative coordinates to classify this as a mouse.
        let keys: AttributeSet<_> = [HIGH_RESOLUTION_SCROLL_BUTTON].into_iter().collect();
        let axes: AttributeSet<_> = HIGH_RESOLUTION_SCROLL_AXES.into_iter().collect();
        let device = VirtualDeviceBuilder::new()?
            .name(device_name)
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self {
            device,
            remainders: [0; HIGH_RESOLUTION_SCROLL_AXIS_COUNT],
        })
    }

    fn scroll(&mut self, x: i32, y: i32) -> ResultType<()> {
        let (events, remainders) = high_resolution_scroll_events(x, y, self.remainders)?;
        if events.is_empty() {
            return Ok(());
        }
        self.device.emit(&events)?;
        self.remainders = remainders;
        Ok(())
    }
}

pub(super) fn spawn_high_resolution_scroll_handler(mut stream: ipc::Connection) {
    tokio::spawn(async move {
        let device_name = high_resolution_connection::device_name();
        let mut mouse = match HighResolutionScrollDevice::new(&device_name) {
            Ok(mouse) => mouse,
            Err(err) => {
                log::error!("Failed to create high-resolution uinput scroll device: {err}");
                return;
            }
        };
        tokio::time::sleep(Duration::from_millis(
            HIGH_RESOLUTION_SCROLL_DEVICE_READY_DELAY_MS,
        ))
        .await;
        if let Err(err) = stream
            .send(&Data::Config((SCROLL_SERVICE_READY_KEY.to_owned(), None)))
            .await
        {
            log::error!("Failed to acknowledge high-resolution uinput scroll device: {err}");
            return;
        }
        loop {
            let (x, y) =
                match high_resolution_connection::next_scroll(&mut stream, &device_name).await {
                    Ok(Some(delta)) => delta,
                    Ok(None) => break,
                    Err(err) => {
                        log::info!("High-resolution uinput ipc connection closed: {err}");
                        break;
                    }
                };
            if let Err(err) = mouse.scroll(x, y) {
                log::error!("Failed to inject high-resolution uinput scroll: {err}");
                continue;
            }
            if let Err(err) = acknowledge_scroll_finish(&mut stream, (x, y)).await {
                log::error!("Failed to acknowledge high-resolution scroll finish: {err}");
                break;
            }
        }
    });
}

pub(super) fn spawn_smooth_scroll_handler(mut stream: ipc::Connection) {
    tokio::spawn(async move {
        let (mut touchpad, device_name) = match prepare_smooth_scroll_device().await {
            Ok(device) => device,
            Err(err) => {
                log::error!("Failed to prepare smooth uinput scroll device: {err}");
                return;
            }
        };
        if let Err(err) = stream
            .send(&Data::Config((
                SCROLL_SERVICE_READY_KEY.to_owned(),
                Some(device_name),
            )))
            .await
        {
            log::error!("Failed to acknowledge smooth uinput scroll device: {err}");
            return;
        }
        loop {
            let (x, y) = match stream.next().await {
                Ok(Some(Data::Mouse(DataMouse::ScrollSmooth(x, y)))) => (x, y),
                Ok(Some(data)) => {
                    log::warn!("Unexpected smooth uinput data: {data:?}");
                    continue;
                }
                Ok(None) => break,
                Err(err) => {
                    log::info!("Smooth uinput ipc connection closed: {err}");
                    break;
                }
            };
            if let Err(err) = touchpad.scroll(x, y).await {
                log::error!("Failed to inject smooth uinput scroll: {err}");
                break;
            }
            if let Err(err) = acknowledge_scroll_finish(&mut stream, (x, y)).await {
                log::error!("Failed to acknowledge smooth scroll finish: {err}");
                break;
            }
        }
    });
}

async fn prepare_smooth_scroll_device() -> ResultType<(SmoothScrollDevice, String)> {
    let device_name = format!(
        "{} {}-{}",
        SMOOTH_SCROLL_DEVICE_NAME_PREFIX,
        std::process::id(),
        SMOOTH_SCROLL_DEVICE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let touchpad = SmoothScrollDevice::new(device_name.as_bytes())?;
    let touchpad = wait_for_smooth_scroll_device_ready(touchpad).await?;
    Ok((touchpad, device_name))
}

async fn wait_for_smooth_scroll_device_ready(
    touchpad: SmoothScrollDevice,
) -> ResultType<SmoothScrollDevice> {
    tokio::task::spawn_blocking(move || -> ResultType<_> {
        let attempts = SMOOTH_SCROLL_READY_TIMEOUT_MS / SMOOTH_SCROLL_READY_POLL_MS;
        for _ in 0..attempts {
            if touchpad.is_classified_as_touchpad()? {
                return Ok(touchpad);
            }
            std::thread::sleep(Duration::from_millis(SMOOTH_SCROLL_READY_POLL_MS));
        }
        bail!("udev did not classify the uinput device as a touchpad")
    })
    .await?
}

async fn acknowledge_scroll_finish(
    stream: &mut ipc::Connection,
    delta: (i32, i32),
) -> ResultType<()> {
    if delta == (0, 0) {
        stream.send(&Data::Empty).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_legacy_companion_events_at_detent_boundaries() {
        let half_step = enigo::HIGH_RESOLUTION_SCROLL_UNITS_PER_STEP / 2;

        assert_eq!(split_high_resolution_scroll(0, half_step), (0, half_step));
        assert_eq!(split_high_resolution_scroll(half_step, half_step), (1, 0));
        assert_eq!(
            split_high_resolution_scroll(-half_step, -half_step),
            (-1, 0)
        );
        assert_eq!(split_high_resolution_scroll(100, -half_step), (0, 40));
    }

    #[test]
    fn batches_diagonal_scroll_axes_in_one_frame() {
        let half_step = enigo::HIGH_RESOLUTION_SCROLL_UNITS_PER_STEP / 2;
        let (events, remainders) = high_resolution_scroll_events(
            half_step,
            -half_step,
            [0; HIGH_RESOLUTION_SCROLL_AXIS_COUNT],
        )
        .unwrap();

        assert_eq!(remainders, [half_step, half_step]);
        assert_eq!(
            events.iter().map(InputEvent::code).collect::<Vec<_>>(),
            vec![
                evdev::RelativeAxisType::REL_WHEEL_HI_RES.0,
                evdev::RelativeAxisType::REL_HWHEEL_HI_RES.0,
            ]
        );
    }
}

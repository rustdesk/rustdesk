use enigo::SMOOTH_SCROLL_UNITS_PER_POINT;
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisType, AttributeSet, BusType, EventType, InputEvent, InputId, Key,
    PropType, UinputAbsSetup,
};
use hbb_common::{log, tokio};
use std::{io, path::PathBuf};

mod motion;
mod udev;

use motion::{fitting_delta, initial_position, moved_positions};

const DEVICE_VENDOR: u16 = 0x5255;
const DEVICE_PRODUCT: u16 = 0x5353;
const DEVICE_VERSION: u16 = 1;
const AXIS_MIN: i32 = 0;
// Increase coordinate precision without changing the virtual touchpad's physical dimensions.
const COORDINATE_RANGE_SCALE: i32 = 10;
const AXIS_X_MAX: i32 = 12_000 * COORDINATE_RANGE_SCALE;
const AXIS_Y_MAX: i32 = 8_000 * COORDINATE_RANGE_SCALE;
const AXIS_RESOLUTION: i32 = 100 * COORDINATE_RANGE_SCALE;
const EDGE_MARGIN: i32 = 1_000 * COORDINATE_RANGE_SCALE;
const FINGER_GAP: i32 = 1_000 * COORDINATE_RANGE_SCALE;
const FINGER_GAP_HALF: i32 = FINGER_GAP / 2;
const SLOT_MIN: i32 = 0;
const SLOT_MAX: i32 = 1;
const TRACKING_ID_MIN: i32 = 0;
const TRACKING_ID_MAX: i32 = 65_535;
const INITIAL_TRACKING_ID: i32 = 1;
const RELEASED_TRACKING_ID: i32 = -1;
// Preserve the existing Linux 0.06 scale as physical motion at the scaled resolution.
const TOUCHPAD_UNITS_PER_POINT: i32 = 6 * COORDINATE_RANGE_SCALE;
const RELEASED: i32 = 0;
const PRESSED: i32 = 1;
const NO_RESOLUTION: i32 = 0;
const AXIS_FUZZ: i32 = 0;
const AXIS_FLAT: i32 = 0;
const MISSING_EVENT_ERROR: &str = "uinput event device was not created";

pub(super) struct SmoothScrollDevice {
    device: VirtualDevice,
    event_path: PathBuf,
    active: bool,
    positions: [(i32, i32); 2],
    remainders: (i32, i32),
    next_tracking_id: i32,
}

struct AxisConfig {
    axis: AbsoluteAxisType,
    minimum: i32,
    maximum: i32,
    resolution: i32,
}

#[rustfmt::skip]
const AXIS_CONFIGS: [AxisConfig; 6] = [
    AxisConfig { axis: AbsoluteAxisType::ABS_X,              minimum: AXIS_MIN,        maximum: AXIS_X_MAX,       resolution: AXIS_RESOLUTION },
    AxisConfig { axis: AbsoluteAxisType::ABS_Y,              minimum: AXIS_MIN,        maximum: AXIS_Y_MAX,       resolution: AXIS_RESOLUTION },
    AxisConfig { axis: AbsoluteAxisType::ABS_MT_SLOT,        minimum: SLOT_MIN,        maximum: SLOT_MAX,         resolution: NO_RESOLUTION },
    AxisConfig { axis: AbsoluteAxisType::ABS_MT_POSITION_X,  minimum: AXIS_MIN,        maximum: AXIS_X_MAX,       resolution: AXIS_RESOLUTION },
    AxisConfig { axis: AbsoluteAxisType::ABS_MT_POSITION_Y,  minimum: AXIS_MIN,        maximum: AXIS_Y_MAX,       resolution: AXIS_RESOLUTION },
    AxisConfig { axis: AbsoluteAxisType::ABS_MT_TRACKING_ID, minimum: TRACKING_ID_MIN, maximum: TRACKING_ID_MAX,  resolution: NO_RESOLUTION },
];

impl SmoothScrollDevice {
    pub(super) fn new(device_name: &[u8]) -> io::Result<Self> {
        let mut device = create_device(device_name)?;
        let event_path = device
            .enumerate_dev_nodes_blocking()?
            .next()
            .transpose()?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, MISSING_EVENT_ERROR))?;
        Ok(Self {
            device,
            event_path,
            active: false,
            positions: [(AXIS_MIN, AXIS_MIN); 2],
            remainders: (0, 0),
            next_tracking_id: INITIAL_TRACKING_ID,
        })
    }

    pub(super) async fn scroll(&mut self, x: i32, y: i32) -> io::Result<()> {
        if x == 0 && y == 0 {
            return self.finish();
        }
        let (dx, x_remainder) = convert_delta(self.remainders.0, x)?;
        let (dy, y_remainder) = convert_delta(self.remainders.1, y)?;
        // One input update must not outlive the IPC acknowledgement budget.
        match tokio::time::timeout(
            std::time::Duration::from_millis(super::IPC_REQUEST_TIMEOUT),
            self.move_delta((dx, dy)),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                return self.finish_after_error(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "smooth scroll update exceeded the IPC response deadline",
                ));
            }
        }
        self.remainders = (x_remainder, y_remainder);
        Ok(())
    }

    pub(super) fn is_classified_as_touchpad(&self) -> io::Result<bool> {
        udev::is_classified_as_touchpad(&self.event_path)
    }

    fn start(&mut self, dx: i32, dy: i32) -> io::Result<()> {
        let center_x = initial_position(dx, AXIS_X_MAX, EDGE_MARGIN + FINGER_GAP_HALF);
        let center_y = initial_position(dy, AXIS_Y_MAX, EDGE_MARGIN);
        let positions = [
            (center_x - FINGER_GAP_HALF, center_y),
            (center_x + FINGER_GAP_HALF, center_y),
        ];
        let (first_tracking_id, second_tracking_id) = self.take_tracking_ids();
        let events = [
            key_event(Key::BTN_TOUCH, PRESSED),
            key_event(Key::BTN_TOOL_DOUBLETAP, PRESSED),
            axis_event(AbsoluteAxisType::ABS_X, positions[0].0),
            axis_event(AbsoluteAxisType::ABS_Y, positions[0].1),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MIN),
            axis_event(AbsoluteAxisType::ABS_MT_TRACKING_ID, first_tracking_id),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_X, positions[0].0),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_Y, positions[0].1),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MAX),
            axis_event(AbsoluteAxisType::ABS_MT_TRACKING_ID, second_tracking_id),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_X, positions[1].0),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_Y, positions[1].1),
        ];
        self.device.emit(&events)?;
        self.positions = positions;
        self.active = true;
        Ok(())
    }

    fn take_tracking_ids(&mut self) -> (i32, i32) {
        let first = self.next_tracking_id;
        let second = next_tracking_id(first);
        self.next_tracking_id = next_tracking_id(second);
        (first, second)
    }

    async fn move_delta(&mut self, delta: (i32, i32)) -> io::Result<()> {
        let mut remaining = delta;
        while remaining != (0, 0) {
            if !self.active {
                self.start(remaining.0, remaining.1)?;
            }
            let step = fitting_delta(self.positions, remaining);
            if step == (0, 0) {
                self.finish()?;
                tokio::task::yield_now().await;
                continue;
            }
            self.move_contacts(step.0, step.1)?;
            remaining = (remaining.0 - step.0, remaining.1 - step.1);
            if remaining != (0, 0) {
                self.finish()?;
                // Bound each poll's work so other scroll clients and the deadline can run.
                tokio::task::yield_now().await;
            }
        }
        Ok(())
    }

    fn move_contacts(&mut self, dx: i32, dy: i32) -> io::Result<()> {
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        let positions = match moved_positions(self.positions, (dx, dy)) {
            Ok(positions) => positions,
            Err(move_error) => return self.finish_after_error(move_error),
        };
        let events = [
            axis_event(AbsoluteAxisType::ABS_X, positions[0].0),
            axis_event(AbsoluteAxisType::ABS_Y, positions[0].1),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MIN),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_X, positions[0].0),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_Y, positions[0].1),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MAX),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_X, positions[1].0),
            axis_event(AbsoluteAxisType::ABS_MT_POSITION_Y, positions[1].1),
        ];
        self.device.emit(&events)?;
        self.positions = positions;
        Ok(())
    }

    fn finish_after_error(&mut self, move_error: io::Error) -> io::Result<()> {
        match self.finish() {
            Ok(()) => Err(move_error),
            Err(finish_error) => Err(io::Error::new(
                finish_error.kind(),
                format!("{move_error}; failed to release touch contacts: {finish_error}"),
            )),
        }
    }

    fn finish(&mut self) -> io::Result<()> {
        if !self.active {
            self.remainders = (0, 0);
            return Ok(());
        }
        let events = [
            key_event(Key::BTN_TOUCH, RELEASED),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MIN),
            axis_event(AbsoluteAxisType::ABS_MT_TRACKING_ID, RELEASED_TRACKING_ID),
            axis_event(AbsoluteAxisType::ABS_MT_SLOT, SLOT_MAX),
            axis_event(AbsoluteAxisType::ABS_MT_TRACKING_ID, RELEASED_TRACKING_ID),
            key_event(Key::BTN_TOOL_DOUBLETAP, RELEASED),
        ];
        self.device.emit(&events)?;
        self.active = false;
        self.remainders = (0, 0);
        Ok(())
    }
}

impl Drop for SmoothScrollDevice {
    fn drop(&mut self) {
        if let Err(err) = self.finish() {
            log::error!("Failed to release smooth uinput contacts: {err}");
        }
    }
}

fn create_device(device_name: &[u8]) -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for key in [
        // Keep libinput's default tap-to-click setting disabled.
        Key::BTN_LEFT,
        Key::BTN_TOUCH,
        Key::BTN_TOOL_FINGER,
        Key::BTN_TOOL_DOUBLETAP,
    ] {
        keys.insert(key);
    }
    let mut properties = AttributeSet::<PropType>::new();
    properties.insert(PropType::POINTER);
    let mut builder = VirtualDeviceBuilder::new()?
        .name(device_name)
        .input_id(InputId::new(
            BusType::BUS_USB,
            DEVICE_VENDOR,
            DEVICE_PRODUCT,
            DEVICE_VERSION,
        ))
        .with_keys(&keys)?
        .with_properties(&properties)?;
    for config in AXIS_CONFIGS {
        let info = AbsInfo::new(
            config.minimum,
            config.minimum,
            config.maximum,
            AXIS_FUZZ,
            AXIS_FLAT,
            config.resolution,
        );
        builder = builder.with_absolute_axis(&UinputAbsSetup::new(config.axis, info))?;
    }
    builder.build()
}

fn convert_delta(remainder: i32, delta: i32) -> io::Result<(i32, i32)> {
    let total = i64::from(remainder) + i64::from(delta) * i64::from(TOUCHPAD_UNITS_PER_POINT);
    let divisor = i64::from(SMOOTH_SCROLL_UNITS_PER_POINT);
    let units = i32::try_from(total / divisor)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "smooth scroll delta overflow"))?;
    Ok((units, (total % divisor) as i32))
}

fn next_tracking_id(current: i32) -> i32 {
    if current == TRACKING_ID_MAX {
        TRACKING_ID_MIN
    } else {
        current + 1
    }
}

fn key_event(key: Key, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY, key.0, value)
}

fn axis_event(axis: AbsoluteAxisType, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE, axis.0, value)
}

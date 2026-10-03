use hbb_common::{anyhow::anyhow, bail, libc, ResultType};
use std::{
    ffi::CStr,
    fs::File,
    io,
    os::{
        fd::{AsRawFd, OwnedFd, RawFd},
        unix::fs::FileExt,
    },
    time::{Duration, Instant},
};
use wayland_client::{
    protocol::{wl_keyboard, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle, WEnum,
};

const KEYMAP_TIMEOUT: Duration = Duration::from_secs(1);
const SEAT_VERSION: u32 = 7;

#[derive(Default)]
struct KeyboardState {
    seat_bound: bool,
    keymap: Option<ResultType<Vec<u8>>>,
}

pub(super) fn read_keymap() -> ResultType<Vec<u8>> {
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    connection.display().get_registry(&queue.handle(), ());
    let mut state = KeyboardState::default();
    let deadline = Instant::now() + KEYMAP_TIMEOUT;
    loop {
        queue.dispatch_pending(&mut state)?;
        if let Some(keymap) = state.keymap.take() {
            return keymap;
        }
        if Instant::now() >= deadline {
            bail!("Timed out reading the Wayland keyboard map");
        }
        queue.flush()?;
        if let Some(reader) = queue.prepare_read() {
            wait_for_read(reader.connection_fd().as_raw_fd(), deadline)?;
            reader.read()?;
        }
    }
}

fn wait_for_read(fd: RawFd, deadline: Instant) -> ResultType<()> {
    let mut descriptor = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = i32::try_from(remaining.as_millis())?;
        let ready = unsafe { libc::poll(&mut descriptor, 1, timeout) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if ready == 0 {
            bail!("Timed out reading the Wayland keyboard map");
        }
        if descriptor.revents & libc::POLLIN == 0 {
            bail!("Wayland keyboard connection closed: {}", descriptor.revents);
        }
        return Ok(());
    }
}

fn read_keymap_file(fd: OwnedFd, size: u32) -> ResultType<Vec<u8>> {
    let file = File::from(fd);
    if file.metadata()?.len() < u64::from(size) {
        bail!("Wayland keyboard map is shorter than its advertised size");
    }
    let mut bytes = vec![0; usize::try_from(size)?];
    // Compositors can share this file description with other clients.
    file.read_exact_at(&mut bytes, 0)?;
    CStr::from_bytes_with_nul(&bytes)?;
    Ok(bytes)
}

impl Dispatch<wl_registry::WlRegistry, ()> for KeyboardState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" && !state.seat_bound {
                registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(SEAT_VERSION), handle, ());
                state.seat_bound = true;
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for KeyboardState {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(handle, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for KeyboardState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            state.keymap = Some(match format {
                WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) => read_keymap_file(fd, size),
                _ => Err(anyhow!(
                    "Wayland compositor did not provide an XKB keyboard map"
                )),
            });
        }
    }
}

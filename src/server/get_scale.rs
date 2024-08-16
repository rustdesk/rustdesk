use wayland_client::{protocol::{wl_output, wl_registry}, Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg;
use hbb_common::anyhow;

type ZxdgOutputManager = xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
type ZxdgOutputManagerEvent = xdg::xdg_output::zv1::client::zxdg_output_manager_v1::Event;
type ZxdgOutput = xdg::xdg_output::zv1::client::zxdg_output_v1::ZxdgOutputV1;
type ZxdgOutputEvent = xdg::xdg_output::zv1::client::zxdg_output_v1::Event;

// This struct represents the state of our app. This simple app does not
// need any state, by this type still supports the `Dispatch` implementations.
#[derive(Default)]
struct AppData {
    wl_output: Option<wl_output::WlOutput>,
    xdg_output_manager: Option<ZxdgOutputManager>,

    logical_width: i32,
    logical_height: i32,
    physical_width: i32,
    physical_height: i32,
}

// Implement `Dispatch<WlRegistry, ()> for out state. This provides the logic
// to be able to process events for the wl_registry interface.
//
// The second type parameter is the user-data of our implementation. It is a
// mechanism that allows you to associate a value to each particular Wayland
// object, and allow different dispatching logic depending on the type of the
// associated value.
//
// In this example, we just use () as we don't have any value to associate. See
// the `Dispatch` documentation for more details about this.
impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    fn event(
        data: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<AppData>,
    ) {
        // When receiving events from the wl_registry, we are only interested in the
        // `global` event, which signals a new available global.
        // When receiving this event, we just print its characteristics in this example.
        if let wl_registry::Event::Global { name, interface, version } = event {
            match &interface[..] {
                "wl_output" => {
                    let output =
                        registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());
                    data.wl_output = Some(output);
                },
                "zxdg_output_manager_v1" => {
                    let xdg_output_manager = registry.bind::<ZxdgOutputManager, _, _>(name, version, qh, ());
                    data.xdg_output_manager = Some(xdg_output_manager);
                },
                _ => {
                },
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for AppData {
    fn event(
        data: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        if let wl_output::Event::Mode { flags, width, height, refresh } = event {
            data.physical_width = width;
            data.physical_height = height;
        }
    }
}

impl Dispatch<ZxdgOutputManager, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManager,
        _: ZxdgOutputManagerEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

impl Dispatch<ZxdgOutput, ()> for AppData {
    fn event(
        data: &mut Self,
        _: &ZxdgOutput,
        event: ZxdgOutputEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        if let ZxdgOutputEvent::LogicalSize { width, height } = event {
            data.logical_width = width;
            data.logical_height = height;
        }
    }
}

// The main function of our program
pub fn get_scale() -> anyhow::Result<f64> {
    // Create a Wayland connection by connecting to the server through the
    // environment-provided configuration.
    let conn = Connection::connect_to_env()?;
    // Retrieve the WlDisplay Wayland object from the connection. This object is
    // the starting point of any Wayland program, from which all other objects will
    // be created.
    let display = conn.display();

    // Create an event queue for our event processing
    let mut event_queue = conn.new_event_queue();
    // And get its handle to associated new objects to it
    let qh = event_queue.handle();

    // Create a wl_registry object by sending the wl_display.get_registry request
    // This method takes two arguments: a handle to the queue the newly created
    // wl_registry will be assigned to, and the user-data that should be associated
    // with this registry (here it is () as we don't need user-data).
    let _registry = display.get_registry(&qh, ());

    let mut data = AppData::default();

    event_queue.blocking_dispatch(&mut data)?;

    if let (Some(wl_output), Some(xdg_output_manager)) = (&data.wl_output, &data.xdg_output_manager) {
        xdg_output_manager.get_xdg_output(&wl_output, &qh, ());
    }

    event_queue.blocking_dispatch(&mut data)?;

    if data.logical_width == 0 {
        return Err(anyhow::anyhow!("Can't divide zero logical width."));
    }
    Ok(data.physical_width as f64 / data.logical_width as f64)
}

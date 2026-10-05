use dbus::{
    arg::{RefArg, Variant},
    blocking::{stdintf::org_freedesktop_dbus::Properties, Connection},
    channel::Channel,
};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{env, fs, io::ErrorKind, path::PathBuf};

const IBUS_SERVICE: &str = "org.freedesktop.IBus";
const IBUS_PATH: &str = "/org/freedesktop/IBus";
// IBusEngineDesc serialization preserves these field positions for its clients.
const ENGINE_TYPE_FIELD: usize = 0;
const ENGINE_NAME_FIELD: usize = 2;
const ENGINE_LAYOUT_FIELD: usize = 9;
const ENGINE_VARIANT_FIELD: usize = 14;

pub(super) fn layout(id: &str) -> ResultType<Option<(String, String)>> {
    let connection = Connection::from(Channel::open_private(&address()?)?);
    let bus = connection.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        super::DBUS_TIMEOUT,
    );
    let _: (String,) = bus.method_call("org.freedesktop.DBus", "Hello", ())?;
    let proxy = connection.with_proxy(IBUS_SERVICE, IBUS_PATH, super::DBUS_TIMEOUT);
    let engine: Variant<Box<dyn RefArg>> = proxy.get(IBUS_SERVICE, "GlobalEngine")?;
    let fields: Vec<_> = engine
        .0
        .as_iter()
        .ok_or_else(|| anyhow!("Invalid IBus engine description"))?
        .collect();
    if field(&fields, ENGINE_TYPE_FIELD)? != "IBusEngineDesc" {
        bail!("IBus returned an unexpected engine description");
    }
    // GNOME may suppress IBus for password entry without updating persisted MRU.
    // Its xkb:us::eng echo engine does not identify the compositor's layout.
    if field(&fields, ENGINE_NAME_FIELD)? != id {
        return Err(
            super::UnreliableSource("MRU-selected IBus engine is not currently active").into(),
        );
    }
    let layout = field(&fields, ENGINE_LAYOUT_FIELD)?;
    if layout.is_empty() || layout == "default" {
        return Ok(None);
    }
    let variant = field(&fields, ENGINE_VARIANT_FIELD)?;
    let variant = if variant == "default" { "" } else { variant };
    Ok(Some((layout.to_owned(), variant.to_owned())))
}

fn field<'a>(fields: &[&'a dyn RefArg], index: usize) -> ResultType<&'a str> {
    fields
        .get(index)
        .and_then(|value| (*value).as_str())
        .ok_or_else(|| anyhow!("Invalid IBus engine field {}", index))
}

fn address() -> ResultType<String> {
    if let Some(address) = env::var_os("IBUS_ADDRESS") {
        return address
            .into_string()
            .map_err(|_| anyhow!("Invalid IBus address"));
    }
    let settings = fs::read_to_string(address_file()?)?;
    settings
        .lines()
        .find_map(|line| line.strip_prefix("IBUS_ADDRESS="))
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("IBus address file contains no address"))
}

fn address_file() -> ResultType<PathBuf> {
    if let Some(path) = env::var_os("IBUS_ADDRESS_FILE") {
        return Ok(path.into());
    }
    let machine = match fs::read_to_string("/var/lib/dbus/machine-id") {
        Err(error) if error.kind() == ErrorKind::NotFound => fs::read_to_string("/etc/machine-id")?,
        result => result?,
    };
    let display = env::var("WAYLAND_DISPLAY")?;
    Ok(gtk::glib::user_config_dir().join("ibus/bus").join(format!(
        "{}-unix-{}",
        machine.trim(),
        display
    )))
}

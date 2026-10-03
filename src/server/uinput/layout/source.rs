use dbus::blocking::Connection;
use gtk::{
    gio::{self, prelude::SettingsExt},
    glib::variant::FromVariant,
};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{ffi::CString, time::Duration};

mod ibus;

pub(super) const DBUS_TIMEOUT: Duration = Duration::from_millis(250);
const GNOME_INPUT_SCHEMA: &str = "org.gnome.desktop.input-sources";
const GNOME_DEFAULT_LAYOUT: &str = "us";

#[derive(PartialEq, Eq)]
pub(super) enum Source {
    Gnome(Names),
    Wayland { keymap: Vec<u8>, group: Option<u32> },
}

#[derive(PartialEq, Eq)]
pub(super) struct Names {
    pub model: CString,
    pub layout: CString,
    pub variant: CString,
    pub options: CString,
}

impl Source {
    pub fn read() -> ResultType<Self> {
        let connection = Connection::new_session()?;
        if name_has_owner(&connection, "org.gnome.Shell")? {
            return Ok(Self::Gnome(gnome_layout()?));
        }
        let keymap = super::wayland::read_keymap()?;
        let group = if name_has_owner(&connection, "org.kde.keyboard")? {
            Some(kde_layout_group(&connection)?)
        } else {
            None
        };
        Ok(Self::Wayland { keymap, group })
    }
}

fn name_has_owner(connection: &Connection, name: &str) -> ResultType<bool> {
    let proxy = connection.with_proxy(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        DBUS_TIMEOUT,
    );
    let (owned,): (bool,) = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", (name,))?;
    Ok(owned)
}

fn kde_layout_group(connection: &Connection) -> ResultType<u32> {
    let proxy = connection.with_proxy("org.kde.keyboard", "/Layouts", DBUS_TIMEOUT);
    let (group,): (u32,) = proxy.method_call("org.kde.KeyboardLayouts", "getLayout", ())?;
    Ok(group)
}

fn gnome_layout() -> ResultType<Names> {
    let schemas = gio::SettingsSchemaSource::default()
        .ok_or_else(|| anyhow!("GSettings schemas are unavailable"))?;
    let schema = schemas
        .lookup(GNOME_INPUT_SCHEMA, true)
        .ok_or_else(|| anyhow!("GNOME input source schema is unavailable"))?;
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    let sources: Vec<(String, String)> = required_setting(&settings, "sources")?;
    let recent: Vec<(String, String)> = required_setting(&settings, "mru-sources")?;
    // GNOME initially selects the first configured source when MRU is empty.
    // Its stored MRU does not track temporary or per-window layout switches.
    let selected = recent
        .iter()
        .find(|source| sources.contains(source))
        .or(sources.first());
    let (layout, variant) = gnome_source_layout(selected)?;
    if layout.is_empty() {
        bail!("GNOME input source has an empty layout");
    }
    let options: Vec<String> = required_setting(&settings, "xkb-options")?;
    // Older GNOME releases use the default model and have no xkb-model setting.
    let model: String = if schema.has_key("xkb-model") {
        required_setting(&settings, "xkb-model")?
    } else {
        String::new()
    };
    Ok(Names {
        model: CString::new(model)?,
        layout: CString::new(layout)?,
        variant: CString::new(variant)?,
        options: CString::new(options.join(","))?,
    })
}

fn gnome_source_layout(selected: Option<&(String, String)>) -> ResultType<(String, String)> {
    let Some((kind, id)) = selected else {
        // GNOME Shell selects this layout when no input source is configured.
        return Ok((GNOME_DEFAULT_LAYOUT.to_owned(), String::new()));
    };
    match kind.as_str() {
        "xkb" => {
            let (layout, variant) = id.split_once('+').unwrap_or((id.as_str(), ""));
            Ok((layout.to_owned(), variant.to_owned()))
        }
        "ibus" => ibus::layout(id),
        _ => bail!("Unsupported GNOME input source type: {}", kind),
    }
}

fn required_setting<T: FromVariant>(settings: &gio::Settings, key: &str) -> ResultType<T> {
    if !settings
        .settings_schema()
        .is_some_and(|schema| schema.has_key(key))
    {
        bail!("Missing GNOME setting {}", key);
    }
    settings
        .value(key)
        .get()
        .ok_or_else(|| anyhow!("Invalid GNOME setting {}", key))
}

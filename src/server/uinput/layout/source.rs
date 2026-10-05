use dbus::blocking::Connection;
use gtk::{
    gio::{self, prelude::SettingsExt},
    glib::variant::FromVariant,
};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{error::Error, ffi::CString, fmt, time::Duration};

mod ibus;

pub(super) const DBUS_TIMEOUT: Duration = Duration::from_millis(250);
const GNOME_INPUT_SCHEMA: &str = "org.gnome.desktop.input-sources";
const GNOME_DEFAULT_LAYOUT: &str = "us";

#[derive(Debug)]
pub(super) struct UnreliableSource(pub &'static str);

impl fmt::Display for UnreliableSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for UnreliableSource {}

#[derive(PartialEq, Eq)]
pub(super) enum Source {
    Gnome(Names),
    Wayland { keymap: Vec<u8>, group: Option<u32> },
    Xwayland { keymap: Vec<u8>, group: Option<u32> },
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
            if let Some(names) = gnome_layout()? {
                return Ok(Self::Gnome(names));
            }
            return Ok(Self::Wayland {
                keymap: super::wayland::read_keymap()?,
                group: None,
            });
        }
        let kde_layouts = name_has_owner(&connection, "org.kde.keyboard")?;
        // KWin 5 does not publish the layout service for a single-layout map.
        if kde_layouts || name_has_owner(&connection, "org.kde.KWin")? {
            return Ok(Self::Wayland {
                keymap: super::wayland::read_keymap()?,
                group: if kde_layouts {
                    Some(kde_layout_group(&connection)?)
                } else {
                    None
                },
            });
        }
        // Other compositors need the effective keymap and active group of our
        // uinput device, including per-device options and custom keymap overrides.
        // wl_keyboard exposes a seat map that may belong to a physical keyboard.
        // Equal layout names/group indices do not prove map identity: Caps can
        // act as AltGr in one map and toggle CapsLock in the other.
        //
        // Sway GET_INPUTS/GET_SEATS expose names and an index, not the device map.
        // GET_CONFIG returns saved config text, not effective runtime input
        // settings; parsing it cannot reliably account for runtime overrides.
        // Hyprland's devices reply reports RMLVO (rules/model/layout/variant/options),
        // but kb_file can override it. Those fields alone cannot reconstruct the
        // loaded map. This does not rule out additional compositor-specific APIs.
        //
        // Reliable native discovery needs a compositor query associating the
        // device's actual map (or verified effective configuration) with its group.
        // uinput supplies keycodes; it cannot report the compositor's XKB policy.
        // Xwayland remains best effort; see xwayland::read for its freshness limits.
        // zwp_virtual_keyboard_manager_v1 needs a separate injection backend and
        // is not universally supported; it does not query an existing uinput map.
        super::xwayland::read()
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

fn gnome_layout() -> ResultType<Option<Names>> {
    let schemas = gio::SettingsSchemaSource::default()
        .ok_or_else(|| anyhow!("GSettings schemas are unavailable"))?;
    let schema = schemas
        .lookup(GNOME_INPUT_SCHEMA, true)
        .ok_or_else(|| anyhow!("GNOME input source schema is unavailable"))?;
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    let sources: Vec<(String, String)> = required_setting(&settings, "sources")?;
    // Focus changes restore a window's source without persisting MRU. Mark it
    // unreliable so refresh also discards a previously accepted MRU mapping.
    if sources.len() > 1 && required_setting::<bool>(&settings, "per-window")? {
        return Err(UnreliableSource(
            "GNOME per-window input sources cannot be determined from persisted MRU",
        )
        .into());
    }
    let recent: Vec<(String, String)> = required_setting(&settings, "mru-sources")?;
    // GNOME initially selects the first configured source when MRU is empty.
    // Its stored MRU still does not track temporary IBus suppression.
    let selected = recent
        .iter()
        .find(|source| sources.contains(source))
        .or(sources.first());
    let Some((layout, variant)) = gnome_source_layout(selected)? else {
        // IBus engines without an XKB override use the compositor's keymap.
        return Ok(None);
    };
    if layout.is_empty() {
        bail!("GNOME input source has an empty layout");
    }
    // Match GNOME Shell's session-wide options; it does not merge IBus
    // engine layout-option metadata into the compositor keymap.
    let options: Vec<String> = required_setting(&settings, "xkb-options")?;
    // Older GNOME releases use the default model and have no xkb-model setting.
    let model: String = if schema.has_key("xkb-model") {
        required_setting(&settings, "xkb-model")?
    } else {
        String::new()
    };
    Ok(Some(Names {
        model: CString::new(model)?,
        layout: CString::new(layout)?,
        variant: CString::new(variant)?,
        options: CString::new(options.join(","))?,
    }))
}

fn gnome_source_layout(
    selected: Option<&(String, String)>,
) -> ResultType<Option<(String, String)>> {
    let Some((kind, id)) = selected else {
        // GNOME Shell selects this layout when no input source is configured.
        return Ok(Some((GNOME_DEFAULT_LAYOUT.to_owned(), String::new())));
    };
    match kind.as_str() {
        "xkb" => {
            let (layout, variant) = id.split_once('+').unwrap_or((id.as_str(), ""));
            Ok(Some((layout.to_owned(), variant.to_owned())))
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

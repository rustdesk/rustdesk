use super::{
    super::{wayland, XkbKeymap, DEFAULT_GROUP},
    Source,
};
use hbb_common::{anyhow::anyhow, bail, libc, libloading::Library, ResultType};
use serde_json::Value;
use std::ffi::CStr;

mod ipc;

const UINPUT_NAME: &str = "RustDesk UInput Keyboard";
const HYPRLAND_UINPUT_NAME: &str = "rustdesk-uinput-keyboard";

pub(super) fn read() -> ResultType<Option<Source>> {
    let Some((kind, reply)) = ipc::read()? else {
        return Ok(None);
    };
    let selected = selection(kind, &reply)?;
    let mut source = Source::Wayland {
        keymap: wayland::read_keymap()?,
        group: Some(DEFAULT_GROUP),
    };
    let names = layout_names(&source)?;
    let active = selected.group(&names)?;
    if let Source::Wayland { group, .. } = &mut source {
        *group = Some(active);
    }
    Ok(Some(source))
}

struct Selection<'a> {
    name: &'a str,
    index: Option<u32>,
}

impl Selection<'_> {
    fn group(&self, names: &[String]) -> ResultType<u32> {
        if let Some(index) = self.index {
            if names.get(index as usize).map(String::as_str) != Some(self.name) {
                bail!("The uinput keyboard layout differs from the Wayland seat keymap");
            }
            return Ok(index);
        }
        // Older Hyprland replies expose the active name, without its index.
        let mut matching = names
            .iter()
            .enumerate()
            .filter(|(_, name)| *name == self.name);
        let (index, _) = matching
            .next()
            .ok_or_else(|| anyhow!("The active uinput layout is absent from the Wayland keymap"))?;
        if matching.next().is_some() {
            bail!("The active uinput layout name is ambiguous in the Wayland keymap");
        }
        Ok(u32::try_from(index)?)
    }
}

fn selection(kind: ipc::Kind, reply: &Value) -> ResultType<Selection<'_>> {
    let (keyboards, device_name, name_field, index_field) = match kind {
        ipc::Kind::Sway => (
            reply,
            UINPUT_NAME,
            "xkb_active_layout_name",
            "xkb_active_layout_index",
        ),
        ipc::Kind::Hyprland => (
            &reply["keyboards"],
            HYPRLAND_UINPUT_NAME,
            "active_keymap",
            "active_layout_index",
        ),
    };
    // Physical keyboards can have different layouts; only our device injects these keys.
    let mut devices = keyboards
        .as_array()
        .ok_or_else(|| anyhow!("Invalid {:?} keyboard list", kind))?
        .iter()
        .filter(|device| {
            device["name"].as_str().is_some_and(|name| {
                name == device_name
                    || (matches!(kind, ipc::Kind::Hyprland) && has_hyprland_uinput_suffix(name))
            })
        });
    let device = devices
        .next()
        .ok_or_else(|| anyhow!("{:?} did not report the RustDesk uinput keyboard", kind))?;
    if devices.next().is_some() {
        bail!("{:?} reports multiple RustDesk uinput keyboards", kind);
    }
    let name = device[name_field]
        .as_str()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("{:?} did not report an active uinput layout", kind))?;
    let index = device
        .get(index_field)
        .map(|index| {
            let index = index
                .as_u64()
                .ok_or_else(|| anyhow!("Invalid {:?} active layout index", kind))?;
            Ok::<_, hbb_common::anyhow::Error>(u32::try_from(index)?)
        })
        .transpose()?;
    Ok(Selection { name, index })
}

fn has_hyprland_uinput_suffix(name: &str) -> bool {
    // Hyprland keeps the suffix after an older same-name keyboard disappears.
    name.strip_prefix(HYPRLAND_UINPUT_NAME)
        .and_then(|suffix| suffix.strip_prefix('-'))
        .is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn layout_names(source: &Source) -> ResultType<Vec<String>> {
    let map = XkbKeymap::open(source)?;
    // xkbcommon-dl 0.4 does not expose this core libxkbcommon function.
    let library = unsafe { Library::new("libxkbcommon.so.0")? };
    let get_name = unsafe {
        library
            .get::<unsafe extern "C" fn(*mut xkbcommon_dl::xkb_keymap, u32) -> *const libc::c_char>(
                b"xkb_keymap_layout_get_name\0",
            )?
    };
    let count = unsafe { (map.api.xkb_keymap_num_layouts)(map.map.as_ptr()) };
    (0..count)
        .map(|index| {
            let name = unsafe { get_name(map.map.as_ptr(), index) };
            if name.is_null() {
                bail!("Wayland keymap has an unnamed layout");
            }
            Ok(unsafe { CStr::from_ptr(name) }.to_str()?.to_owned())
        })
        .collect()
}

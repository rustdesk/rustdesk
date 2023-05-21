use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

// virtual display index range: 0 - 2 are reserved for headless and other special uses.
const VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS: u32 = 0;
const VIRTUAL_DISPLAY_START_FOR_PEER: u32 = 3;
const VIRTUAL_DISPLAY_MAX_COUNT: u32 = 10;

lazy_static::lazy_static! {
    static ref VIRTUAL_DISPLAY_MANAGER: Arc<Mutex<VirtualDisplayManager>> =
        Arc::new(Mutex::new(VirtualDisplayManager::default()));
}

#[derive(Default)]
struct VirtualDisplayManager {
    headless_index_name: Option<(u32, String)>,
    peer_index_name: HashMap<u32, String>,
}

impl VirtualDisplayManager {
    fn prepare_driver() -> ResultType<()> {
        if let Err(e) = virtual_display::create_device() {
            if !e.to_string().contains("Device is already created") {
                bail!("Create device failed {}", e);
            }
        }
        // Reboot is not required for this case.
        let mut _reboot_required = false;
        allow_err!(virtual_display::install_update_driver(
            &mut _reboot_required
        ));
        Ok(())
    }

    fn plug_in_monitor(index: u32, modes: &[virtual_display::MonitorMode]) -> ResultType<()> {
        if let Err(e) = virtual_display::plug_in_monitor(index) {
            bail!("Plug in monitor failed {}", e);
        }
        if let Err(e) = virtual_display::update_monitor_modes(index, &modes) {
            log::error!("Update monitor modes failed {}", e);
        }
        Ok(())
    }
}

pub fn plug_in_headless() -> ResultType<()> {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    VirtualDisplayManager::prepare_driver()?;
    let modes = [virtual_display::MonitorMode {
        width: 1920,
        height: 1080,
        sync: 60,
    }];
    let device_names = windows::get_device_names();
    VirtualDisplayManager::plug_in_monitor(VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS, &modes)?;
    let device_name = get_new_device_name(&device_names);
    manager.headless_index_name = Some((VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS, device_name));
    Ok(())
}

pub fn plug_out_headless() -> bool {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    if let Some((index, _)) = manager.headless_index_name.take() {
        if let Err(e) = virtual_display::plug_out_monitor(index) {
            log::error!("Plug out monitor failed {}", e);
        }
        true
    } else {
        false
    }
}

fn get_new_device_name(device_names: &HashSet<String>) -> String {
    for _ in 0..3 {
        let device_names_af = windows::get_device_names();
        let diff_names: Vec<_> = device_names_af.difference(&device_names).collect();
        if diff_names.len() == 1 {
            return diff_names[0].clone();
        } else if diff_names.len() > 1 {
            log::error!(
                "Failed to get diff device names after plugin virtual display, more than one diff names: {:?}",
                &diff_names
            );
            return "".to_string();
        }
        // Sleep is needed here to wait for the virtual display to be ready.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    log::error!("Failed to get diff device names after plugin virtual display",);
    "".to_string()
}

pub fn plug_in_peer_request(modes: Vec<Vec<virtual_display::MonitorMode>>) -> ResultType<Vec<u32>> {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    VirtualDisplayManager::prepare_driver()?;

    let mut indices: Vec<u32> = Vec::new();
    for m in modes.iter() {
        for idx in VIRTUAL_DISPLAY_START_FOR_PEER..VIRTUAL_DISPLAY_MAX_COUNT {
            if !manager.peer_index_name.contains_key(&idx) {
                let device_names = windows::get_device_names();
                match VirtualDisplayManager::plug_in_monitor(idx, m) {
                    Ok(_) => {
                        let device_name = get_new_device_name(&device_names);
                        manager.peer_index_name.insert(idx, device_name);
                        indices.push(idx);
                    }
                    Err(e) => {
                        log::error!("Plug in monitor failed {}", e);
                    }
                }
            }
        }
    }

    Ok(indices)
}

pub fn plug_out_peer_request(modes: &[u32]) -> ResultType<()> {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    for idx in modes.iter() {
        if manager.peer_index_name.contains_key(idx) {
            allow_err!(virtual_display::plug_out_monitor(*idx));
            manager.peer_index_name.remove(idx);
        }
    }
    Ok(())
}

pub fn is_virtual_display(name: &str) -> bool {
    let lock = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    if let Some((_, device_name)) = &lock.headless_index_name {
        if windows::is_device_name(device_name, name) {
            return true;
        }
    }
    for (_, v) in lock.peer_index_name.iter() {
        if windows::is_device_name(v, name) {
            return true;
        }
    }
    false
}

fn change_resolution(index: u32, w: u32, h: u32) -> bool {
    let modes = [virtual_display::MonitorMode {
        width: w,
        height: h,
        sync: 60,
    }];
    match virtual_display::update_monitor_modes(index, &modes) {
        Ok(_) => true,
        Err(e) => {
            log::error!("Update monitor {} modes {:?} failed: {}", index, &modes, e);
            false
        }
    }
}

pub fn change_resolution_if_is_virtual_display(name: &str, w: u32, h: u32) -> Option<bool> {
    let lock = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    if let Some((index, device_name)) = &lock.headless_index_name {
        if windows::is_device_name(device_name, name) {
            return Some(change_resolution(*index, w, h));
        }
    }

    for (k, v) in lock.peer_index_name.iter() {
        if windows::is_device_name(v, name) {
            return Some(change_resolution(*k, w, h));
        }
    }
    None
}

mod windows {
    use std::{collections::HashSet, ptr::null_mut};
    use winapi::{
        shared::minwindef::{DWORD, FALSE},
        um::{
            wingdi::{
                DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_MIRRORING_DRIVER,
            },
            winuser::{EnumDisplayDevicesW, EnumDisplaySettingsExW, ENUM_CURRENT_SETTINGS},
        },
    };

    // This string is defined here.
    //  https://github.com/fufesou/RustDeskIddDriver/blob/b370aad3f50028b039aad211df60c8051c4a64d6/RustDeskIddDriver/RustDeskIddDriver.inf#LL73C1-L73C40
    const IDD_DEVICE_STRING: &'static str = "RustDeskIddDriver Device\0";

    #[inline]
    pub(super) fn is_device_name(device_name: &str, name: &str) -> bool {
        if name.len() == device_name.len() {
            name == device_name
        } else if name.len() > device_name.len() {
            false
        } else {
            &device_name[..name.len()] == name && device_name.as_bytes()[name.len() as usize] == 0
        }
    }

    pub(super) fn get_device_names() -> HashSet<String> {
        let mut device_names = HashSet::new();
        let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as DWORD;
        let mut i_dev_num = 0;
        loop {
            let result = unsafe { EnumDisplayDevicesW(null_mut(), i_dev_num, &mut dd, 0) };
            if result == 0 {
                break;
            }
            i_dev_num += 1;

            if 0 == (dd.StateFlags & DISPLAY_DEVICE_ACTIVE)
                || (dd.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER) > 0
            {
                continue;
            }

            let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
            dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
            dm.dmDriverExtra = 0;
            let ok = unsafe {
                EnumDisplaySettingsExW(
                    dd.DeviceName.as_ptr(),
                    ENUM_CURRENT_SETTINGS,
                    &mut dm as _,
                    0,
                )
            };
            if ok == FALSE {
                continue;
            }
            if dm.dmPelsHeight == 0 || dm.dmPelsWidth == 0 {
                continue;
            }

            if let (Ok(device_name), Ok(device_string)) = (
                String::from_utf16(&dd.DeviceName),
                String::from_utf16(&dd.DeviceString),
            ) {
                if &device_string[..IDD_DEVICE_STRING.len()] == IDD_DEVICE_STRING {
                    device_names.insert(device_name);
                }
            }
        }
        device_names
    }
}

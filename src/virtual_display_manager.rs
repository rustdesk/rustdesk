use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
use std::{
    collections::HashSet,
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
    headless_index: Option<u32>,
    peer_required_indices: HashSet<u32>,
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
    VirtualDisplayManager::plug_in_monitor(VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS, &modes)?;
    manager.headless_index = Some(VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS);
    Ok(())
}

pub fn plug_out_headless() -> bool {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    if let Some(index) = manager.headless_index.take() {
        if let Err(e) = virtual_display::plug_out_monitor(index) {
            log::error!("Plug out monitor failed {}", e);
        }
        true
    } else {
        false
    }
}

pub fn plug_in_peer_required(
    modes: Vec<Vec<virtual_display::MonitorMode>>,
) -> ResultType<Vec<u32>> {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    VirtualDisplayManager::prepare_driver()?;

    let mut indices: Vec<u32> = Vec::new();
    for m in modes.iter() {
        for idx in VIRTUAL_DISPLAY_START_FOR_PEER..VIRTUAL_DISPLAY_MAX_COUNT {
            if !manager.peer_required_indices.contains(&idx) {
                match VirtualDisplayManager::plug_in_monitor(idx, m) {
                    Ok(_) => {
                        manager.peer_required_indices.insert(idx);
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

pub fn plug_out_peer_required(modes: &[u32]) -> ResultType<()> {
    let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
    for idx in modes.iter() {
        if manager.peer_required_indices.contains(idx) {
            allow_err!(virtual_display::plug_out_monitor(*idx));
            manager.peer_required_indices.remove(idx);
        }
    }
    Ok(())
}

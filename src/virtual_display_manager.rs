use hbb_common::{bail, platform::windows::is_windows_version_or_greater, ResultType};

// This string is defined here.
//  https://github.com/rustdesk-org/RustDeskIddDriver/blob/b370aad3f50028b039aad211df60c8051c4a64d6/RustDeskIddDriver/RustDeskIddDriver.inf#LL73C1-L73C40
pub const RUSTDESK_IDD_DEVICE_STRING: &'static str = "RustDeskIddDriver Device\0";
pub const AMYUNI_IDD_DEVICE_STRING: &'static str = "USB Mobile Monitor Virtual Display\0";

const IDD_IMPL: &str = IDD_IMPL_AMYUNI;
const IDD_IMPL_RUSTDESK: &str = "rustdesk_idd";
const IDD_IMPL_AMYUNI: &str = "amyuni_idd";
const IDD_PLUG_OUT_ALL_INDEX: i32 = -1;

pub fn is_amyuni_idd() -> bool {
    IDD_IMPL == IDD_IMPL_AMYUNI
}

pub fn get_cur_device_string() -> &'static str {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => RUSTDESK_IDD_DEVICE_STRING,
        IDD_IMPL_AMYUNI => AMYUNI_IDD_DEVICE_STRING,
        _ => "",
    }
}

pub fn is_virtual_display_supported() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_windows_version_or_greater(10, 0, 19041, 0, 0)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub fn plug_in_headless() -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_headless(),
        IDD_IMPL_AMYUNI => amyuni_idd::plug_in_headless(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn get_platform_additions() -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    if !crate::platform::windows::is_self_service_running() {
        return map;
    }
    map.insert("idd_impl".into(), serde_json::json!(IDD_IMPL));
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => {
            let virtual_displays = rustdesk_idd::get_virtual_displays();
            if !virtual_displays.is_empty() {
                map.insert(
                    "rustdesk_virtual_displays".into(),
                    serde_json::json!(virtual_displays),
                );
            }
        }
        IDD_IMPL_AMYUNI => {
            let c = amyuni_idd::get_monitor_count();
            if c > 0 {
                map.insert("amyuni_virtual_displays".into(), serde_json::json!(c));
            }
        }
        _ => {}
    }
    map
}

#[inline]
pub fn plug_in_monitor(idx: u32, modes: Vec<virtual_display::MonitorMode>) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_index_modes(idx, modes),
        IDD_IMPL_AMYUNI => amyuni_idd::plug_in_monitor(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_out_monitor(index: i32, force_all: bool, force_one: bool) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => {
            let indices = if index == IDD_PLUG_OUT_ALL_INDEX {
                rustdesk_idd::get_virtual_displays()
            } else {
                vec![index as _]
            };
            rustdesk_idd::plug_out_peer_request(&indices)
        }
        IDD_IMPL_AMYUNI => amyuni_idd::plug_out_monitor(index, force_all, force_one),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_in_peer_request(modes: Vec<Vec<virtual_display::MonitorMode>>) -> ResultType<Vec<u32>> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_peer_request(modes),
        IDD_IMPL_AMYUNI => {
            amyuni_idd::plug_in_monitor()?;
            Ok(vec![0])
        }
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_out_monitor_indices(
    indices: &[u32],
    force_all: bool,
    force_one: bool,
) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_out_peer_request(indices),
        IDD_IMPL_AMYUNI => {
            for _idx in indices.iter() {
                amyuni_idd::plug_out_monitor(0, force_all, force_one)?;
            }
            Ok(())
        }
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn reset_all() -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::reset_all(),
        IDD_IMPL_AMYUNI => amyuni_idd::reset_all(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub mod rustdesk_idd {
    use super::windows;
    use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
    use std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    };

    // virtual display index range: 0 - 2 are reserved for headless and other special uses.
    const VIRTUAL_DISPLAY_INDEX_FOR_HEADLESS: u32 = 0;
    const VIRTUAL_DISPLAY_START_FOR_PEER: u32 = 1;
    const VIRTUAL_DISPLAY_MAX_COUNT: u32 = 5;

    lazy_static::lazy_static! {
        static ref VIRTUAL_DISPLAY_MANAGER: Arc<Mutex<VirtualDisplayManager>> =
            Arc::new(Mutex::new(VirtualDisplayManager::default()));
    }

    #[derive(Default)]
    struct VirtualDisplayManager {
        headless_index_name: Option<(u32, String)>,
        peer_index_name: HashMap<u32, String>,
        is_driver_installed: bool,
    }

    impl VirtualDisplayManager {
        fn prepare_driver(&mut self) -> ResultType<()> {
            if !self.is_driver_installed {
                self.install_update_driver()?;
            }
            Ok(())
        }

        fn install_update_driver(&mut self) -> ResultType<()> {
            if let Err(e) = virtual_display::create_device() {
                if !e.to_string().contains("Device is already created") {
                    bail!("Create device failed {}", e);
                }
            }
            // Reboot is not required for this case.
            let mut _reboot_required = false;
            virtual_display::install_update_driver(&mut _reboot_required)?;
            self.is_driver_installed = true;
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

    pub fn install_update_driver() -> ResultType<()> {
        VIRTUAL_DISPLAY_MANAGER
            .lock()
            .unwrap()
            .install_update_driver()
    }

    #[inline]
    fn get_device_names() -> Vec<String> {
        windows::get_device_names(Some(super::RUSTDESK_IDD_DEVICE_STRING))
    }

    pub fn plug_in_headless() -> ResultType<()> {
        let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
        manager.prepare_driver()?;
        let modes = [virtual_display::MonitorMode {
            width: 1920,
            height: 1080,
            sync: 60,
        }];
        let device_names = get_device_names().into_iter().collect();
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
            let device_names_af: HashSet<String> = get_device_names().into_iter().collect();
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

    pub fn get_virtual_displays() -> Vec<u32> {
        VIRTUAL_DISPLAY_MANAGER
            .lock()
            .unwrap()
            .peer_index_name
            .keys()
            .cloned()
            .collect()
    }

    pub fn plug_in_index_modes(
        idx: u32,
        mut modes: Vec<virtual_display::MonitorMode>,
    ) -> ResultType<()> {
        let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
        manager.prepare_driver()?;
        if !manager.peer_index_name.contains_key(&idx) {
            let device_names = get_device_names().into_iter().collect();
            if modes.is_empty() {
                modes.push(virtual_display::MonitorMode {
                    width: 1920,
                    height: 1080,
                    sync: 60,
                });
            }
            match VirtualDisplayManager::plug_in_monitor(idx, modes.as_slice()) {
                Ok(_) => {
                    let device_name = get_new_device_name(&device_names);
                    manager.peer_index_name.insert(idx, device_name);
                }
                Err(e) => {
                    log::error!("Plug in monitor failed {}", e);
                }
            }
        }
        Ok(())
    }

    pub fn reset_all() -> ResultType<()> {
        if super::is_virtual_display_supported() {
            return Ok(());
        }

        if let Err(e) = plug_out_peer_request(&get_virtual_displays()) {
            log::error!("Failed to plug out virtual displays: {}", e);
        }
        let _ = plug_out_headless();
        Ok(())
    }

    pub fn plug_in_peer_request(
        modes: Vec<Vec<virtual_display::MonitorMode>>,
    ) -> ResultType<Vec<u32>> {
        let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
        manager.prepare_driver()?;

        let mut indices: Vec<u32> = Vec::new();
        for m in modes.iter() {
            for idx in VIRTUAL_DISPLAY_START_FOR_PEER..VIRTUAL_DISPLAY_MAX_COUNT {
                if !manager.peer_index_name.contains_key(&idx) {
                    let device_names = get_device_names().into_iter().collect();
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
                    break;
                }
            }
        }

        Ok(indices)
    }

    pub fn plug_out_peer_request(indices: &[u32]) -> ResultType<()> {
        let mut manager = VIRTUAL_DISPLAY_MANAGER.lock().unwrap();
        for idx in indices.iter() {
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
}

pub mod amyuni_idd {
    use super::windows;
    use crate::platform::{reg_display_settings, win_device};
    use hbb_common::{bail, lazy_static, log, tokio::time::Instant, ResultType};
    use std::{
        ptr::null_mut,
        sync::{atomic, Arc, Mutex},
        time::Duration,
    };
    use winapi::{
        shared::{guiddef::GUID, winerror::ERROR_NO_MORE_ITEMS},
        um::shellapi::ShellExecuteA,
    };

    const INF_PATH: &str = r#"usbmmidd_v2\usbmmIdd.inf"#;
    const INTERFACE_GUID: GUID = GUID {
        Data1: 0xb5ffd75f,
        Data2: 0xda40,
        Data3: 0x4353,
        Data4: [0x8f, 0xf8, 0xb6, 0xda, 0xf6, 0xf1, 0xd8, 0xca],
    };
    const HARDWARE_ID: &str = "usbmmidd";
    const PLUG_MONITOR_IO_CONTROL_CDOE: u32 = 2307084;
    const INSTALLER_EXE_FILE: &str = "deviceinstaller64.exe";

    lazy_static::lazy_static! {
        static ref LOCK: Arc<Mutex<()>> = Default::default();
        static ref LAST_PLUG_IN_HEADLESS_TIME: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    }
    const VIRTUAL_DISPLAY_MAX_COUNT: usize = 4;
    // The count of virtual displays plugged in.
    // This count is not accurate, because:
    // 1. The virtual display driver may also be controlled by other processes.
    // 2. RustDesk may crash and restart, but the virtual displays are kept.
    //
    // to-do: Maybe a better way is to add an option asking the user if plug out all virtual displays on disconnect.
    static VIRTUAL_DISPLAY_COUNT: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

    fn get_deviceinstaller64_work_dir() -> ResultType<Option<Vec<u8>>> {
        let cur_exe = std::env::current_exe()?;
        let Some(cur_dir) = cur_exe.parent() else {
            bail!("Cannot get parent of current exe file.");
        };
        let work_dir = cur_dir.join("usbmmidd_v2");
        if !work_dir.exists() {
            return Ok(None);
        }
        let exe_path = work_dir.join(INSTALLER_EXE_FILE);
        if !exe_path.exists() {
            return Ok(None);
        }

        let Some(work_dir) = work_dir.to_str() else {
            bail!("Cannot convert work_dir to string.");
        };
        let mut work_dir2 = work_dir.as_bytes().to_vec();
        work_dir2.push(0);
        Ok(Some(work_dir2))
    }

    pub fn uninstall_driver() -> ResultType<()> {
        if let Ok(Some(work_dir)) = get_deviceinstaller64_work_dir() {
            if crate::platform::windows::is_x64() {
                log::info!("Uninstalling driver by deviceinstaller64.exe");
                install_if_x86_on_x64(&work_dir, "remove usbmmidd")?;
                return Ok(());
            }
        }

        log::info!("Uninstalling driver by SetupAPI");
        let mut reboot_required = false;
        let _ = unsafe { win_device::uninstall_driver(HARDWARE_ID, &mut reboot_required)? };
        Ok(())
    }

    // SetupDiCallClassInstaller() will always fail if current_exe() is built as x86 and running on x64.
    // So we need to call another x64 version exe to install and uninstall the driver.
    fn install_if_x86_on_x64(work_dir: &[u8], args: &str) -> ResultType<()> {
        const SW_HIDE: i32 = 0;
        let mut args = args.bytes().collect::<Vec<_>>();
        args.push(0);
        let mut exe_file = INSTALLER_EXE_FILE.bytes().collect::<Vec<_>>();
        exe_file.push(0);
        let hi = unsafe {
            ShellExecuteA(
                null_mut(),
                "open\0".as_ptr() as _,
                exe_file.as_ptr() as _,
                args.as_ptr() as _,
                work_dir.as_ptr() as _,
                SW_HIDE,
            ) as i32
        };
        if hi <= 32 {
            log::error!("Failed to run deviceinstaller: {}", hi);
            bail!("Failed to run deviceinstaller.")
        }
        Ok(())
    }

    // If the driver is installed by "deviceinstaller64.exe", the driver will be installed asynchronously.
    // The caller must wait some time before using the driver.
    fn check_install_driver(is_async: &mut bool) -> ResultType<()> {
        let _l = LOCK.lock().unwrap();
        let drivers = windows::get_display_drivers();
        if drivers
            .iter()
            .any(|(s, c)| s == super::AMYUNI_IDD_DEVICE_STRING && *c == 0)
        {
            *is_async = false;
            return Ok(());
        }

        if let Ok(Some(work_dir)) = get_deviceinstaller64_work_dir() {
            if crate::platform::windows::is_x64() {
                log::info!("Installing driver by deviceinstaller64.exe");
                install_if_x86_on_x64(&work_dir, "install usbmmidd.inf usbmmidd")?;
                *is_async = true;
                return Ok(());
            }
        }

        let exe_file = std::env::current_exe()?;
        let Some(cur_dir) = exe_file.parent() else {
            bail!("Cannot get parent of current exe file");
        };
        let inf_path = cur_dir.join(INF_PATH);
        if !inf_path.exists() {
            bail!("Driver inf file not found.");
        }
        let inf_path = inf_path.to_string_lossy().to_string();

        log::info!("Installing driver by SetupAPI");
        let mut reboot_required = false;
        let _ =
            unsafe { win_device::install_driver(&inf_path, HARDWARE_ID, &mut reboot_required)? };
        *is_async = false;
        Ok(())
    }

    pub fn reset_all() -> ResultType<()> {
        let _ = crate::privacy_mode::turn_off_privacy(0, None);
        let _ = plug_out_monitor(super::IDD_PLUG_OUT_ALL_INDEX, true, false);
        *LAST_PLUG_IN_HEADLESS_TIME.lock().unwrap() = None;
        Ok(())
    }

    #[inline]
    fn plug_monitor_(
        add: bool,
        wait_timeout: Option<Duration>,
    ) -> Result<(), win_device::DeviceError> {
        let cmd = if add { 0x10 } else { 0x00 };
        let cmd = [cmd, 0x00, 0x00, 0x00];
        let now = Instant::now();
        let c1 = get_monitor_count();
        unsafe {
            win_device::device_io_control(&INTERFACE_GUID, PLUG_MONITOR_IO_CONTROL_CDOE, &cmd, 0)?;
        }
        if let Some(wait_timeout) = wait_timeout {
            while now.elapsed() < wait_timeout {
                if get_monitor_count() != c1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        }
        // No need to consider concurrency here.
        if add {
            // If the monitor is plugged in, increase the count.
            // Though there's already a check of `VIRTUAL_DISPLAY_MAX_COUNT`, it's still better to check here for double ensure.
            if VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::SeqCst) < VIRTUAL_DISPLAY_MAX_COUNT {
                VIRTUAL_DISPLAY_COUNT.fetch_add(1, atomic::Ordering::SeqCst);
            }
        } else {
            if VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::SeqCst) > 0 {
                VIRTUAL_DISPLAY_COUNT.fetch_sub(1, atomic::Ordering::SeqCst);
            }
        }
        Ok(())
    }

    // `std::thread::sleep()` with a timeout is acceptable here.
    // Because user can wait for a while to plug in a monitor.
    fn plug_in_monitor_(
        add: bool,
        is_driver_async_installed: bool,
        wait_timeout: Option<Duration>,
    ) -> ResultType<()> {
        let timeout = Duration::from_secs(3);
        let now = Instant::now();
        let reg_connectivity_old = reg_display_settings::read_reg_connectivity();
        loop {
            match plug_monitor_(add, wait_timeout) {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    if is_driver_async_installed {
                        if let win_device::DeviceError::WinApiLastErr(_, e2) = &e {
                            if e2.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as _) {
                                if now.elapsed() < timeout {
                                    std::thread::sleep(Duration::from_millis(100));
                                    continue;
                                }
                            }
                        }
                    }
                    return Err(e.into());
                }
            }
        }
        // Workaround for the issue that we can't set the default the resolution.
        if let Ok(old_connectivity_old) = reg_connectivity_old {
            std::thread::spawn(move || {
                try_reset_resolution_on_first_plug_in(old_connectivity_old.len(), 1920, 1080);
            });
        }

        Ok(())
    }

    fn try_reset_resolution_on_first_plug_in(
        old_connectivity_len: usize,
        width: usize,
        height: usize,
    ) {
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(300));
            if let Ok(reg_connectivity_new) = reg_display_settings::read_reg_connectivity() {
                if reg_connectivity_new.len() != old_connectivity_len {
                    for name in
                        windows::get_device_names(Some(super::AMYUNI_IDD_DEVICE_STRING)).iter()
                    {
                        crate::platform::change_resolution(&name, width, height).ok();
                    }
                    break;
                }
            }
        }
    }

    pub fn plug_in_headless() -> ResultType<()> {
        let mut tm = LAST_PLUG_IN_HEADLESS_TIME.lock().unwrap();
        if let Some(tm) = &mut *tm {
            if tm.elapsed() < Duration::from_secs(3) {
                bail!("Plugging in too frequently.");
            }
        }
        *tm = Some(Instant::now());
        drop(tm);

        let mut is_async = false;
        if let Err(e) = check_install_driver(&mut is_async) {
            log::error!("Failed to install driver: {}", e);
            bail!("Failed to install driver.");
        }

        plug_in_monitor_(true, is_async, Some(Duration::from_millis(3_000)))
    }

    pub fn plug_in_monitor() -> ResultType<()> {
        let mut is_async = false;
        if let Err(e) = check_install_driver(&mut is_async) {
            log::error!("Failed to install driver: {}", e);
            bail!("Failed to install driver.");
        }

        if get_monitor_count() == VIRTUAL_DISPLAY_MAX_COUNT {
            bail!("There are already {VIRTUAL_DISPLAY_MAX_COUNT} monitors plugged in.");
        }

        plug_in_monitor_(true, is_async, None)
    }

    // `index` the display index to plug out. -1 means plug out all.
    // `force_all` is used to forcibly plug out all virtual displays.
    // `force_one` is used to forcibly plug out one virtual display managed by other processes
    //             if there're no virtual displays managed by RustDesk.
    pub fn plug_out_monitor(index: i32, force_all: bool, force_one: bool) -> ResultType<()> {
        let plug_out_all = index == super::IDD_PLUG_OUT_ALL_INDEX;
        // If `plug_out_all and force_all` is true, forcibly plug out all virtual displays.
        // Though the driver may be controlled by other processes,
        // we still forcibly plug out all virtual displays.
        //
        // 1. RustDesk plug in 2 virtual displays. (RustDesk)
        // 2. Other process plug out all virtual displays. (User manually)
        // 3. Other process plug in 1 virtual display. (User manually)
        // 4. RustDesk plug out all virtual displays in this call. (RustDesk disconnect)
        //
        // This is not a normal scenario, RustDesk will plug out virtual display unexpectedly.
        let mut plug_in_count = VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::Relaxed);
        let amyuni_count = get_monitor_count();
        if !plug_out_all {
            if plug_in_count == 0 && amyuni_count > 0 {
                if force_one {
                    plug_in_count = 1;
                } else {
                    bail!("The virtual display is managed by other processes.");
                }
            }
        } else {
            // Ignore the message if trying to plug out all virtual displays.
        }

        let all_count = windows::get_device_names(None).len();
        let mut to_plug_out_count = match all_count {
            0 => return Ok(()),
            1 => {
                if plug_in_count == 0 {
                    bail!("No virtual displays to plug out.")
                } else {
                    if force_all {
                        1
                    } else {
                        bail!("This only virtual display cannot be plugged out.")
                    }
                }
            }
            _ => {
                if all_count == plug_in_count {
                    if force_all {
                        all_count
                    } else {
                        all_count - 1
                    }
                } else {
                    plug_in_count
                }
            }
        };
        if to_plug_out_count != 0 && !plug_out_all {
            to_plug_out_count = 1;
        }

        for _i in 0..to_plug_out_count {
            let _ = plug_monitor_(false, None);
        }
        Ok(())
    }

    #[inline]
    pub fn get_monitor_count() -> usize {
        windows::get_device_names(Some(super::AMYUNI_IDD_DEVICE_STRING)).len()
    }

    #[inline]
    pub fn is_my_display(name: &str) -> bool {
        windows::get_device_names(Some(super::AMYUNI_IDD_DEVICE_STRING))
            .iter()
            .any(|s| windows::is_device_name(s, name))
    }
}

mod windows {
    use std::ptr::null_mut;
    use winapi::{
        shared::{
            devguid::GUID_DEVCLASS_DISPLAY,
            minwindef::{DWORD, FALSE},
            ntdef::ULONG,
        },
        um::{
            cfgmgr32::{CM_Get_DevNode_Status, CR_SUCCESS},
            cguid::GUID_NULL,
            setupapi::{
                SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceRegistryPropertyW,
                SP_DEVINFO_DATA,
            },
            wingdi::{
                DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_MIRRORING_DRIVER,
            },
            winnt::HANDLE,
            winuser::{EnumDisplayDevicesW, EnumDisplaySettingsExW, ENUM_CURRENT_SETTINGS},
        },
    };

    const DIGCF_PRESENT: DWORD = 0x00000002;
    const SPDRP_DEVICEDESC: DWORD = 0x00000000;
    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

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

    pub(super) fn get_device_names(device_string: Option<&str>) -> Vec<String> {
        let mut device_names = Vec::new();
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

            if let (Ok(device_name), Ok(ds)) = (
                String::from_utf16(&dd.DeviceName),
                String::from_utf16(&dd.DeviceString),
            ) {
                if let Some(s) = device_string {
                    if ds.len() >= s.len() && &ds[..s.len()] == s {
                        device_names.push(device_name);
                    }
                } else {
                    device_names.push(device_name);
                }
            }
        }
        device_names
    }

    pub(super) fn get_display_drivers() -> Vec<(String, u32)> {
        let mut display_drivers: Vec<(String, u32)> = Vec::new();

        let device_info_set = unsafe {
            SetupDiGetClassDevsW(
                &GUID_DEVCLASS_DISPLAY,
                null_mut(),
                null_mut(),
                DIGCF_PRESENT,
            )
        };

        if device_info_set == INVALID_HANDLE_VALUE {
            println!(
                "Failed to get device information set. Error: {}",
                std::io::Error::last_os_error()
            );
            return display_drivers;
        }

        let mut device_info_data = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ClassGuid: GUID_NULL,
            DevInst: 0,
            Reserved: 0,
        };

        let mut device_index = 0;
        loop {
            let result = unsafe {
                SetupDiEnumDeviceInfo(device_info_set, device_index, &mut device_info_data)
            };
            if result == 0 {
                break;
            }

            let mut data_type: DWORD = 0;
            let mut required_size: DWORD = 0;

            // Get the required buffer size for the driver description
            let mut buffer;
            unsafe {
                SetupDiGetDeviceRegistryPropertyW(
                    device_info_set,
                    &mut device_info_data,
                    SPDRP_DEVICEDESC,
                    &mut data_type,
                    null_mut(),
                    0,
                    &mut required_size,
                );

                buffer = vec![0; required_size as usize / 2];
                SetupDiGetDeviceRegistryPropertyW(
                    device_info_set,
                    &mut device_info_data,
                    SPDRP_DEVICEDESC,
                    &mut data_type,
                    buffer.as_mut_ptr() as *mut u8,
                    required_size,
                    null_mut(),
                );
            }

            let Ok(driver_description) = String::from_utf16(&buffer) else {
                println!("Failed to convert driver description to string");
                device_index += 1;
                continue;
            };

            let mut status: ULONG = 0;
            let mut problem_number: ULONG = 0;
            // Get the device status and problem number
            let config_ret = unsafe {
                CM_Get_DevNode_Status(
                    &mut status,
                    &mut problem_number,
                    device_info_data.DevInst,
                    0,
                )
            };
            if config_ret != CR_SUCCESS {
                println!(
                    "Failed to get device status. Error: {}",
                    std::io::Error::last_os_error()
                );
                device_index += 1;
                continue;
            }
            display_drivers.push((driver_description, problem_number));
            device_index += 1;
        }

        display_drivers
    }
}

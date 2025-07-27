#[cfg(windows)]
use dylib_virtual_display::win10::{idd, DRIVER_INSTALL_PATH};

#[cfg(windows)]
use std::{
    ffi::CStr,
    io::{self, Read},
    path::Path,
};

#[cfg(windows)]
fn prompt_input() -> u8 {
    println!("Press  key                  execute:");
    println!("       1. 'x'               1. exit");
    println!("       2. 'i'               2. install or update driver");
    println!("       3. 'u'               3. uninstall driver");
    println!("       4. 'c'               4. create device");
    println!("       5. 'd'               5. destroy device");
    println!("       6. '1','2','3'       6. plug in monitor 0,1,2");
    println!("       7. '4','5','6'       7. plug out monitor 0,1,2");

    io::stdin()
        .bytes()
        .next()
        .and_then(|result| result.ok())
        .unwrap_or(0)
}

#[cfg(windows)]
unsafe fn plug_in(index: idd::UINT, edid: idd::UINT) {
    println!("Plug in monitor begin");
    if idd::FALSE == idd::MonitorPlugIn(index, edid, 25) {
        println!(
            "{}",
            CStr::from_ptr(idd::GetLastMsg())
                .to_str()
                .unwrap_or_default()
        );
    } else {
        println!("Plug in monitor done");

        let mut modes: Vec<idd::MonitorMode> = Vec::new();
        modes.push(idd::MonitorMode {
            width: 1920 as idd::DWORD,
            height: 1080 as idd::DWORD,
            sync: 60 as idd::DWORD,
        });
        modes.push(idd::MonitorMode {
            width: 1024 as idd::DWORD,
            height: 768 as idd::DWORD,
            sync: 60 as idd::DWORD,
        });
        if idd::FALSE == idd::MonitorModesUpdate(index, modes.len() as u32, modes.as_mut_ptr()) {
            println!(
                "{}",
                CStr::from_ptr(idd::GetLastMsg())
                    .to_str()
                    .unwrap_or_default()
            );
        }
    }
}

#[cfg(windows)]
unsafe fn plug_out(index: idd::UINT) {
    println!("Plug out monitor begin");
    if idd::FALSE == idd::MonitorPlugOut(index) {
        println!(
            "{}",
            CStr::from_ptr(idd::GetLastMsg())
                .to_str()
                .unwrap_or_default()
        );
    } else {
        println!("Plug out monitor done");
    }
}

fn main() {
    #[cfg(windows)]
    {
        let abs_path = match Path::new(DRIVER_INSTALL_PATH).canonicalize() {
            Ok(p) => p,
            Err(e) => {
                println!("Failed to get absolute path of driver install: {:?}", e);
                return;
            }
        };

        unsafe {
            let invalid_device = 0 as idd::HSWDEVICE;
            let mut h_sw_device = invalid_device;

            let full_inf_path: Vec<u16> = abs_path
                .to_string_lossy()
                .as_ref()
                .encode_utf16()
                .chain(Some(0).into_iter())
                .collect();

            loop {
                match prompt_input() as char {
                    'x' => break,
                    'i' => {
                        println!("Install or update driver begin, {}", abs_path.display());
                        let mut reboot_required = idd::FALSE;
                        if idd::InstallUpdate(full_inf_path.as_ptr() as _, &mut reboot_required)
                            == idd::FALSE
                        {
                            println!(
                                "{}",
                                CStr::from_ptr(idd::GetLastMsg())
                                    .to_str()
                                    .unwrap_or_default()
                            );
                        } else {
                            println!(
                                "Install or update driver done, reboot is {} required",
                                if reboot_required == idd::FALSE {
                                    "not"
                                } else {
                                    ""
                                }
                            );
                        }
                    }
                    'u' => {
                        println!("Uninstall driver begin {}", abs_path.display());
                        let mut reboot_required = idd::FALSE;
                        if idd::Uninstall(full_inf_path.as_ptr() as _, &mut reboot_required)
                            == idd::FALSE
                        {
                            println!(
                                "{}",
                                CStr::from_ptr(idd::GetLastMsg())
                                    .to_str()
                                    .unwrap_or_default()
                            );
                        } else {
                            println!(
                                "Uninstall driver done, reboot is {} required",
                                if reboot_required == idd::FALSE {
                                    "not"
                                } else {
                                    ""
                                }
                            );
                        }
                    }
                    'c' => {
                        println!("Create device begin");
                        if h_sw_device != invalid_device {
                            println!("Device created before");
                            continue;
                        }
                        if idd::FALSE == idd::DeviceCreate(&mut h_sw_device) {
                            println!(
                                "{}",
                                CStr::from_ptr(idd::GetLastMsg())
                                    .to_str()
                                    .unwrap_or_default()
                            );
                            idd::DeviceClose(h_sw_device);
                            h_sw_device = invalid_device;
                        } else {
                            println!("Create device done");
                        }
                    }
                    'd' => {
                        println!("Close device begin");
                        idd::DeviceClose(h_sw_device);
                        h_sw_device = invalid_device;
                        println!("Close device done");
                    }
                    '1' => plug_in(0, 0),
                    '2' => plug_in(1, 0),
                    '3' => plug_in(2, 0),
                    '4' => plug_out(0),
                    '5' => plug_out(1),
                    '6' => plug_out(2),
                    _ => {}
                }
            }

            idd::DeviceClose(h_sw_device);
        }
    }
}

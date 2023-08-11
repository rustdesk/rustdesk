use std::io::{self, Read};
#[cfg(windows)]
use virtual_display;

#[cfg(windows)]
fn prompt_input() -> u8 {
    println!("Press  key          execute:");
    println!("       1. 'q'       1. quit");
    println!("       2. 'i'       2. install or update driver");
    println!("       3. 'u'       3. uninstall driver");
    println!("       4. 'c'       4. create device");
    println!("       5. 'd'       5. destroy device");
    println!("       6. '1'       6. plug in monitor 0,1,2");
    println!("       7. '4'       7. plug out monitor 0,1,2");

    io::stdin()
        .bytes()
        .next()
        .and_then(|result| result.ok())
        .unwrap_or(0)
}

#[cfg(windows)]
fn plug_in(monitor_index: u32) {
    println!("Plug in monitor begin");
    if let Err(e) = virtual_display::plug_in_monitor(monitor_index as _) {
        println!("{}", e);
    } else {
        println!("Plug in monitor done");
    }
}

#[cfg(windows)]
fn plug_out(monitor_index: u32) {
    println!("Plug out monitor begin");
    if let Err(e) = virtual_display::plug_out_monitor(monitor_index as _) {
        println!("{}", e);
    } else {
        println!("Plug out monitor done");
    }
}

#[cfg(windows)]
fn main() {
    loop {
        let chr = prompt_input();
        match chr as char {
            'q' => break,
            'i' => {
                println!("Install or update driver begin");
                let mut reboot_required = false;
                if let Err(e) = virtual_display::install_update_driver(&mut reboot_required) {
                    println!("{}", e);
                } else {
                    println!(
                        "Install or update driver done, reboot is {} required",
                        if reboot_required { "" } else { "not" }
                    );
                }
            }
            'u' => {
                println!("Uninstall driver begin");
                let mut reboot_required = false;
                if let Err(e) = virtual_display::uninstall_driver(&mut reboot_required) {
                    println!("{}", e);
                } else {
                    println!(
                        "Uninstall driver done, reboot is {} required",
                        if reboot_required { "" } else { "not" }
                    );
                }
            }
            'c' => {
                println!("Create device begin");
                if virtual_display::is_device_created() {
                    println!("Device created before");
                    continue;
                }
                if let Err(e) = virtual_display::create_device() {
                    println!("{}", e);
                } else {
                    println!("Create device done");
                }
            }
            'd' => {
                println!("Close device begin");
                virtual_display::close_device();
                println!("Close device done");
            }
            '1' => plug_in(0),
            '2' => plug_in(1),
            '3' => plug_in(2),
            '4' => plug_out(0),
            '5' => plug_out(1),
            '6' => plug_out(2),
            _ => {}
        }
    }
}

#[cfg(not(windows))]
fn main() {}

use std::io::{self, Read};
use virtual_display;

fn prompt_input() -> u8 {
    println!("Press  key          execute:");
    println!("       1. 'x'       1. exit");
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

fn plug_in() {
    println!("Plug in monitor begin");
    if let Err(e) = virtual_display::plug_in_monitor() {
        println!("{}", e);
    } else {
        println!("Plug in monitor done");
    }
}

fn plug_out() {
    println!("Plug out monitor begin");
    if let Err(e) = virtual_display::plug_out_monitor() {
        println!("{}", e);
    } else {
        println!("Plug out monitor done");
    }
}

fn main() {
    loop {
        match prompt_input() as char {
            'x' => break,
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
            '1' => plug_in(),
            '4' => plug_out(),
            _ => {}
        }
    }
}

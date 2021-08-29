// Specify the Windows subsystem to eliminate console window.
// Requires Rust 1.18.
//#![windows_subsystem = "windows"]

use hbb_common::log;
use rustdesk::*;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() {
    common::test_rendezvous_server();
    common::test_nat_type();
    #[cfg(target_os = "android")]
    crate::common::check_software_update();
    mobile::Session::start("");
}

#[cfg(not(any(target_os = "android", target_os = "ios", feature = "cli")))]
fn main() {
    let mut args = Vec::new();
    let mut i = 0;
    for arg in std::env::args() {
        if i > 0 {
            args.push(arg);
        }
        i += 1;
    }
    if args.len() > 0 && args[0] == "--version" {
        println!("{}", crate::VERSION);
        return;
    }
    #[cfg(not(feature = "inline"))]
    {
        use hbb_common::env_logger::*;
        init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    }
    #[cfg(feature = "inline")]
    {
        let mut path = hbb_common::config::Config::log_path();
        if args.len() > 0 && args[0].starts_with("--") {
            let name = args[0].replace("--", "");
            if !name.is_empty() {
                path.push(name);
            }
        }
        use flexi_logger::*;
        Logger::with_env_or_str("debug")
            .log_to_file()
            .format(opt_format)
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(6),
            )
            .directory(path)
            .start()
            .ok();
    }
    if args.is_empty() {
        std::thread::spawn(move || start_server(false, false));
    } else {
        #[cfg(windows)]
        {
            if args[0] == "--uninstall" {
                if let Err(err) = platform::uninstall_me() {
                    log::error!("Failed to uninstall: {}", err);
                }
                return;
            } else if args[0] == "--update" {
                hbb_common::allow_err!(platform::update_me());
                return;
            } else if args[0] == "--reinstall" {
                hbb_common::allow_err!(platform::uninstall_me());
                hbb_common::allow_err!(platform::install_me("desktopicon startmenu",));
                return;
            }
        }
        if args[0] == "--remove" {
            if args.len() == 2 {
                // sleep a while so that process of removed exe exit
                std::thread::sleep(std::time::Duration::from_secs(1));
                std::fs::remove_file(&args[1]).ok();
                return;
            }
        } else if args[0] == "--service" {
            log::info!("start --service");
            start_os_service();
            return;
        } else if args[0] == "--server" {
            log::info!("start --server");
            start_server(true, true);
            return;
        } else if args[0] == "--import-config" {
            if args.len() == 2 {
                hbb_common::config::Config::import(&args[1]);
            }
            return;
        } else if args[0] == "--password" {
            if args.len() == 2 {
                ipc::set_password(args[1].to_owned()).unwrap();
            }
            return;
        }
    }
    ui::start(&mut args[..]);
}

#[cfg(feature = "cli")]
fn main() {
    use clap::App;
    let args = format!(
        "-p, --port-forward=[PORT-FORWARD-OPTIONS] 'Format: remote-id:local-port:remote-port[:remote-host]'
       -s, --server... 'Start server'",
    );
    let matches = App::new("rustdesk")
        .version(crate::VERSION)
        .author("CarrieZ Studio<info@rustdesk.com>")
        .about("RustDesk command line tool")
        .args_from_usage(&args)
        .get_matches();
    use hbb_common::env_logger::*;
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    if let Some(p) = matches.value_of("port-forward") {
        let options: Vec<String> = p.split(":").map(|x| x.to_owned()).collect();
        if options.len() < 3 {
            log::error!("Wrong port-forward options");
            return;
        }
        let mut port = 0;
        if let Ok(v) = options[1].parse::<i32>() {
            port = v;
        } else {
            log::error!("Wrong local-port");
            return;
        }
        let mut remote_port = 0;
        if let Ok(v) = options[2].parse::<i32>() {
            remote_port = v;
        } else {
            log::error!("Wrong remote-port");
            return;
        }
        let mut remote_host = "localhost".to_owned();
        if options.len() > 3 {
            remote_host = options[3].clone();
        }
        cli::start_one_port_forward(options[0].clone(), port, remote_host, remote_port);
    }
}

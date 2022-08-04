// Specify the Windows subsystem to eliminate console window.
// Requires Rust 1.18.
//#![windows_subsystem = "windows"]

use hbb_common::log;
use librustdesk::*;

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() {
    common::test_rendezvous_server();
    common::test_nat_type();
    #[cfg(target_os = "android")]
    crate::common::check_software_update();
}

#[cfg(not(any(target_os = "android", target_os = "ios", feature = "cli")))]
fn main() {
    // https://docs.rs/flexi_logger/latest/flexi_logger/error_info/index.html#write
    let mut _async_logger_holder: Option<flexi_logger::LoggerHandle> = None;
    let mut args = Vec::new();
    let mut i = 0;
    let mut is_setup = false;
    for arg in std::env::args() {
        if i == 0 && common::is_setup(&arg) {
            is_setup = true;
        } else if i > 0 {
            args.push(arg);
        }
        i += 1;
    }
    if is_setup {
        if args.is_empty() {
            args.push("--install".to_owned());
        } else if args[0] == "--noinstall" {
            args.clear();
        }
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
        if let Ok(x) = Logger::try_with_env_or_str("debug") {
            _async_logger_holder = x
                .log_to_file(FileSpec::default().directory(path))
                .write_mode(WriteMode::Async)
                .format(opt_format)
                .rotate(
                    Criterion::Age(Age::Day),
                    Naming::Timestamps,
                    Cleanup::KeepLogFiles(6),
                )
                .start()
                .ok();
        }
    }
    if args.is_empty() {
        std::thread::spawn(move || start_server(false));
    } else {
        #[cfg(windows)]
        {
            if args[0] == "--uninstall" {
                if let Err(err) = platform::uninstall_me() {
                    log::error!("Failed to uninstall: {}", err);
                }
                return;
            } else if args[0] == "--after-install" {
                if let Err(err) = platform::run_after_install() {
                    log::error!("Failed to after-install: {}", err);
                }
                return;
            } else if args[0] == "--before-uninstall" {
                if let Err(err) = platform::run_before_uninstall() {
                    log::error!("Failed to before-uninstall: {}", err);
                }
                return;
            } else if args[0] == "--update" {
                hbb_common::allow_err!(platform::update_me());
                return;
            } else if args[0] == "--reinstall" {
                hbb_common::allow_err!(platform::uninstall_me());
                hbb_common::allow_err!(platform::install_me(
                    "desktopicon startmenu",
                    "".to_owned(),
                    false,
                    false,
                ));
                return;
            } else if args[0] == "--silent-install" {
                hbb_common::allow_err!(platform::install_me(
                    "desktopicon startmenu",
                    "".to_owned(),
                    true,
                    args.len() > 1,
                ));
                return;
            } else if args[0] == "--extract" {
                #[cfg(feature = "with_rc")]
                hbb_common::allow_err!(crate::rc::extract_resources(&args[1]));
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
            #[cfg(not(target_os = "macos"))]
            {
                start_server(true);
                return;
            }
            #[cfg(target_os = "macos")]
            {
                std::thread::spawn(move || start_server(true));
            }
        } else if args[0] == "--import-config" {
            if args.len() == 2 {
                let filepath;
                let path = std::path::Path::new(&args[1]);
                if !path.is_absolute() {
                    let mut cur = std::env::current_dir().unwrap();
                    cur.push(path);
                    filepath = cur.to_str().unwrap().to_string();
                } else {
                    filepath = path.to_str().unwrap().to_string();
                }
                import_config(&filepath);
            }
            return;
        } else if args[0] == "--password" {
            if args.len() == 2 {
                ipc::set_permanent_password(args[1].to_owned()).unwrap();
            }
            return;
        } else if args[0] == "--check-hwcodec-config" {
            #[cfg(feature = "hwcodec")]
            scrap::hwcodec::check_config();
            return;
        }
    }
    ui::start(&mut args[..]);
    _async_logger_holder.map(|x| x.flush());
}

fn import_config(path: &str) {
    use hbb_common::{config::*, get_modified_time};
    let path2 = path.replace(".toml", "2.toml");
    let path2 = std::path::Path::new(&path2);
    let path = std::path::Path::new(path);
    log::info!("import config from {:?} and {:?}", path, path2);
    let config: Config = load_path(path.into());
    if config.is_empty() {
        log::info!("Empty source config, skipped");
        return;
    }
    if get_modified_time(&path) > get_modified_time(&Config::file()) {
        if store_path(Config::file(), config).is_err() {
            log::info!("config written");
        }
    }
    let config2: Config2 = load_path(path2.into());
    if get_modified_time(&path2) > get_modified_time(&Config2::file()) {
        if store_path(Config2::file(), config2).is_err() {
            log::info!("config2 written");
        }
    }
}

#[cfg(feature = "cli")]
fn main() {
    use clap::App;
    let args = format!(
        "-p, --port-forward=[PORT-FORWARD-OPTIONS] 'Format: remote-id:local-port:remote-port[:remote-host]'
        -k, --key=[KEY] ''
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
        let key = matches.value_of("key").unwrap_or("").to_owned();
        cli::start_one_port_forward(options[0].clone(), port, remote_host, remote_port, key);
    }
}

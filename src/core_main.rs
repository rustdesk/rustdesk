use std::{env::Args, collections::HashMap};

use hbb_common::log;

struct SetOption {
    name: String,
    allowed_values: Vec<String>,
    remove_on_value: String,
    value_as_upper: bool
}

// shared by flutter and sciter main function
pub fn core_main() -> Option<Vec<String>> {
    // https://docs.rs/flexi_logger/latest/flexi_logger/error_info/index.html#write
    // though async logger more efficient, but it also causes more problems, disable it for now
    // let mut _async_logger_holder: Option<flexi_logger::LoggerHandle> = None;
    let mut args = Vec::new();
    let mut flutter_args = Vec::new();
    let mut i = 0;
    let mut is_setup = false;
    let mut _is_elevate = false;
    let mut _is_run_as_system = false;
    let mut _is_flutter_connect = false;
    for arg in std::env::args() {
        // to-do: how to pass to flutter?
        if i == 0 && crate::common::is_setup(&arg) {
            is_setup = true;
        } else if i > 0 {
            #[cfg(feature = "flutter")]
            if arg == "--connect" {
                _is_flutter_connect = true;
            }
            if arg == "--elevate" {
                _is_elevate = true;
            } else if arg == "--run-as-system" {
                _is_run_as_system = true;
            } else {
                args.push(arg);
            }
        }
        i += 1;
    }

    #[cfg(feature = "flutter")]
    if _is_flutter_connect {
        return core_main_invoke_new_connection(std::env::args());
    }
    if args.contains(&"--install".to_string()) {
        is_setup = true;
    }
    if is_setup {

        if !flutter_args.contains(&"--install".to_string()) {
            flutter_args.push("--install".to_string());
        }

        if !args.contains(&"--install".to_string()) {
            args.push("--install".to_string());
        }
    }
    if args.contains(&"--noinstall".to_string()) {
        args.clear();
    }
    if args.len() > 0 && args[0] == "--version" {
        println!("{}", crate::VERSION);
        return None;
    }
    #[cfg(debug_assertions)]
    {
        use hbb_common::env_logger::*;
        init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    }
    #[cfg(not(debug_assertions))]
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
            // _async_logger_holder =
            x.log_to_file(FileSpec::default().directory(path))
                //.write_mode(WriteMode::Async)
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
    #[cfg(windows)]
    #[cfg(not(debug_assertions))]
    if !crate::platform::is_installed() && args.is_empty() {
        crate::platform::elevate_or_run_as_system(is_setup, _is_elevate, _is_run_as_system);
    }
    if args.is_empty() {
        std::thread::spawn(move || crate::start_server(false));
    } else {
        #[cfg(windows)]
        {
            use crate::platform;
            if args[0] == "--uninstall" {
                if let Err(err) = platform::uninstall_me() {
                    log::error!("Failed to uninstall: {}", err);
                }
                return None;
            } else if args[0] == "--after-install" {
                if let Err(err) = platform::run_after_install() {
                    log::error!("Failed to after-install: {}", err);
                }
                return None;
            } else if args[0] == "--before-uninstall" {
                if let Err(err) = platform::run_before_uninstall() {
                    log::error!("Failed to before-uninstall: {}", err);
                }
                return None;
            } else if args[0] == "--update" {
                hbb_common::allow_err!(platform::update_me());
                return None;
            } else if args[0] == "--reinstall" {
                hbb_common::allow_err!(platform::uninstall_me());
                hbb_common::allow_err!(platform::install_me(
                    "desktopicon startmenu",
                    "".to_owned(),
                    false,
                    false,
                ));
                return None;
            } else if args[0] == "--silent-install" {
                hbb_common::allow_err!(platform::install_me(
                    "desktopicon startmenu",
                    "".to_owned(),
                    true,
                    args.len() > 1,
                ));
                return None;
            } else if args[0] == "--extract" {
                #[cfg(feature = "with_rc")]
                hbb_common::allow_err!(crate::rc::extract_resources(&args[1]));
                return None;
            }
        }

        return parse_args(args, flutter_args);
    }
    //_async_logger_holder.map(|x| x.flush());
    #[cfg(feature = "flutter")]
    return Some(flutter_args);
    #[cfg(not(feature = "flutter"))]
    return Some(args);
}

fn parse_args(args: Vec<String>, flutter_args: Vec<String>) -> Option<Vec<String>>
{
    let mut inter = args.clone().into_iter();

    let mut unhandled_commands: Vec<String> = [].to_vec();

    let mut any_changes: bool = false;

    let mut item : Option<String> = inter.next();
    while !item.is_none() {
        let field: String = item.to_owned().unwrap();

        if field == "--remove" {
            let filename: Option<String> = inter.next();

            if !filename.is_none() {
                // sleep a while so that process of removed exe exit
                std::thread::sleep(std::time::Duration::from_secs(1));
                std::fs::remove_file(filename.unwrap()).ok();
                return None;
            }
        } else if field == "--service" {
            log::info!("start --service");
            crate::start_os_service();
            return None;
        } else if field == "--server" {
            log::info!("start --server");
            #[cfg(not(target_os = "macos"))]
            {
                crate::start_server(true);
                return None;
            }
            #[cfg(target_os = "macos")]
            {
                std::thread::spawn(move || crate::start_server(true));
                // to-do: for flutter, starting tray not ready yet, or we can reuse sciter's tray implementation.
            }
        } else if field == "-import-config" {
            let filename: Option<String> = inter.next();

            if !filename.is_none() {
                let filepath;
                let path_file = &filename.unwrap().to_owned();
                let path = std::path::Path::new(path_file);
                if !path.is_absolute() {
                    let mut cur = std::env::current_dir().unwrap();
                    cur.push(path);
                    filepath = cur.to_str().unwrap().to_string();
                } else {
                    filepath = path.to_str().unwrap().to_string();
                }
                import_config(&filepath);
                any_changes = true;
            }
        } else if field == "--password" {
            let new_pass: Option<String> = inter.next();
            if !new_pass.is_none() {
                let p: String = new_pass.unwrap().to_owned();
                match crate::ipc::set_permanent_password(p) {
                    Ok(_) => { any_changes = true; },
                    Err(e) => log::warn!("Failed to set password: {e}")
                }
            } else {
                log::warn!("Error changing password: no new password specified.");
            }
        } else if field == "--check-hwcodec-config" {
            #[cfg(feature = "hwcodec")]
            {
                scrap::hwcodec::check_config();
                return None;
            }
            continue;
        } else if field == "--cm" {
            // call connection manager to establish connections
            // meanwhile, return true to call flutter window to show control panel
            #[cfg(feature = "flutter")]
            crate::flutter::connection_manager::start_listen_ipc_thread();
        } else if field.starts_with("--set-") {

            let real_field: String = field[6..].to_string();
            let value_opt: Option<String> = inter.next();

            if !value_opt.is_none() && !real_field.is_empty() {
                let none_value: String = String::from("NONE");
                let yes_value: String = String::from("Y");
                let no_value: String = String::from("N");

                let empty_vec: Vec<String> = [].to_vec();
                let bool_vec: Vec<String> = [yes_value.clone(), no_value.clone()].to_vec();
                
                let available_options = [
                    SetOption { name: String::from("rendezvous_server"), allowed_values: empty_vec.clone(), remove_on_value: none_value.clone(), value_as_upper: false },
                    SetOption { name: String::from("custom-rendezvous-server"), allowed_values: empty_vec.clone(), remove_on_value: none_value.clone(), value_as_upper: false },
                    SetOption { name: String::from("relay-server"), allowed_values: empty_vec.clone(), remove_on_value: none_value.clone(), value_as_upper: false },
                    SetOption { name: String::from("video-save-directory"), allowed_values: empty_vec.clone(), remove_on_value: none_value.clone(), value_as_upper: false },
                    SetOption { name: String::from("audio-input"), allowed_values: empty_vec.clone(), remove_on_value: none_value.clone(), value_as_upper: false },
                    
                    SetOption { name: String::from("allow-darktheme"), allowed_values: bool_vec.clone(), remove_on_value: no_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-clipboard"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-lan-discovery"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("direct-server"), allowed_values: bool_vec.clone(), remove_on_value: no_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("allow-auto-record-incoming"), allowed_values: bool_vec.clone(), remove_on_value: no_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-record-session"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-remote-restart"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("allow-remote-config-modification"), allowed_values: bool_vec.clone(), remove_on_value: no_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-tunnel"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-keyboard"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-audio"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    SetOption { name: String::from("enable-file-transfer"), allowed_values: bool_vec.clone(), remove_on_value: yes_value.clone(), value_as_upper: true },
                    
                    SetOption { name: String::from("temporary-password-length"), allowed_values: [String::from("6"), String::from("8"), String::from("10")].to_vec(), remove_on_value: yes_value.clone(), value_as_upper: false },
                    SetOption { name: String::from("verification-method"), allowed_values: [String::from("use-permanent-password"), String::from("use-temporary-password"), String::from("use-both-passwords")].to_vec(), remove_on_value: none_value.clone(), value_as_upper: false }
                ];

                let special_options: [String; 5] = [String::from("whitelist"), String::from("socks"), String::from("socks-proxy"), String::from("socks-username"), String::from("socks-password")];

                let option_orig: String = real_field.to_owned();
                let option: String = option_orig.to_lowercase();

                let option_str: &str = option.as_str();

                let value : String = value_opt.unwrap().to_owned();

                if special_options.contains(&option) {
                    if option == String::from("whitelist") {
                        if value.is_empty() || value == none_value {
                            crate::ipc::set_option(option_str, "");
                            any_changes = true;
                        } else {
                            let mut all_valid: bool = true;
                            let ips = value.split(",");
                            for ip in ips {

                                let res = ip.parse::<std::net::IpAddr>();
                                if res.is_err() {
                                    log::warn!("{ip} is not a valid IP-address (Option: whitelist)");
                                    all_valid = false;
                                    break;
                                }
                            }

                            if all_valid {
                                crate::ipc::set_option(option_str, value.as_str());
                                any_changes = true;
                            }

                        }
                        log::info!("Option: {option} = {value}.");
                        item = inter.next();
                        continue;
                    } else if option == String::from("socks") || option == String::from("socks-proxy") || option == String::from("socks-username") || option == String::from("socks-password") {

                        let socks_res = crate::ipc::get_socks();
                        
                        let mut socks;

                        if socks_res.is_none() {
                            socks = hbb_common::config::Socks5Server::default();
                        } else {
                            socks = socks_res.unwrap();
                        }
                        
                        if value.is_empty() || value == none_value {
                            socks.proxy = String::from("");
                            socks.username = String::from("");
                            socks.password = String::from("");
                        } else if option == String::from("socks") {
                            let parts = value.split(";;").collect::<Vec<&str>>();

                            if parts.len() != 3 {
                                log::warn!("Failed to update Socket5 configuration: Unexpected format. Format: PROXY;;USERNAME;;PASSWORD");
                                return None;
                            } else {
                                socks.proxy = parts.get(0).unwrap().to_string();
                                socks.username = parts.get(1).unwrap().to_string();
                                socks.password = parts.get(2).unwrap().to_string();
                            }
                        } else if option == String::from("socks-proxy") {
                            socks.proxy = value;
                        } else if option == String::from("socks-username") {
                            socks.username = value;
                        } else if option == String::from("socks-password") {
                            socks.password = value;
                        }

                        let res = crate::ipc::set_socks(socks);
                        if res.is_err() {
                            log::warn!("Failed to update Socket5 configuration.");
                            item = inter.next();
                            continue;
                        }

                        any_changes = true;
                        log::info!("Socks5 parameters updated.");
                        item = inter.next();
                        continue;
                    }
                }

                let mut handled: bool = false;

                for o in available_options
                {
                    if o.name == option {
                        if o.allowed_values.is_empty() || o.allowed_values.contains(&value.to_lowercase()) || o.allowed_values.contains(&value.to_uppercase()) {
                            if value.is_empty() || value == none_value || o.remove_on_value.to_lowercase() == value.to_lowercase() {
                                crate::ipc::set_option(option_str, "");
                            } else if o.value_as_upper {
                                crate::ipc::set_option(option_str, value.to_uppercase().as_str());
                            } else {
                                crate::ipc::set_option(option_str, value.as_str());
                            }
                            
                            any_changes = true;
                            log::info!("Option: {option} = {value}.");
                        } else {
                            log::warn!("Value {value} is not allowed in option {option}");
                        }
                        handled = true;
                        break;
                    }
                }

                if !handled {
                    log::warn!("Option {option} is not configurable through --set-*.");
                }

            } else if real_field.is_empty() {
                log::warn!("No option specified");
            } else {
                log::warn!("No value specified");
            }
        } else if field == "--exit" {
            return None; //Use --exit if you only want to set the password using --password for example.. Stops the program from running any further!
        } else {
            unhandled_commands.push(field.clone());
        }

        item = inter.next();
    }

    if any_changes {

        let h = hbb_common::config::Config::file().to_owned();
        let tmp_config = h.to_owned().with_file_name("_RustDesk.toml");
        let tmp_config2 = h.with_file_name("_RustDesk2.toml");

        let config: hbb_common::config::Config = hbb_common::config::load_path(hbb_common::config::Config::file());
        let config2: hbb_common::config::Config2 = hbb_common::config::load_path(hbb_common::config::Config2::file());

        let _ = hbb_common::config::store_path( tmp_config, config);
        let _ = hbb_common::config::store_path( tmp_config2, config2);
        log::info!("Storing new configuration in temporary files.");
    }

    #[cfg(feature = "flutter")]
    return Some(flutter_args);
    #[cfg(not(feature = "flutter"))]
    return Some(unhandled_commands);
}

fn import_config(path: &str) {
    use hbb_common::{config::*, get_exe_time, get_modified_time};
    let path2 = path.replace(".toml", "2.toml");
    let path2 = std::path::Path::new(&path2);
    let path = std::path::Path::new(path);
    log::info!("import config from {:?} and {:?}", path, path2);
    let config: Config = load_path(path.into());
    if config.is_empty() {
        log::info!("Empty source config, skipped");
        return;
    }
    if get_modified_time(&path) > get_modified_time(&Config::file())
        && get_modified_time(&path) < get_exe_time()
    {
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

/// invoke a new connection
///
/// [Note]
/// this is for invoke new connection from dbus.
#[cfg(feature = "flutter")]
fn core_main_invoke_new_connection(mut args: Args) -> Option<Vec<String>> {
    args.position(|element| {
        return element == "--connect";
    })
    .unwrap();
    let peer_id = args.next().unwrap_or("".to_string());
    if peer_id.is_empty() {
        eprintln!("please provide a valid peer id");
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        use crate::dbus::invoke_new_connection;

        match invoke_new_connection(peer_id) {
            Ok(()) => {
                return None;
            }
            Err(err) => {
                log::error!("{}", err.as_ref());
                // return Some to invoke this new connection by self
                return Some(Vec::new());
            }
        }
    }
    return None;
}

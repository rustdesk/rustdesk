#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use librustdesk::*;

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
fn main() {
    if !common::global_init() {
        eprintln!("Global initialization failed.");
        return;
    }
    common::test_rendezvous_server();
    common::test_nat_type();
    common::global_clean();
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    feature = "cli",
    feature = "flutter"
)))]
fn main() {
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2);
    }
    if let Some(args) = crate::core_main::core_main().as_mut() {
        ui::start(args);
    }
    common::global_clean();
}

#[cfg(feature = "cli")]
fn main() {
    if !common::global_init() {
        return;
    }
    use clap::{Arg, ArgAction, Command};
    use hbb_common::log;
    let matches = Command::new("rustdesk")
        .version(crate::VERSION)
        .author("Purslane Ltd<info@rustdesk.com>")
        .about("RustDesk command line tool")
        .arg(
            Arg::new("port-forward")
                .short('p')
                .long("port-forward")
                .value_name("PORT-FORWARD-OPTIONS")
                .help("Format: remote-id:local-port:remote-port[:remote-host]"),
        )
        .arg(
            Arg::new("connect")
                .short('c')
                .long("connect")
                .value_name("REMOTE_ID")
                .help("test only"),
        )
        .arg(Arg::new("key").short('k').long("key").value_name("KEY"))
        .arg(
            Arg::new("server")
                .short('s')
                .long("server")
                .action(ArgAction::SetTrue)
                .help("Start server"),
        )
        .get_matches();
    use hbb_common::{config::LocalConfig, env_logger::*};
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    if let Some(p) = matches.get_one::<String>("port-forward") {
        let options: Vec<String> = p.split(':').map(|x| x.to_owned()).collect();
        if options.len() < 3 {
            log::error!("Wrong port-forward options");
            return;
        }
        let port = match options[1].parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                log::error!("Wrong local-port");
                return;
            }
        };
        let remote_port = match options[2].parse::<i32>() {
            Ok(v) => v,
            Err(_) => {
                log::error!("Wrong remote-port");
                return;
            }
        };
        let mut remote_host = "localhost".to_owned();
        if options.len() > 3 {
            remote_host = options[3].clone();
        }
        common::test_rendezvous_server();
        common::test_nat_type();
        let key = matches
            .get_one::<String>("key")
            .map(String::as_str)
            .unwrap_or("")
            .to_owned();
        let token = LocalConfig::get_option("access_token");
        cli::start_one_port_forward(
            options[0].clone(),
            port,
            remote_host,
            remote_port,
            key,
            token,
        );
    } else if let Some(p) = matches.get_one::<String>("connect") {
        common::test_rendezvous_server();
        common::test_nat_type();
        let key = matches
            .get_one::<String>("key")
            .map(String::as_str)
            .unwrap_or("")
            .to_owned();
        let token = LocalConfig::get_option("access_token");
        cli::connect_test(p, key, token);
    } else if matches.get_flag("server") {
        log::info!("id={}", hbb_common::config::Config::get_id());
        crate::start_server(true, false);
    }
    common::global_clean();
}

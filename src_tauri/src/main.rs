// Specify the Windows subsystem to eliminate console window.
// Requires Rust 1.18.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() {
    if !common::global_init() {
        return;
    }
    common::test_rendezvous_server();
    common::test_nat_type();
    #[cfg(target_os = "android")]
    crate::common::check_software_update();
    common::global_clean();
}

use hbb_common::log;
use librustdesk::{
    core_main,
    ui,
    invoke_handler::invoke_handler, common,
};
use tauri::{GlobalShortcutManager, Manager};


fn main() {
    if !common::global_init() {
        return;
    }
    println!("{}", !has_feature("custom-protocol"));
    let mut builder = tauri::Builder::default();
    builder = invoke_handler(builder);
    builder
    .setup(|app| {
        if let Some(args) = core_main::core_main().as_mut(){
            ui::start(&app.handle(), args);
        }
        Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
    common::global_clean();
}

// checks if the given Cargo feature is enabled.
fn has_feature(feature: &str) -> bool {
    use heck::AsShoutySnakeCase;
    // when a feature is enabled, Cargo sets the `CARGO_FEATURE_<name` env var to 1
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
    std::env::var(format!("CARGO_FEATURE_{}", AsShoutySnakeCase(feature)))
        .map(|x| x == "1")
        .unwrap_or(false)
}

#[cfg(feature = "cli")]
fn main() {
    if !common::global_init() {
        return;
    }
    use clap::App;
    use hbb_common::log;
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
    use hbb_common::{config::LocalConfig, env_logger::*};
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
        let token = LocalConfig::get_option("access_token");
        cli::start_one_port_forward(
            options[0].clone(),
            port,
            remote_host,
            remote_port,
            key,
            token,
        );
    }
    common::global_clean();
}

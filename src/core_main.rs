use hbb_common::log;

use crate::{flutter::connection_manager, start_os_service, start_server};

/// Main entry of the RustDesk Core.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
pub fn core_main() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    // TODO: implement core_main()
    if args.len() > 1 {
        if args[1] == "--cm" {
            // call connection manager to establish connections
            // meanwhile, return true to call flutter window to show control panel
            connection_manager::start_listen_ipc_thread();
            return true;
        }
        
        use hbb_common::env_logger::*;
        init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
        if args[1] == "--service" {
            log::info!("start --service");
            start_os_service();
            return false;
        }
        if args[1] == "--server" {
            log::info!("start --server");
            #[cfg(not(target_os = "macos"))]
            {
                start_server(true);
            }
            #[cfg(target_os = "macos")]
            {
                std::thread::spawn(move || start_server(true));
            }
            return false;
        }
    }
    true
}

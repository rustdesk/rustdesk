use hbb_common::log;

use crate::start_os_service;

/// Main entry of the RustDesk Core.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
pub fn core_main() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    // TODO: implement core_main()
    if args.len() > 1 {
        if args[1] == "--cm" {
            // call connection manager to establish connections
            // meanwhile, return true to call flutter window to show control panel
            return true;
        }
        if args[1] == "--service" {
            log::info!("start --service");
            start_os_service();
            return false;
        }
    }
    true
}

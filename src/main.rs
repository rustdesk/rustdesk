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

#[cfg(not(any(target_os = "android", target_os = "ios", feature = "flutter")))]
fn main() -> std::process::ExitCode {
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2);
    }
    let exit_code = match crate::core_main::core_main() {
        Some(crate::core_main::CoreMainAction::StartUi(mut args)) => {
            ui::start(&mut args);
            std::process::ExitCode::SUCCESS
        }
        Some(crate::core_main::CoreMainAction::ExitFailure(err)) => {
            eprintln!("Command failed: {err}");
            hbb_common::log::error!("Command failed: {err}");
            std::process::ExitCode::FAILURE
        }
        None => std::process::ExitCode::SUCCESS,
    };
    common::global_clean();
    exit_code
}

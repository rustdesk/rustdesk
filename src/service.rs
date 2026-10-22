use librustdesk::*;

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    crate::common::load_custom_client();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--copy-service-config")
    {
        if let Err(error) = crate::core_main::copy_service_config(&args[1..]) {
            eprintln!("Failed to copy service configuration: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() > 1 && args[1] == "--write-plists" {
        if let Err(e) = librustdesk::platform::write_plists() {
            eprintln!("Failed to write plists: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }
    hbb_common::init_log(false, "service");
    crate::start_os_service();
}

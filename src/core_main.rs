/// Main entry of the RustDesk Core.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
pub fn core_main() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    // TODO: implement core_main()
    if args.len() > 1 {
        if args[1] == "--cm" {
            // For test purpose only, this should stop any new window from popping up when a new connection is established.
            return false;
        }
    }
    true
}

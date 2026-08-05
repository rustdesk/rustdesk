import Cocoa
import Darwin
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
    var launched = false;

    static func main() {
        if rustdesk_headless_terminal_is_requested() {
            if rustdesk_core_main() {
                exit(EXIT_FAILURE)
            }
            exit(EXIT_SUCCESS)
        }
        _ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
    }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
      dummy_method_to_enforce_bundling()
    // https://github.com/leanflutter/window_manager/issues/214
    return false
  }
    
    override func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        if (launched) {
            handle_applicationShouldOpenUntitledFile();
        }
        return true
    }
    
    override func applicationDidFinishLaunching(_ aNotification: Notification) {
        launched = true;
        NSApplication.shared.activate(ignoringOtherApps: true);
    }
}

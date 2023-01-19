import Cocoa
import FlutterMacOS

@NSApplicationMain
class AppDelegate: FlutterAppDelegate {
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
      dummy_method_to_enforce_bundling()
    return true
  }
    
    override func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        handle_applicationShouldOpenUntitledFile();
        return true
    }
}

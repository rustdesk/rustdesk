import Cocoa
import FlutterMacOS
// import bitsdojo_window_macos

class MainFlutterWindow: NSWindow {
    override func awakeFromNib() {
        if (!rustdesk_core_main()){
            print("Rustdesk core returns false, exiting without launching Flutter app")
            NSApplication.shared.terminate(self)
        }
        let flutterViewController = FlutterViewController.init()
        let windowFrame = self.frame
        self.contentViewController = flutterViewController
        self.setFrame(windowFrame, display: true)
        
        RegisterGeneratedPlugins(registry: flutterViewController)
        
        super.awakeFromNib()
    }
    
//     override func bitsdojo_window_configure() -> UInt {
//         return BDW_CUSTOM_FRAME | BDW_HIDE_ON_STARTUP
//     }
}

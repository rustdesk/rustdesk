import Cocoa
import FlutterMacOS
import desktop_multi_window
// import bitsdojo_window_macos

import desktop_drop
import device_info_plus_macos
import flutter_custom_cursor
import package_info_plus_macos
import path_provider_macos
import screen_retriever
import sqflite
import tray_manager
import uni_links_desktop
import url_launcher_macos
import wakelock_macos
import window_manager
import window_size

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

        FlutterMultiWindowPlugin.setOnWindowCreatedCallback { controller in
            // Register the plugin which you want access from other isolate.
            // DesktopLifecyclePlugin.register(with: controller.registrar(forPlugin: "DesktopLifecyclePlugin"))
            DesktopDropPlugin.register(with: controller.registrar(forPlugin: "DesktopDropPlugin"))
            DeviceInfoPlusMacosPlugin.register(with: controller.registrar(forPlugin: "DeviceInfoPlusMacosPlugin"))
            FlutterCustomCursorPlugin.register(with: controller.registrar(forPlugin: "FlutterCustomCursorPlugin"))
            FLTPackageInfoPlusPlugin.register(with: controller.registrar(forPlugin: "FLTPackageInfoPlusPlugin"))
            PathProviderPlugin.register(with: controller.registrar(forPlugin: "PathProviderPlugin"))
            SqflitePlugin.register(with: controller.registrar(forPlugin: "SqflitePlugin"))
            TrayManagerPlugin.register(with: controller.registrar(forPlugin: "TrayManagerPlugin"))
            UniLinksDesktopPlugin.register(with: controller.registrar(forPlugin: "UniLinksDesktopPlugin"))
            UrlLauncherPlugin.register(with: controller.registrar(forPlugin: "UrlLauncherPlugin"))
            WakelockMacosPlugin.register(with: controller.registrar(forPlugin: "WakelockMacosPlugin"))
            WindowSizePlugin.register(with: controller.registrar(forPlugin: "WindowSizePlugin"))
        }
        
        super.awakeFromNib()
    }
    
//     override func bitsdojo_window_configure() -> UInt {
//         return BDW_CUSTOM_FRAME | BDW_HIDE_ON_STARTUP
//     }
}

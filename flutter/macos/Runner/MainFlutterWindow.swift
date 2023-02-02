import Cocoa
import FlutterMacOS
import desktop_multi_window
// import bitsdojo_window_macos

import desktop_drop
import device_info_plus_macos
import flutter_custom_cursor
import package_info_plus_macos
import path_provider_foundation
import screen_retriever
import sqflite
// import tray_manager
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
        // register self method handler
        let registrar = flutterViewController.registrar(forPlugin: "RustDeskPlugin")
        setMethodHandler(registrar: registrar)
        
        RegisterGeneratedPlugins(registry: flutterViewController)

        FlutterMultiWindowPlugin.setOnWindowCreatedCallback { controller in
            // Register the plugin which you want access from other isolate.
            // DesktopLifecyclePlugin.register(with: controller.registrar(forPlugin: "DesktopLifecyclePlugin"))
            self.setMethodHandler(registrar: controller.registrar(forPlugin: "RustDeskPlugin"))
            DesktopDropPlugin.register(with: controller.registrar(forPlugin: "DesktopDropPlugin"))
            DeviceInfoPlusMacosPlugin.register(with: controller.registrar(forPlugin: "DeviceInfoPlusMacosPlugin"))
            FlutterCustomCursorPlugin.register(with: controller.registrar(forPlugin: "FlutterCustomCursorPlugin"))
            FLTPackageInfoPlusPlugin.register(with: controller.registrar(forPlugin: "FLTPackageInfoPlusPlugin"))
            PathProviderPlugin.register(with: controller.registrar(forPlugin: "PathProviderPlugin"))
            SqflitePlugin.register(with: controller.registrar(forPlugin: "SqflitePlugin"))
            // TrayManagerPlugin.register(with: controller.registrar(forPlugin: "TrayManagerPlugin"))
            UniLinksDesktopPlugin.register(with: controller.registrar(forPlugin: "UniLinksDesktopPlugin"))
            UrlLauncherPlugin.register(with: controller.registrar(forPlugin: "UrlLauncherPlugin"))
            WakelockMacosPlugin.register(with: controller.registrar(forPlugin: "WakelockMacosPlugin"))
            WindowSizePlugin.register(with: controller.registrar(forPlugin: "WindowSizePlugin"))
        }
        
        super.awakeFromNib()
    }
    
    override public func order(_ place: NSWindow.OrderingMode, relativeTo otherWin: Int) {
        super.order(place, relativeTo: otherWin)
        hiddenWindowAtLaunch()
    }
    
    /// Override window theme.
    public func setWindowInterfaceMode(window: NSWindow, themeName: String) {
        window.appearance = NSAppearance(named: themeName == "light" ? .aqua : .darkAqua)
    }
    
    public func setMethodHandler(registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(name: "org.rustdesk.rustdesk/macos", binaryMessenger: registrar.messenger)
        channel.setMethodCallHandler({
            (call, result) -> Void in
                switch call.method {
                case "setWindowTheme":
                    let arg = call.arguments as! [String: Any]
                    let themeName = arg["themeName"] as? String
                    guard let window = registrar.view?.window else {
                        result(nil)
                        return
                    }
                    self.setWindowInterfaceMode(window: window,themeName: themeName ?? "light")
                    result(nil)
                    break;
                default:
                    result(FlutterMethodNotImplemented)
                }
        })
    }
}


import Cocoa
import AVFoundation
import FlutterMacOS
import desktop_multi_window
// import bitsdojo_window_macos

import desktop_drop
import device_info_plus
import flutter_custom_cursor
import package_info_plus
import path_provider_foundation
import screen_retriever
import sqflite
// import tray_manager
import uni_links_desktop
import url_launcher_macos
import wakelock_plus
import window_manager
import window_size
import texture_rgba_renderer

// Global state for relative mouse mode
// All properties and methods must be accessed on the main thread since they
// interact with NSEvent monitors, CoreGraphics APIs, and Flutter channels.
// Note: We avoid @MainActor to maintain macOS 10.14 compatibility.
class RelativeMouseState {
    static let shared = RelativeMouseState()

    var enabled = false
    var eventMonitor: Any?
    var deltaChannel: FlutterMethodChannel?
    var accumulatedDeltaX: CGFloat = 0
    var accumulatedDeltaY: CGFloat = 0

    private init() {}
}

class MainFlutterWindow: NSWindow {
    private static let fullscreenWorkAreaSizes = NSMapTable<NSWindow, NSValue>(
        keyOptions: [.weakMemory, .objectPointerPersonality],
        valueOptions: .strongMemory
    )
    private static let fullscreenObserver = NotificationCenter.default.addObserver(
        forName: NSWindow.willEnterFullScreenNotification,
        object: nil,
        queue: .main
    ) { notification in
        guard let window = notification.object as? NSWindow,
              let screen = window.screen else {
            return
        }
        fullscreenWorkAreaSizes.setObject(
            NSValue(size: screen.visibleFrame.size),
            forKey: window
        )
    }
    // Windows whose selected session enabled immersive mode.
    private static let immersiveWindows = NSHashTable<NSWindow>.weakObjects()
    // When a fullscreen window becomes key again, AppKit restores the options it
    // got at fullscreen entry, so re-apply a toggle made since then.
    private static let immersiveKeyObserver = NotificationCenter.default.addObserver(
        forName: NSWindow.didBecomeKeyNotification,
        object: nil,
        queue: .main
    ) { notification in
        guard let window = notification.object as? NSWindow else { return }
        updateImmersiveMode(window: window)
    }

    // AppKit applies a fullscreen window's presentation options itself after
    // didEnterFullScreen, overriding any set directly at that point, and asks
    // the window delegate for them. desktop_multi_window's delegate does not
    // implement this, so add it; other windows get the proposal back unchanged.
    private static func addFullScreenPresentationOptionsHook(to window: NSWindow) {
        guard let delegate = window.delegate,
              let delegateClass = object_getClass(delegate) else {
            return
        }
        let selector = #selector(NSWindowDelegate.window(_:willUseFullScreenPresentationOptions:))
        guard class_getInstanceMethod(delegateClass, selector) == nil else { return }
        let hook: @convention(block) (AnyObject, NSWindow, UInt) -> UInt = { _, window, proposed in
            guard immersiveWindows.contains(window) else { return proposed }
            return immersiveOptions(from: NSApplication.PresentationOptions(rawValue: proposed)).rawValue
        }
        class_addMethod(delegateClass, selector, imp_implementationWithBlock(hook), "Q@:@Q")
    }

    private static func immersiveOptions(
        from options: NSApplication.PresentationOptions
    ) -> NSApplication.PresentationOptions {
        var options = options
        // autoHideToolbar is only valid together with autoHideMenuBar.
        options.subtract([.autoHideDock, .autoHideMenuBar, .autoHideToolbar])
        options.formUnion([.hideDock, .hideMenuBar])
        return options
    }

    private static func updateImmersiveMode(window: NSWindow) {
        let current = NSApp.presentationOptions
        // AppKit raises on invalid combinations, so only adjust the presentation
        // of the key window once it is fullscreen.
        guard window.isKeyWindow,
              window.styleMask.contains(.fullScreen),
              current.contains(.fullScreen) else {
            return
        }
        if immersiveWindows.contains(window) {
            if !current.isSuperset(of: [.hideDock, .hideMenuBar]) {
                NSApp.presentationOptions = immersiveOptions(from: current)
            }
        } else if current.contains(.hideMenuBar) {
            var options = current
            options.subtract([.hideDock, .hideMenuBar])
            options.formUnion([.autoHideDock, .autoHideMenuBar])
            NSApp.presentationOptions = options
        }
    }

    override func awakeFromNib() {
        rustdesk_core_main();
        _ = MainFlutterWindow.fullscreenObserver
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
            // Note: copy below from above RegisterGeneratedPlugins
            self.setMethodHandler(registrar: controller.registrar(forPlugin: "RustDeskPlugin"))
            DesktopDropPlugin.register(with: controller.registrar(forPlugin: "DesktopDropPlugin"))
            DeviceInfoPlusMacosPlugin.register(with: controller.registrar(forPlugin: "DeviceInfoPlusMacosPlugin"))
            FlutterCustomCursorPlugin.register(with: controller.registrar(forPlugin: "FlutterCustomCursorPlugin"))
            FPPPackageInfoPlusPlugin.register(with: controller.registrar(forPlugin: "FPPPackageInfoPlusPlugin"))
            PathProviderPlugin.register(with: controller.registrar(forPlugin: "PathProviderPlugin"))
            SqflitePlugin.register(with: controller.registrar(forPlugin: "SqflitePlugin"))
            // TrayManagerPlugin.register(with: controller.registrar(forPlugin: "TrayManagerPlugin"))
            UniLinksDesktopPlugin.register(with: controller.registrar(forPlugin: "UniLinksDesktopPlugin"))
            UrlLauncherPlugin.register(with: controller.registrar(forPlugin: "UrlLauncherPlugin"))
            WakelockPlusMacosPlugin.register(with: controller.registrar(forPlugin: "WakelockPlusMacosPlugin"))
            WindowSizePlugin.register(with: controller.registrar(forPlugin: "WindowSizePlugin"))
            TextureRgbaRendererPlugin.register(with: controller.registrar(forPlugin: "TextureRgbaRendererPlugin"))
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

    private func enableNativeRelativeMouseMode(channel: FlutterMethodChannel) -> Bool {
        assert(Thread.isMainThread, "enableNativeRelativeMouseMode must be called on the main thread")
        let state = RelativeMouseState.shared
        if state.enabled {
            // Already enabled: update the channel so this caller receives deltas.
            state.deltaChannel = channel
            return true
        }

        // Dissociate mouse from cursor position - this locks the cursor in place
        // Do this FIRST before setting any state
        let result = CGAssociateMouseAndMouseCursorPosition(0)
        if result != CGError.success {
            NSLog("[RustDesk] Failed to dissociate mouse from cursor position: %d", result.rawValue)
            return false
        }

        // Only set state after CG call succeeds
        state.deltaChannel = channel
        state.accumulatedDeltaX = 0
        state.accumulatedDeltaY = 0

        // Add local event monitor to capture mouse delta.
        // Note: Local event monitors are always called on the main thread,
        // so accessing main-thread-only state is safe here.
        state.eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged]) { [weak state] event in
            guard let state = state else { return event }
            // Guard against race: mode may be disabled between weak capture and this check.
            guard state.enabled else { return event }
            let deltaX = event.deltaX
            let deltaY = event.deltaY

            if deltaX != 0 || deltaY != 0 {
                // Accumulate delta (main thread only - NSEvent local monitors always run on main thread)
                state.accumulatedDeltaX += deltaX
                state.accumulatedDeltaY += deltaY

                // Only send if we have integer movement
                let intX = Int(state.accumulatedDeltaX)
                let intY = Int(state.accumulatedDeltaY)

                if intX != 0 || intY != 0 {
                    state.accumulatedDeltaX -= CGFloat(intX)
                    state.accumulatedDeltaY -= CGFloat(intY)

                    // Send delta to Flutter (already on main thread)
                    state.deltaChannel?.invokeMethod("onMouseDelta", arguments: ["dx": intX, "dy": intY])
                }
            }

            return event
        }

        // Check if monitor was created successfully
        if state.eventMonitor == nil {
            NSLog("[RustDesk] Failed to create event monitor for relative mouse mode")
            // Re-associate mouse since we failed
            CGAssociateMouseAndMouseCursorPosition(1)
            state.deltaChannel = nil
            return false
        }

        // Set enabled LAST after everything succeeds
        state.enabled = true
        return true
    }

    private func disableNativeRelativeMouseMode() {
        assert(Thread.isMainThread, "disableNativeRelativeMouseMode must be called on the main thread")
        let state = RelativeMouseState.shared
        if !state.enabled { return }

        state.enabled = false

        // Remove event monitor
        if let monitor = state.eventMonitor {
            NSEvent.removeMonitor(monitor)
            state.eventMonitor = nil
        }

        state.deltaChannel = nil
        state.accumulatedDeltaX = 0
        state.accumulatedDeltaY = 0

        // Re-associate mouse with cursor position (non-blocking with async retry)
        let result = CGAssociateMouseAndMouseCursorPosition(1)
        if result != CGError.success {
            NSLog("[RustDesk] Failed to re-associate mouse with cursor position: %d, scheduling retry...", result.rawValue)
            // Non-blocking retry after 50ms
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                let retryResult = CGAssociateMouseAndMouseCursorPosition(1)
                if retryResult != CGError.success {
                    NSLog("[RustDesk] Retry failed to re-associate mouse: %d. Cursor may remain locked.", retryResult.rawValue)
                }
            }
        }
    }

    public func setMethodHandler(registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(name: "org.rustdesk.rustdesk/host", binaryMessenger: registrar.messenger)
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
                case "terminate":
                    NSApplication.shared.terminate(self)
                    result(nil)
                case "canRecordAudio":
                    switch AVCaptureDevice.authorizationStatus(for: .audio) {
                    case .authorized:
                        result(1)
                        break
                    case .notDetermined:
                        result(0)
                        break
                    default:
                        result(-1)
                        break
                    }
                case "requestRecordAudio":
                    AVCaptureDevice.requestAccess(for: .audio, completionHandler: { granted in
                        DispatchQueue.main.async {
                            result(granted)
                        }
                    })
                    break
                case "bumpMouse":
                    var dx = 0
                    var dy = 0

                    if let argMap = call.arguments as? [String: Any] {
                        dx = (argMap["dx"] as? Int) ?? 0
                        dy = (argMap["dy"] as? Int) ?? 0
                    }
                    else if let argList = call.arguments as? [Any] {
                        dx = argList.count >= 1 ? (argList[0] as? Int) ?? 0 : 0
                        dy = argList.count >= 2 ? (argList[1] as? Int) ?? 0 : 0
                    }

                    var mouseLoc: CGPoint

                    if let dummyEvent = CGEvent(source: nil) { // can this ever fail?
                        mouseLoc = dummyEvent.location
                    }
                    else if let screenFrame = NSScreen.screens.first?.frame {
                        // NeXTStep: Origin is lower-left of primary screen, positive is up
                        // Cocoa Core Graphics: Origin is upper-left of primary screen, positive is down
                        let nsMouseLoc = NSEvent.mouseLocation

                        mouseLoc = CGPoint(
                            x: nsMouseLoc.x,
                            y: NSHeight(screenFrame) - nsMouseLoc.y)
                    }
                    else {
                        result(false)
                        break
                    }

                    let newLoc = CGPoint(x: mouseLoc.x + CGFloat(dx), y: mouseLoc.y + CGFloat(dy))

                    CGDisplayMoveCursorToPoint(0, newLoc)

                    // By default, Cocoa suppresses mouse events briefly after a call to warp the
                    // cursor to a new location. This is good if you want to draw the user's
                    // attention to the fact that the mouse is now in a particular location, but
                    // it's bad in this case; we get called as part of the handling of edge
                    // scrolling, which means the mouse is typically still in motion, and we want
                    // the cursor to keep moving smoothly uninterrupted.
                    //
                    // This function's main action is to toggle whether the mouse cursor is
                    // associated with the mouse position, but setting it to true when it's
                    // already true has the side-effect of cancelling this motion suppression.
                    //
                    // However, we must NOT call this when relative mouse mode is active,
                    // as it would break the pointer lock established by enableNativeRelativeMouseMode.
                    if !RelativeMouseState.shared.enabled {
                        CGAssociateMouseAndMouseCursorPosition(1 /* true */)
                    }

                    result(true)

                case "enableNativeRelativeMouseMode":
                    let success = self.enableNativeRelativeMouseMode(channel: channel)
                    result(success)

                case "disableNativeRelativeMouseMode":
                    self.disableNativeRelativeMouseMode()
                    result(true)

                case "getMacOSWorkAreaSize":
                    guard Thread.isMainThread,
                          let window = registrar.view?.window,
                          let size = MainFlutterWindow.fullscreenWorkAreaSizes
                            .object(forKey: window)?.sizeValue else {
                        result(nil)
                        break
                    }
                    result([Double(size.width), Double(size.height)])

                case "setImmersiveMode":
                    guard let window = registrar.view?.window else {
                        result(nil)
                        break
                    }
                    let enabled = (call.arguments as? [String: Any])?["enabled"] as? Bool ?? false
                    if enabled {
                        _ = MainFlutterWindow.immersiveKeyObserver
                        MainFlutterWindow.addFullScreenPresentationOptionsHook(to: window)
                        MainFlutterWindow.immersiveWindows.add(window)
                    } else {
                        MainFlutterWindow.immersiveWindows.remove(window)
                    }
                    MainFlutterWindow.updateImmersiveMode(window: window)
                    result(nil)

                default:
                    result(FlutterMethodNotImplemented)
                }
        })
    }
}

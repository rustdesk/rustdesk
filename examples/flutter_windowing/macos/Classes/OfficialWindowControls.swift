import Cocoa
import FlutterMacOS

enum OfficialWindowControls {
  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "rustdesk/desktop_multi_window/macos",
      binaryMessenger: registrar.messenger
    )
    channel.setMethodCallHandler { call, result in
      guard let args = call.arguments as? [String: Any],
            let handle = (args["handle"] as? NSNumber)?.uint64Value,
            let window = NSApplication.shared.windows.first(where: {
              UInt64(UInt(bitPattern: Unmanaged.passUnretained($0).toOpaque())) == handle
            }) else {
        result(FlutterError(code: "invalid_window", message: "Expected a live NSWindow", details: nil))
        return
      }
      switch call.method {
      case "show", "focus":
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
      case "hide":
        window.orderOut(nil)
      case "isHidden":
        result(!window.isVisible)
        return
      case "getFrame":
        let frame = window.frame
        let desktopTop = NSScreen.screens.first?.frame.maxY ?? 0
        result(["x": frame.minX, "y": desktopTop - frame.maxY,
                "width": frame.width, "height": frame.height])
        return
      case "setFrame":
        guard let x = (args["x"] as? NSNumber)?.doubleValue,
              let y = (args["y"] as? NSNumber)?.doubleValue,
              let width = (args["width"] as? NSNumber)?.doubleValue,
              let height = (args["height"] as? NSNumber)?.doubleValue,
              width > 0, height > 0 else {
          result(FlutterError(code: "invalid_frame", message: "Expected valid window bounds", details: nil))
          return
        }
        let desktopTop = NSScreen.screens.first?.frame.maxY ?? 0
        let frame = NSRect(x: CGFloat(x), y: desktopTop - CGFloat(y + height),
                           width: CGFloat(width), height: CGFloat(height))
        window.setFrame(frame, display: false)
      case "center":
        window.center()
      case "showTitleBar":
        let show = (args["show"] as? NSNumber)?.intValue == 1
        window.titleVisibility = show ? .visible : .hidden
        window.titlebarAppearsTransparent = !show
      default:
        result(FlutterMethodNotImplemented)
        return
      }
      result(nil)
    }
  }
}

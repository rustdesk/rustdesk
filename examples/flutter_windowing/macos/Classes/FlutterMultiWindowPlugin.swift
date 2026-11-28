import Cocoa
import FlutterMacOS

public class FlutterMultiWindowPlugin: NSObject, FlutterPlugin {
  public static var RegisterGeneratedPlugins: ((FlutterPluginRegistry) -> Void)?

  static func registerInternal(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(name: "mixin.one/flutter_multi_window", binaryMessenger: registrar.messenger)
    let instance = FlutterMultiWindowPlugin()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public static func register(with registrar: FlutterPluginRegistrar) {
    registerInternal(with: registrar)
    OfficialWindowControls.register(with: registrar)
    guard let app = NSApplication.shared.delegate as? FlutterAppDelegate else {
      debugPrint("failed to find flutter main window, application delegate is not FlutterAppDelegate")
      return
    }
    guard let window = app.mainFlutterWindow else {
      debugPrint("failed to find flutter main window")
      return
    }
    let mainWindowChannel = WindowChannel.register(with: registrar, windowId: 0)
    MultiWindowManager.shared.attachMainWindow(window: window, mainWindowChannel)
  }

  public typealias OnWindowCreatedCallback = (FlutterViewController) -> Void
  static var onWindowCreatedCallback: OnWindowCreatedCallback?

  public static func setOnWindowCreatedCallback(_ callback: @escaping OnWindowCreatedCallback) {
    onWindowCreatedCallback = callback
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "createWindow":
      let arguments = call.arguments as? String
      let windowId = MultiWindowManager.shared.create(arguments: arguments ?? "")
      result(windowId)
    case "show":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.show(windowId: windowId)
      result(nil)
    case "hide":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.hide(windowId: windowId)
      result(nil)
    case "isHidden":
      let windowId = call.arguments as! Int64
      let res = MultiWindowManager.shared.isHidden(windowId: windowId)
      result(res)
    case "close":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.close(windowId: windowId)
      result(nil)
    case "center":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.center(windowId: windowId)
      result(nil)
    case "setFrame":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let left = arguments["left"] as! Double
      let top = arguments["top"] as! Double
      let width = arguments["width"] as! Double
      let height = arguments["height"] as! Double
      var rect = NSRect(x: left, y: top, width: width, height: height)
      // fix: convert from origin coordinate to topLeft one
      rect.topLeft.x = rect.origin.x
      rect.topLeft.y = rect.origin.y
      MultiWindowManager.shared.setFrame(windowId: windowId, frame: rect)
      result(nil)
    case "getFrame":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      result(MultiWindowManager.shared.getFrame(windowId: windowId))
    case "setTitle":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let title = arguments["title"] as! String
      MultiWindowManager.shared.setTitle(windowId: windowId, title: title)
      result(nil)
    case "setFrameAutosaveName":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let frameAutosaveName = arguments["name"] as! String
      MultiWindowManager.shared.setFrameAutosaveName(windowId: windowId, name: frameAutosaveName)
      result(nil)
    case "getAllSubWindowIds":
      let subWindowIds = MultiWindowManager.shared.getAllSubWindowIds()
      result(subWindowIds)
    case "focus":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.focus(windowId: windowId)
      result(nil)
    case "minimize":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.minimize(windowId: windowId)
      result(nil)
    case "maximize":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.maximize(windowId: windowId)
      result(nil)
    case "unmaximize":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.unmaximize(windowId: windowId)
      result(nil)
    case "isMaximized":
      let windowId = call.arguments as! Int64
      let res = MultiWindowManager.shared.isMaximized(windowId: windowId)
      result(res)
    case "isMinimized":
      let windowId = call.arguments as! Int64
      let res = MultiWindowManager.shared.isMinimized(windowId: windowId)
      result(res)
    case "startDragging":
      let windowId = call.arguments as! Int64
      MultiWindowManager.shared.startDragging(windowId: windowId)
      result(nil)
    case "setMovable":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let isMovable = arguments["isMovable"] as! Bool
      MultiWindowManager.shared.setMovable(windowId: windowId, isMovable: isMovable)
      result(nil)
    case "showTitleBar":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let showTitleBar = arguments["show"] as! Bool
      MultiWindowManager.shared.showTitleBar(windowId: windowId, show: showTitleBar)
      result(nil)
    case "isFullScreen":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let isFullScreen = MultiWindowManager.shared.isFullScreen(windowId: windowId)
      result(isFullScreen)
    case "setFullscreen":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let fullscreen = arguments["fullscreen"] as! Bool
      MultiWindowManager.shared.setFullscreen(windowId: windowId, fullscreen: fullscreen)
      result(nil)
    case "startResizing":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      MultiWindowManager.shared.startResizing(windowId: windowId, arguments: arguments)
      result(nil)
    case "isPreventClose":
      let windowId = call.arguments as! Int64
      let res = MultiWindowManager.shared.isPreventClose(windowId: windowId)
      result(res)
    case "setPreventClose":
      let arguments = call.arguments as! [String: Any?]
      let windowId = arguments["windowId"] as! Int64
      let setPreventClose = arguments["setPreventClose"] as! Bool
      MultiWindowManager.shared.setPreventClose(windowId: windowId, setPreventClose: setPreventClose)
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }
}

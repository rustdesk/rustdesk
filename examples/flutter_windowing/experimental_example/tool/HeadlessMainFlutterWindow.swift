import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  private let engine = FlutterEngine(name: "windowing_smoke", project: nil)

  override func awakeFromNib() {
    super.awakeFromNib()
    // The SDK enables multiview when it creates the first window. Leave the
    // nib's window without a Flutter view controller so the engine can opt in.
    guard engine.run(withEntrypoint: nil) else {
      fatalError("Could not start the Flutter windowing smoke test")
    }
    RegisterGeneratedPlugins(registry: engine)
  }
}

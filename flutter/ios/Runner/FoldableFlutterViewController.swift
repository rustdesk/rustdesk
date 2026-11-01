import Flutter
import UIKit

/// Supplies view-local reserved regions until Flutter exposes them on iOS.
/// Older SDKs and phones keep the existing mobile session layout.
class FoldableFlutterViewController: FlutterViewController, FlutterStreamHandler {
  private var layoutChannel: FlutterEventChannel?
  private var layoutSink: FlutterEventSink?
  private var lastLayout: NSDictionary?
  private var hingeAvailable = false

  override func viewDidLoad() {
    super.viewDidLoad()
    let channel = FlutterEventChannel(
      name: "rustdesk/foldable_layout", binaryMessenger: binaryMessenger)
    channel.setStreamHandler(self)
    layoutChannel = channel
    #if compiler(>=6.4)
    if #available(iOS 27.1, *) {
      view.addInteraction(UIHingeInteraction { [weak self] _, update in
        guard let self, self.view.window != nil else { return }
        self.hingeAvailable = update.hinge != nil
        self.view.setNeedsLayout()
      })
    }
    #endif
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    publishLayout()
  }

  override func viewSafeAreaInsetsDidChange() {
    super.viewSafeAreaInsetsDidChange()
    view.setNeedsLayout()
  }

  func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
    layoutSink = events
    lastLayout = nil
    publishLayout()
    return nil
  }

  func onCancel(withArguments arguments: Any?) -> FlutterError? {
    layoutSink = nil
    lastLayout = nil
    return nil
  }

  private func publishLayout() {
    guard let sink = layoutSink, view.window != nil else { return }
    var supported = false
    var divisions: [[CGFloat]] = []
    #if compiler(>=6.4)
    if #available(iOS 27.1, *) {
      let regions = view.reservedRegions(kind: .division, options: .includeInactive)
      supported = hingeAvailable || !regions.isEmpty
      divisions = regions.filter(\.isActive).map {
        [$0.frame.minX, $0.frame.minY, $0.frame.width, $0.frame.height]
      }
    }
    #endif
    let layout: NSDictionary = [
      "supported": supported,
      "width": view.bounds.width,
      "height": view.bounds.height,
      "divisions": divisions,
    ]
    guard layout != lastLayout else { return }
    lastLayout = layout
    sink(layout)
  }
}

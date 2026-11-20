import Flutter
import UIKit

final class IosWindowGeometryStream: NSObject, FlutterStreamHandler {
    private weak var rootView: UIView?
    private var geometryView: IosWindowGeometryView?
    private var sink: FlutterEventSink?

    init(messenger: FlutterBinaryMessenger) {
        super.init()
        FlutterEventChannel(name: "rustdesk/ios_window_geometry", binaryMessenger: messenger)
            .setStreamHandler(self)
    }

    func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        sink = events
        installObserver()
        return nil
    }

    func attach(view: UIView) {
        geometryView?.removeFromSuperview()
        geometryView = nil
        rootView = view
        installObserver()
    }

    func refresh() {
        geometryView?.publish()
    }

    private func installObserver() {
        guard geometryView == nil, let rootView, let sink else { return }
        let observer = IosWindowGeometryView(frame: rootView.bounds, sink: sink)
        observer.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        rootView.addSubview(observer)
        geometryView = observer
        observer.publish()
    }

    func onCancel(withArguments arguments: Any?) -> FlutterError? {
        geometryView?.removeFromSuperview()
        geometryView = nil
        sink = nil
        return nil
    }
}

private final class IosWindowGeometryView: UIView {
    private let sink: FlutterEventSink
    private var previous: NSDictionary?
    private var hingeStatus = "unavailable"
    private var keyboardScreenFrame: CGRect?

    init(frame: CGRect, sink: @escaping FlutterEventSink) {
        self.sink = sink
        super.init(frame: frame)
        isUserInteractionEnabled = false
        backgroundColor = .clear
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)),
                                               name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardHidden(_:)),
                                               name: UIResponder.keyboardDidHideNotification, object: nil)
        if #available(iOS 27.1, *) {
            addInteraction(UIHingeInteraction { [weak self] _, update in
                guard let self else { return }
                if let hinge = update.hinge {
                    switch hinge.status {
                    case .closed: hingeStatus = "closed"
                    case .partiallyOpen: hingeStatus = "partiallyOpen"
                    case .fullyOpen: hingeStatus = "fullyOpen"
                    default: hingeStatus = "unknown"
                    }
                } else {
                    hingeStatus = "unavailable"
                }
                publish()
            })
        }
    }

    required init?(coder: NSCoder) { return nil }

    deinit { NotificationCenter.default.removeObserver(self) }

    override func layoutSubviews() {
        super.layoutSubviews()
        publish()
    }

    override func safeAreaInsetsDidChange() {
        super.safeAreaInsetsDidChange()
        publish()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        publish()
    }

    @objc private func keyboardChanged(_ notification: Notification) {
        if let local = notification.userInfo?[UIResponder.keyboardIsLocalUserInfoKey] as? Bool, !local { return }
        keyboardScreenFrame = (notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
        publish()
    }

    @objc private func keyboardHidden(_ notification: Notification) {
        keyboardScreenFrame = nil
        publish()
    }

    private func rectangle(_ rect: CGRect) -> [String: Double] {
        ["x": Double(rect.minX), "y": Double(rect.minY),
         "width": Double(rect.width), "height": Double(rect.height)]
    }

    func publish() {
        guard window != nil, bounds.width > 0, bounds.height > 0 else { return }
        var divisions: [[String: Any]] = []
        if #available(iOS 27.1, *) {
            divisions = reservedRegions(kind: .division).filter(\.isActive).map {
                ["frame": rectangle($0.frame),
                 "margins": ["top": Double($0.margins.top), "left": Double($0.margins.left),
                             "bottom": Double($0.margins.bottom), "right": Double($0.margins.right)]]
            }
        }
        var keyboard: Any = NSNull()
        if let screenFrame = keyboardScreenFrame, let screen = window?.screen {
            let visible = convert(screenFrame, from: screen.coordinateSpace).intersection(bounds)
            if !visible.isNull && visible.height > 0 { keyboard = rectangle(visible) }
        }
        let payload: NSDictionary = [
            "bounds": rectangle(bounds), "hingeStatus": hingeStatus,
            "divisions": divisions, "keyboard": keyboard
        ]
        guard previous?.isEqual(payload) != true else { return }
        previous = payload
        sink(payload)
    }
}

import UIKit
import Flutter
import ObjectiveC

/// Flutter 3.24.5 reads the engine shell during viewDidLoad on displays faster
/// than 60 Hz. On iOS 26 that pointer is still null, so the lookup faults.
private func disableTouchRateVSyncCrash() {
  guard let method = class_getInstanceMethod(
    FlutterViewController.self,
    NSSelectorFromString("createTouchRateCorrectionVSyncClientIfNeeded")
  ) else {
    return
  }
  let block: @convention(block) (AnyObject) -> Void = { _ in }
  method_setImplementation(method, imp_implementationWithBlock(block))
}

/// Flutter's iPhone text input view stays at an empty rect, so iOS leaves
/// 简体中文手写 out of the globe menu. A real UITextView owns the keyboard.
/// Handwriting commits go straight to the remote session. Other IMEs still
/// use the Flutter text input view.
private final class HandwritingTextView: UITextView, UITextViewDelegate {
  weak var flutterInput: NSObject?
  var suppressPublish = false
  var handledBySession = false
  var tracked = ""

  /// iOS 26 hides 简体中文手写 unless the focused view answers this with false.
  @objc func disableHandwritingKeyboard() -> Bool {
    false
  }

  override func insertText(_ text: String) {
    if isHandwritingMode() {
      HandwritingBridge.shared.prepareForHandwritingCommit()
    }
    super.insertText(text)
    publishTextChange(reason: "insert")
    handledBySession = false
  }

  /// 简体中文手写提交候选字时走这个方法，不会再调用 insertText:。
  @objc(insertText:alternatives:style:)
  func insertText(_ text: String, alternatives: NSArray?, style: Int) {
    handledBySession = true
    HandwritingBridge.shared.prepareForHandwritingCommit()
    invokeTextViewInsert(text, alternatives: alternatives, style: style)
    publishTextChange(reason: "alternatives")
    handledBySession = false
  }

  override func deleteBackward() {
    super.deleteBackward()
    publishTextChange(reason: "delete")
  }

  func textViewDidChange(_ textView: UITextView) {
    publishTextChange(reason: "delegate")
  }

  func publishTextChange(reason _: String) {
    let now = text ?? ""
    tracked = now
    if suppressPublish { return }
    let handwriting = isHandwritingMode()
    // 拼音组字时文字还带标记，先留在手机上，等确认成汉字再发送。
    if !handwriting && markedTextRange != nil { return }
    HandwritingBridge.shared.noteLocalText(now, coalesce: handwriting)
  }

  override func caretRect(for position: UITextPosition) -> CGRect {
    .zero
  }

  func resetTrackedText() {
    suppressPublish = true
    text = ""
    tracked = ""
    suppressPublish = false
  }

  private func invokeTextViewInsert(_ text: String, alternatives: NSArray?, style: Int) {
    let sel = NSSelectorFromString("insertText:alternatives:style:")
    guard let method = class_getInstanceMethod(UITextView.self, sel) else { return }
    typealias Insert = @convention(c) (AnyObject, Selector, NSString, NSArray?, Int) -> Void
    let fn = unsafeBitCast(method_getImplementation(method), to: Insert.self)
    fn(self, sel, text as NSString, alternatives, style)
  }

  func isHandwritingMode() -> Bool {
    guard let mode = textInputMode ?? UIApplication.shared.textInputMode else { return false }
    let object = mode as NSObject
    for name in ["identifier", "normalizedIdentifier"] {
      let sel = NSSelectorFromString(name)
      guard object.responds(to: sel),
            let id = object.perform(sel)?.takeUnretainedValue() as? String else {
        continue
      }
      if id.contains("HWR") { return true }
    }
    return false
  }

  private func forwardInserted(_ text: String) {
    guard !text.isEmpty, let target = flutterInput else { return }
    let sel = NSSelectorFromString("insertText:")
    guard target.responds(to: sel) else { return }
    typealias Insert = @convention(c) (AnyObject, Selector, NSString) -> Void
    let fn = unsafeBitCast(target.method(for: sel), to: Insert.self)
    fn(target, sel, text as NSString)
  }

  private func forwardDeleted() {
    guard let target = flutterInput else { return }
    let sel = NSSelectorFromString("deleteBackward")
    guard target.responds(to: sel) else { return }
    typealias Delete = @convention(c) (AnyObject, Selector) -> Void
    let fn = unsafeBitCast(target.method(for: sel), to: Delete.self)
    fn(target, sel)
  }

  func textDelta(from old: String, to new: String) -> (removed: Int, inserted: String) {
    let previous = Array(old)
    let current = Array(new)
    var prefix = 0
    while prefix < previous.count && prefix < current.count && previous[prefix] == current[prefix] {
      prefix += 1
    }
    var suffix = 0
    while suffix < previous.count - prefix && suffix < current.count - prefix
            && previous[previous.count - 1 - suffix] == current[current.count - 1 - suffix] {
      suffix += 1
    }
    let removed = previous.count - prefix - suffix
    let inserted = String(current[prefix..<(current.count - suffix)])
    return (removed, inserted)
  }
}

private final class HandwritingBridge {
  static let shared = HandwritingBridge()
  let field: HandwritingTextView
  weak var captured: UIResponder?
  var channel: FlutterMethodChannel?

  private init() {
    field = HandwritingTextView(frame: CGRect(x: 0, y: 0, width: 280, height: 44))
    field.autocorrectionType = .yes
    field.spellCheckingType = .yes
    field.keyboardType = .default
    field.autocapitalizationType = .none
    if #available(iOS 17.0, *) {
      field.inlinePredictionType = .yes
    }
    field.smartQuotesType = .yes
    field.smartDashesType = .yes
    field.isEditable = true
    field.isSelectable = true
    field.backgroundColor = .clear
    field.textColor = .clear
    field.tintColor = .clear
    field.font = UIFont.systemFont(ofSize: 17)
    field.delegate = field
    NotificationCenter.default.addObserver(
      forName: UITextInputMode.currentInputModeDidChangeNotification,
      object: nil,
      queue: .main
    ) { _ in
      HandwritingBridge.shared.keepHandwritingModeIfReverted()
    }
  }

  private var activating = false
  private var focusTicket = 0
  var remoteKeyboardArmed = false
  private var restoringInputMode = false
  private var retainedHandwritingMode: NSObject?
  private var lastHandwritingEdit = Date.distantPast

  func setRemoteKeyboardArmed(_ armed: Bool) {
    remoteKeyboardArmed = armed
    if !armed {
      hide()
    }
  }

  func show(flutterInput: NSObject) -> Bool {
    guard let window = hostWindow() else {
      NSLog("RustDesk handwriting: no window")
      return false
    }
    focusTicket += 1
    field.flutterInput = flutterInput
    allowHandwriting(field)
    if let traits = field.perform(NSSelectorFromString("textInputTraits"))?.takeUnretainedValue() as? NSObject {
      allowHandwriting(traits)
    }
    if field.isFirstResponder {
      return true
    }
    activating = true
    defer { activating = false }
    cancelFlush()
    sent = ""
    latest = ""
    field.resetTrackedText()
    field.frame = CGRect(x: 16, y: window.safeAreaInsets.top + 8, width: 280, height: 44)
    if field.superview !== window {
      window.addSubview(field)
    }
    window.makeKeyAndVisible()
    let ok = field.becomeFirstResponder()
    if let traits = field.perform(NSSelectorFromString("textInputTraits"))?.takeUnretainedValue() as? NSObject {
      allowHandwriting(traits)
    }
    field.reloadInputViews()
    NSLog("RustDesk handwriting bridge shown=%d", ok)
    return ok
  }

  func noteKeyboard() {
    captured = nil
    UIApplication.shared.sendAction(#selector(UIResponder.rustdeskCapture(_:)), to: nil, from: nil, for: nil)
    let responder = captured
    let view = responder as? UIView
    let traits = responder as? UITextInputTraits
    let auto = traits?.autocorrectionType?.rawValue ?? -1
    let keyboard = traits?.keyboardType?.rawValue ?? -1
    let modes = switchToHandwritingMode()
    let saved = savedKeyboardIDs().joined(separator: "|")
    let line = "first=\(responder.map { String(describing: type(of: $0)) } ?? "nil") frame=\(view?.frame.debugDescription ?? "-") auto=\(auto) kb=\(keyboard) hw=\(handwritingDisabledFlag()) \(modes) saved=\(saved)"
    NSLog("RustDesk handwriting %@", line)
    appendHwLog(line)
  }

  private func switchToHandwritingMode() -> String {
    guard let cls = NSClassFromString("UIKeyboardInputModeController") else {
      return "modes=no-controller"
    }
    let shared = NSSelectorFromString("sharedInputModeController")
    guard let method = class_getClassMethod(cls, shared) else {
      return "modes=no-shared"
    }
    typealias Shared = @convention(c) (AnyClass, Selector) -> Unmanaged<NSObject>?
    let sharedFn = unsafeBitCast(method_getImplementation(method), to: Shared.self)
    guard let controller = sharedFn(cls, shared)?.takeUnretainedValue() else {
      return "modes=nil"
    }
    let idSel = NSSelectorFromString("enabledInputModeIdentifiers")
    guard controller.responds(to: idSel) else {
      return "modes=unavailable"
    }
    let identifiers = controller.perform(idSel)?.takeUnretainedValue()
    let idList = (identifiers as? [String]) ?? (identifiers as? NSArray)?.compactMap { $0 as? String } ?? []
    let joined = idList.joined(separator: "|")
    guard let chosenID = idList.first(where: { $0.contains("Hans") && $0.contains("HWR") })
            ?? idList.first(where: { $0.contains("HWR") }) else {
      return "modes=\(joined) switched=none selectable=\(selectableModeIDs(controller))"
    }
    let lookup = NSSelectorFromString("keyboardInputModeWithIdentifier:")
    guard controller.responds(to: lookup),
          let mode = controller.perform(lookup, with: chosenID)?.takeUnretainedValue(),
          !(mode is NSString) else {
      return "modes=\(joined) switched=no-mode \(chosenID)"
    }
    let setSel = NSSelectorFromString("setCurrentInputMode:")
    guard controller.responds(to: setSel) else {
      return "modes=\(joined) switched=no-setter \(chosenID)"
    }
    controller.perform(setSel, with: mode)
    return "modes=\(joined) switched=\(chosenID) selectable=\(selectableModeIDs(controller))"
  }

  private func handwritingDisabledFlag() -> Int {
    let traitsSel = NSSelectorFromString("textInputTraits")
    let flagSel = NSSelectorFromString("disableHandwritingKeyboard")
    guard field.responds(to: traitsSel),
          let traits = field.perform(traitsSel)?.takeUnretainedValue() as? NSObject,
          traits.responds(to: flagSel) else {
      return -1
    }
    typealias Flag = @convention(c) (AnyObject, Selector) -> Bool
    let fn = unsafeBitCast(traits.method(for: flagSel), to: Flag.self)
    return fn(traits, flagSel) ? 1 : 0
  }

  private func selectableModeIDs(_ controller: NSObject) -> String {
    let sel = NSSelectorFromString("keyboardInputModes")
    guard controller.responds(to: sel) else { return "no-modes" }
    return describeModeIDs(controller.perform(sel)?.takeUnretainedValue())
  }

  func hide() {
    if activating {
      return
    }
    focusTicket += 1
    let ticket = focusTicket
    DispatchQueue.main.async {
      guard ticket == self.focusTicket else { return }
      self.dismissField()
    }
  }

  private func dismissField() {
    flushEditing()
    field.suppressPublish = true
    field.resignFirstResponder()
    field.removeFromSuperview()
    field.flutterInput = nil
    field.resetTrackedText()
    sent = ""
    latest = ""
    field.suppressPublish = false
  }

  func prepareForHandwritingCommit() {
    guard field.isFirstResponder else { return }
    allowHandwriting(field)
    if let traits = field.perform(NSSelectorFromString("textInputTraits"))?.takeUnretainedValue() as? NSObject {
      allowHandwriting(traits)
    }
    if let mode = currentKeyboardMode() {
      let identifier = modeIdentifier(mode)
      if identifier.contains("HWR") {
        retainedHandwritingMode = mode
      }
    }
    lastHandwritingEdit = Date()
  }

  func keepHandwritingModeIfReverted() {
    if restoringInputMode || !field.isFirstResponder {
      return
    }
    guard let mode = currentKeyboardMode() else { return }
    let identifier = modeIdentifier(mode)
    if identifier.contains("HWR") {
      retainedHandwritingMode = mode
      return
    }
    guard Date().timeIntervalSince(lastHandwritingEdit) < 0.4,
          retainedHandwritingMode != nil else {
      return
    }
    DispatchQueue.main.async {
      guard Date().timeIntervalSince(self.lastHandwritingEdit) < 0.4,
            self.field.isFirstResponder,
            let retained = self.retainedHandwritingMode,
            let mode = self.currentKeyboardMode(),
            !self.modeIdentifier(mode).contains("HWR") else {
        return
      }
      self.restoringInputMode = true
      self.setKeyboardMode(retained)
      self.restoringInputMode = false
    }
  }

  /// 手写会先插入临时字，再连续改判。这里把这一串合成一次发送。
  private var sent = ""
  private var latest = ""
  private var flushItem: DispatchWorkItem?

  func noteLocalText(_ text: String, coalesce: Bool) {
    if text == latest && flushItem == nil { return }
    latest = text
    flushItem?.cancel()
    flushItem = nil
    if !coalesce {
      flushEditing()
      return
    }
    let item = DispatchWorkItem { [weak self] in
      self?.flushEditing()
    }
    flushItem = item
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: item)
  }

  func flushEditing() {
    flushItem?.cancel()
    flushItem = nil
    let delta = field.textDelta(from: sent, to: latest)
    guard delta.removed > 0 || !delta.inserted.isEmpty else { return }
    guard let channel else { return }
    sent = latest
    channel.invokeMethod("edit", arguments: ["removed": delta.removed, "text": delta.inserted])
  }

  private func cancelFlush() {
    flushItem?.cancel()
    flushItem = nil
  }

  private func currentKeyboardMode() -> NSObject? {
    guard let controller = sharedInputModeController() else { return nil }
    let sel = NSSelectorFromString("currentInputMode")
    guard controller.responds(to: sel),
          let mode = controller.perform(sel)?.takeUnretainedValue() as? NSObject,
          !(mode is NSString) else {
      return nil
    }
    return mode
  }

  private func setKeyboardMode(_ mode: NSObject) {
    guard let controller = sharedInputModeController() else { return }
    let sel = NSSelectorFromString("setCurrentInputMode:")
    guard controller.responds(to: sel) else { return }
    controller.perform(sel, with: mode)
  }

  private func modeIdentifier(_ mode: NSObject) -> String {
    if let text = mode as? String { return text }
    let sel = NSSelectorFromString("identifier")
    guard mode.responds(to: sel),
          let identifier = mode.perform(sel)?.takeUnretainedValue() as? String else {
      return ""
    }
    return identifier
  }

  private func sharedInputModeController() -> NSObject? {
    guard let cls = NSClassFromString("UIKeyboardInputModeController") else { return nil }
    let sel = NSSelectorFromString("sharedInputModeController")
    guard let method = class_getClassMethod(cls, sel) else { return nil }
    typealias Shared = @convention(c) (AnyClass, Selector) -> Unmanaged<NSObject>?
    let fn = unsafeBitCast(method_getImplementation(method), to: Shared.self)
    return fn(cls, sel)?.takeUnretainedValue()
  }

  func allowHandwriting(_ object: NSObject) {
    let setter = NSSelectorFromString("setDisableHandwritingKeyboard:")
    guard object.responds(to: setter) else {
      return
    }
    typealias SetFlag = @convention(c) (AnyObject, Selector, Bool) -> Void
    let fn = unsafeBitCast(object.method(for: setter), to: SetFlag.self)
    fn(object, setter, false)
  }
}

extension UIResponder {
  @objc func rustdeskCapture(_ sender: Any) {
    HandwritingBridge.shared.captured = self
  }
}

private var expandingKeyboardModes = false

private func describeModeIDs(_ value: Any?) -> String {
  guard let array = value as? NSArray else { return "-" }
  let ids: [String] = array.compactMap { item in
    if let text = item as? String { return text }
    guard let object = item as? NSObject else { return nil }
    let sel = NSSelectorFromString("identifier")
    guard object.responds(to: sel) else { return nil }
    return object.perform(sel)?.takeUnretainedValue() as? String
  }
  return ids.joined(separator: "|")
}

private func savedKeyboardIDs() -> [String] {
  let keys = ["AppleKeyboardsExpanded", "AppleKeyboards"]
  var ids: [String] = []
  for key in keys {
    if let values = UserDefaults.standard.array(forKey: key) as? [String] {
      ids.append(contentsOf: values)
    }
    let raw = CFPreferencesCopyAppValue(key as CFString, kCFPreferencesAnyApplication)
    if let values = raw as? [String] {
      ids.append(contentsOf: values)
    } else if let values = raw as? NSArray {
      ids.append(contentsOf: values.compactMap { $0 as? String })
    }
  }
  var seen = Set<String>()
  return ids.filter { seen.insert($0).inserted }
}

private func preferredInstalledIdentifiers() -> [String] {
  let saved = savedKeyboardIDs()
  var identifiers: [String] = []
  if let handwriting = saved.first(where: { $0.contains("HWR") }) {
    identifiers.append(handwriting)
  } else {
    identifiers.append("zh_Hans-HWR@sw=HWR-Simplified")
  }
  if let grid = saved.first(where: { $0.contains("Pinyin10") }) {
    identifiers.append(grid)
  }
  return identifiers
}

private func withInstalledKeyboardIdentifiers(_ current: NSArray) -> NSArray {
  let existing = current.compactMap { $0 as? String }
  var extra: [String] = []
  for identifier in preferredInstalledIdentifiers() {
    let already = existing.contains { id in
      if identifier.contains("HWR") { return id.contains("HWR") }
      if identifier.contains("Pinyin10") { return id.contains("Pinyin10") }
      return id == identifier
    }
    if !already { extra.append(identifier) }
  }
  guard !extra.isEmpty else { return current }
  let merged = NSMutableArray(array: current)
  extra.forEach { merged.add($0) }
  return merged
}

private var expandingEnabledIdentifiers = false

private func keepEnabledChineseKeyboards() {
  guard let cls = NSClassFromString("UIKeyboardInputModeController") else { return }
  let listed = NSSelectorFromString("enabledInputModeIdentifiers")
  if let method = class_getInstanceMethod(cls, listed) {
    let original = method_getImplementation(method)
    let block: @convention(block) (AnyObject) -> NSArray = { object in
      typealias Original = @convention(c) (AnyObject, Selector) -> NSArray
      let fn = unsafeBitCast(original, to: Original.self)
      if expandingEnabledIdentifiers { return fn(object, listed) }
      expandingEnabledIdentifiers = true
      defer { expandingEnabledIdentifiers = false }
      return withInstalledKeyboardIdentifiers(fn(object, listed))
    }
    method_setImplementation(method, imp_implementationWithBlock(block))
  }
  let listedFlag = NSSelectorFromString("enabledInputModeIdentifiers:")
  if let method = class_getInstanceMethod(cls, listedFlag) {
    let original = method_getImplementation(method)
    let block: @convention(block) (AnyObject, Bool) -> NSArray = { object, includeAll in
      typealias Original = @convention(c) (AnyObject, Selector, Bool) -> NSArray
      let fn = unsafeBitCast(original, to: Original.self)
      if expandingEnabledIdentifiers { return fn(object, listedFlag, includeAll) }
      expandingEnabledIdentifiers = true
      defer { expandingEnabledIdentifiers = false }
      return withInstalledKeyboardIdentifiers(fn(object, listedFlag, includeAll))
    }
    method_setImplementation(method, imp_implementationWithBlock(block))
  }
}

private func keepHandwritingTraitEnabled() {
  guard let cls = NSClassFromString("UITextInputTraits") else { return }
  let sel = NSSelectorFromString("setDisableHandwritingKeyboard:")
  guard let method = class_getInstanceMethod(cls, sel) else { return }
  let original = method_getImplementation(method)
  let block: @convention(block) (AnyObject, Bool) -> Void = { object, disabled in
    typealias Original = @convention(c) (AnyObject, Selector, Bool) -> Void
    let fn = unsafeBitCast(original, to: Original.self)
    let writing = HandwritingBridge.shared.field.isFirstResponder
    fn(object, sel, writing ? false : disabled)
  }
  method_setImplementation(method, imp_implementationWithBlock(block))
}

private func inputMode(identifier: String) -> NSObject? {
  guard let modeClass = NSClassFromString("UIKeyboardInputMode") else { return nil }
  let sel = NSSelectorFromString("keyboardInputModeWithIdentifier:")
  guard (modeClass as AnyObject).responds(to: sel),
        let mode = (modeClass as AnyObject).perform(sel, with: identifier)?.takeUnretainedValue() as? NSObject,
        !(mode is NSString) else {
    return nil
  }
  return mode
}

private func keepInstalledChineseKeyboards() {
  guard let cls = NSClassFromString("UIKeyboardInputModeController") else { return }
  let sel = NSSelectorFromString("keyboardInputModes")
  guard let method = class_getInstanceMethod(cls, sel) else {
    NSLog("RustDesk handwriting: keyboardInputModes missing")
    return
  }
  let original = method_getImplementation(method)
  let block: @convention(block) (AnyObject) -> NSArray = { object in
    typealias Original = @convention(c) (AnyObject, Selector) -> NSArray
    let fn = unsafeBitCast(original, to: Original.self)
    let current = fn(object, sel)
    if expandingKeyboardModes { return current }
    expandingKeyboardModes = true
    defer { expandingKeyboardModes = false }
    let existing = describeModeIDs(current).split(separator: "|").map(String.init)
    let wanted = preferredInstalledIdentifiers()
    var extra: [NSObject] = []
    for identifier in wanted {
      let already = existing.contains { id in
        if identifier.contains("HWR") { return id.contains("HWR") }
        if identifier.contains("Pinyin10") { return id.contains("Pinyin10") }
        return id == identifier
      }
      if already { continue }
      if let mode = inputMode(identifier: identifier) {
        extra.append(mode)
      }
    }
    guard !extra.isEmpty else { return current }
    let merged = NSMutableArray(array: current)
    extra.forEach { merged.add($0) }
    return merged
  }
  method_setImplementation(method, imp_implementationWithBlock(block))
  NSLog("RustDesk handwriting: keyboard list patch=1")
}

private func appendHwLog(_ line: String) {
  guard let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?
    .appendingPathComponent("hw.log") else { return }
  let text = line + "\n"
  guard let data = text.data(using: .utf8) else { return }
  if FileManager.default.fileExists(atPath: url.path),
     let handle = try? FileHandle(forWritingTo: url) {
    handle.seekToEndOfFile()
    handle.write(data)
    try? handle.close()
  } else {
    try? data.write(to: url)
  }
}

/// 手写候选提交进的是 insertAttributedText:，不会经过 insertText:。
private var insertingAttributedText = false

private func hookHandwritingCommit() {
  let sel = NSSelectorFromString("insertAttributedText:")
  guard let method = class_getInstanceMethod(UITextView.self, sel) else {
    NSLog("RustDesk handwriting: insertAttributedText missing")
    return
  }
  let original = method_getImplementation(method)
  let block: @convention(block) (AnyObject, NSAttributedString) -> Void = { object, text in
    typealias Original = @convention(c) (AnyObject, Selector, NSAttributedString) -> Void
    let fn = unsafeBitCast(original, to: Original.self)
    if insertingAttributedText {
      fn(object, sel, text)
      return
    }
    insertingAttributedText = true
    let view = object as? HandwritingTextView
    view?.handledBySession = true
    if view?.isHandwritingMode() == true {
      HandwritingBridge.shared.prepareForHandwritingCommit()
    }
    fn(object, sel, text)
    view?.publishTextChange(reason: "attr")
    insertingAttributedText = false
    DispatchQueue.main.async {
      view?.handledBySession = false
    }
  }
  method_setImplementation(method, imp_implementationWithBlock(block))
  NSLog("RustDesk handwriting: attributed insert hook=1")
}

private func hostWindow() -> UIWindow? {
  let application = UIApplication.shared
  for case let scene as UIWindowScene in application.connectedScenes {
    if let key = scene.windows.first(where: { $0.isKeyWindow }) {
      return key
    }
  }
  if let delegateWindow = (application.delegate as? AppDelegate)?.window {
    return delegateWindow
  }
  return application.windows.first(where: { $0.isKeyWindow }) ?? application.windows.first
}

private func keepChineseHandwritingKeyboard() {
  guard let inputClass = NSClassFromString("FlutterTextInputView") else {
    NSLog("RustDesk handwriting: FlutterTextInputView missing")
    return
  }
  let allowSel = NSSelectorFromString("disableHandwritingKeyboard")
  let allowBlock: @convention(block) (AnyObject) -> Bool = { _ in false }
  let added = class_addMethod(inputClass, allowSel, imp_implementationWithBlock(allowBlock), "B@:")
  NSLog("RustDesk handwriting allow method added=%d", added)
  let focusSel = #selector(UIResponder.becomeFirstResponder)
  guard let focusMethod = class_getInstanceMethod(UIResponder.self, focusSel) else {
    return
  }
  let focusOriginal = method_getImplementation(focusMethod)
  let focusBlock: @convention(block) (AnyObject) -> Bool = { object in
    typealias Focus = @convention(c) (AnyObject, Selector) -> Bool
    let fn = unsafeBitCast(focusOriginal, to: Focus.self)
    if HandwritingBridge.shared.remoteKeyboardArmed,
       object_getClass(object) === inputClass,
       (object as? NSObject)?.value(forKey: "secureTextEntry") as? Bool != true {
      let target = object as! NSObject
      if HandwritingBridge.shared.show(flutterInput: target) {
        return true
      }
    }
    return fn(object, focusSel)
  }
  method_setImplementation(focusMethod, imp_implementationWithBlock(focusBlock))

  let resignSel = #selector(UIResponder.resignFirstResponder)
  guard let resignMethod = instanceMethod(inputClass, resignSel) else {
    NSLog("RustDesk handwriting bridge focus=1 resign=0")
    return
  }
  let resignOriginal = method_getImplementation(resignMethod)
  let resignBlock: @convention(block) (AnyObject) -> Bool = { object in
    let bridge = HandwritingBridge.shared
    if bridge.remoteKeyboardArmed || bridge.field.isFirstResponder {
      bridge.hide()
    }
    typealias Resign = @convention(c) (AnyObject, Selector) -> Bool
    let fn = unsafeBitCast(resignOriginal, to: Resign.self)
    return fn(object, resignSel)
  }
  method_setImplementation(resignMethod, imp_implementationWithBlock(resignBlock))
  NSLog("RustDesk handwriting bridge focus=1 resign=1")
}

private func instanceMethod(_ cls: AnyClass, _ sel: Selector) -> Method? {
  var count: UInt32 = 0
  guard let list = class_copyMethodList(cls, &count) else {
    return nil
  }
  defer { free(list) }
  for index in 0..<Int(count) {
    if method_getName(list[index]) == sel {
      return list[index]
    }
  }
  return nil
}

@main
@objc class AppDelegate: FlutterAppDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    disableTouchRateVSyncCrash()
    keepChineseHandwritingKeyboard()
    keepInstalledChineseKeyboards()
    keepEnabledChineseKeyboards()
    keepHandwritingTraitEnabled()
    hookHandwritingCommit()
    let engine = FlutterEngine(name: "main", project: nil, allowHeadlessExecution: true)
    engine.run()
    HandwritingBridge.shared.channel = FlutterMethodChannel(
      name: "org.rustdesk.rustdesk/handwriting",
      binaryMessenger: engine.binaryMessenger
    )
    HandwritingBridge.shared.channel?.setMethodCallHandler { call, result in
      switch call.method {
      case "arm":
        HandwritingBridge.shared.setRemoteKeyboardArmed(true)
        result(nil)
      case "disarm":
        HandwritingBridge.shared.setRemoteKeyboardArmed(false)
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    }
    GeneratedPluginRegistrant.register(with: engine)
    let controller = FlutterViewController(engine: engine, nibName: nil, bundle: nil)
    let win = UIWindow(frame: UIScreen.main.bounds)
    win.rootViewController = controller
    win.makeKeyAndVisible()
    window = win
    dummyMethodToEnforceBundling()
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }
    
  public func dummyMethodToEnforceBundling() {
      dummy_method_to_enforce_bundling();
    session_get_rgba(nil, 0);
  }
}

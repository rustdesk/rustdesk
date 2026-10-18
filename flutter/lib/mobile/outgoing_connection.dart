import '../consts.dart';

/// The Android service counts each start against a later stop. A second
/// start for the same session would keep the wake lock after the session ends.
class OutgoingConnectionHold {
  var _held = false;

  void apply(bool active, void Function(String method) invoke) {
    if (active) {
      if (_held) return;
      _held = true;
      invoke(AndroidChannel.kStartOutgoingForeground);
    } else {
      if (!_held) return;
      _held = false;
      invoke(AndroidChannel.kStopOutgoingForeground);
    }
  }
}

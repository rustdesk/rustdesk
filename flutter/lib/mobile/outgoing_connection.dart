import '../consts.dart';

/// The Android service counts each start against a later stop. A second
/// start for the same session would keep the wake lock after the session ends.
class OutgoingConnectionHold {
  var _held = false;
  var _busy = false;

  // A stop can arrive while start is still waiting on Android. Keep that
  // request; dropping it would leave the wake lock held.
  bool? _queued;

  Future<void> apply(
      bool active, Future<bool> Function(String method) invoke) async {
    if (_busy) {
      _queued = active;
      return;
    }
    if (active == _held) return;
    _busy = true;
    var ok = false;
    try {
      ok = await invoke(active
          ? AndroidChannel.kStartOutgoingForeground
          : AndroidChannel.kStopOutgoingForeground);
    } catch (_) {
      ok = false;
    } finally {
      _busy = false;
    }
    if (ok) _held = active;
    final queued = _queued;
    _queued = null;
    if (queued != null && queued != _held) {
      await apply(queued, invoke);
    }
  }
}

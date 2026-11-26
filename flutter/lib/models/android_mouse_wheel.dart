import 'package:flutter/gestures.dart';

/// Converts Android wheel input that some firmware delivers as a bogus touch
/// sequence (source=TOUCHSCREEN, toolType=UNKNOWN, buttons=0, no AXIS_VSCROLL)
/// back into ordinary scroll signals.
///
/// Observed on HarmonyOS 3 tablets (MRO-W00, AGS5-W00): one wheel notch arrives
/// as down + two moves of a fixed vertical step + up, all with
/// `kind == PointerDeviceKind.unknown` and `size == 1`.
class AndroidMouseWheel {
  AndroidMouseWheel({required this.logicalPerNotch});

  /// Logical pixels per wheel notch on this device.
  ///
  /// Calibrated on MRO-W00 as 64 logical px at DPR 2.25 (= 144 physical px), so
  /// pass [kPhysicalPxPerNotch] / devicePixelRatio to stay density-correct.
  final double logicalPerNotch;

  static const double kPhysicalPxPerNotch = 144.0;

  Offset? _mousePosition;
  int? _pointer;
  Offset? _position;
  double _remainder = 0;
  int _moves = 0;
  double _seqDy = 0;
  int _seqTicks = 0;

  /// Mouse position to use when dispatching a recognized wheel signal.
  Offset get scrollPosition => _mousePosition ?? _position ?? Offset.zero;

  static bool _isSyntheticTouch(PointerEvent event) =>
      event.size <= 1.5 && event.kind == PointerDeviceKind.unknown;

  /// Returns null for unrelated input, otherwise the number of complete notches.
  int? handle(PointerEvent event) {
    if (event is PointerHoverEvent || event is PointerUpEvent) {
      if (event.kind == PointerDeviceKind.mouse) {
        _mousePosition = event.buttons == 0 ? event.position : null;
        return null;
      }
    }
    if (event is PointerDownEvent) {
      _pointer = null;
      _position = null;
      _remainder = 0;
      _moves = 0;
      _seqDy = 0;
      _seqTicks = 0;
      if (!_isSyntheticTouch(event)) {
        if (event.kind != PointerDeviceKind.unknown) _mousePosition = null;
        return null;
      }
      _pointer = event.pointer;
      _position = event.position;
      // The synthetic sequence always starts at the cursor, so it is also the
      // best position to dispatch the resulting scroll at.
      _mousePosition = event.position;
      return 0;
    }
    if (_pointer == null || event.pointer != _pointer) return null;
    if (event is PointerUpEvent || event is PointerCancelEvent) {
      // Safety net: one completed sequence is one notch. Covers firmware whose
      // per-notch distance differs from [logicalPerNotch], so a density
      // mismatch degrades into "one notch per sequence" instead of "nothing".
      var ticks = 0;
      if (event is PointerUpEvent &&
          _seqTicks == 0 &&
          _moves >= 1 &&
          _seqDy.abs() >= 8) {
        ticks = _seqDy.sign.toInt();
      }
      _pointer = null;
      _position = null;
      _remainder = 0;
      _moves = 0;
      _seqDy = 0;
      _seqTicks = 0;
      return ticks;
    }
    if (event is! PointerMoveEvent) return null;
    if (event.kind != PointerDeviceKind.unknown) return null;
    if (event.position.dx != _position!.dx) {
      _pointer = null;
      return null;
    }
    _moves++;
    final step = event.position.dy - _position!.dy;
    _seqDy += step;
    _remainder += step;
    _position = event.position;
    final notches = _remainder / logicalPerNotch;
    final ticks = (notches + notches.sign * 0.000001).truncate();
    _remainder -= ticks * logicalPerNotch;
    _seqTicks += ticks.abs();
    return ticks;
  }
}

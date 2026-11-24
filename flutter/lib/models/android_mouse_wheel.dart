import 'package:flutter/gestures.dart';

/// Converts Android wheel input delivered as unknown touch sequences.
class AndroidMouseWheel {
  static const scrollDistance = 64.0;

  Offset? _mousePosition;
  int? _pointer;
  Offset? _position;
  double _remainder = 0;

  /// Mouse position to use when dispatching a recognized wheel signal.
  Offset get scrollPosition => _mousePosition!;

  /// Returns null for unrelated input, otherwise the number of complete notches.
  int? handle(PointerEvent event) {
    if ((event is PointerHoverEvent || event is PointerUpEvent) &&
        event.kind == PointerDeviceKind.mouse) {
      _mousePosition = event.buttons == 0 ? event.position : null;
      return null;
    }
    if (event is PointerDownEvent) {
      _pointer = null;
      _position = null;
      _remainder = 0;
      if (event.kind != PointerDeviceKind.unknown) {
        _mousePosition = null;
        return null;
      }
      // Flutter can omit unchanged hovers, so wheel input may follow a stale one.
      if (event.buttons != kPrimaryButton || event.position != _mousePosition) {
        return null;
      }
      _pointer = event.pointer;
      _position = event.position;
      return 0;
    }
    if (_pointer == null || event.pointer != _pointer) return null;
    if (event is PointerUpEvent || event is PointerCancelEvent) {
      _pointer = null;
      _position = null;
      _remainder = 0;
      return 0;
    }
    if (event is! PointerMoveEvent || event.kind != PointerDeviceKind.unknown) {
      return null;
    }
    if (event.position.dx != _position!.dx) {
      _pointer = null;
      return null;
    }
    // The observed synthetic wheel notch consists of two 32-logical-pixel moves.
    _remainder += event.position.dy - _position!.dy;
    _position = event.position;
    final notches = _remainder / scrollDistance;
    final ticks = (notches + notches.sign * 0.000001).truncate();
    _remainder -= ticks * scrollDistance;
    return ticks;
  }
}

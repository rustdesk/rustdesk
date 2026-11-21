import 'dart:ui' show Offset;

/// Sample the OS cursor when handling a move, so queued Flutter notifications
/// from before a warp cannot introduce movement after it.
class WindowsRelativePointer {
  WindowsRelativePointer(this.readPosition);
  final Offset? Function() readPosition;
  Offset? _last;

  Offset? movement() {
    final previous = _last;
    final current = _last = readPosition();
    return previous == null || current == null ? null : current - previous;
  }

  // Windows may clamp SetCursorPos to the clip rectangle.
  void didWarp() => _last = readPosition();
  void reset() => _last = null;
}

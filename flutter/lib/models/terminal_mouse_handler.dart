import 'package:xterm/xterm.dart';

/// xterm 4.0.0 encodes wheel buttons as 68..71; the extra bit reads as a Shift
/// modifier, so strict full-screen apps ignore the report and never scroll.
/// Upstream fix: TerminalStudio/xterm.dart#238.
class WheelButtonFixMouseHandler implements TerminalMouseHandler {
  const WheelButtonFixMouseHandler();

  @override
  String? call(TerminalMouseEvent event) {
    if (!event.button.isWheel) {
      return defaultMouseHandler(event);
    }
    // Same gate as UpDownMouseHandler: only the scroll modes report a wheel,
    // and a wheel release is never reported, so the report is always a press.
    if (!event.state.mouseMode.reportScroll ||
        event.buttonState == TerminalMouseButtonState.up) {
      return null;
    }
    return _reportWheel(event);
  }

  String _reportWheel(TerminalMouseEvent event) {
    // Wheel buttons 4..7 go on the wire as 64..67, but `id` is 64 + 4..7.
    final button = event.button.id - 4;
    final x = event.position.x + 1;
    final y = event.position.y + 1;
    switch (event.state.mouseReportMode) {
      case MouseReportMode.normal:
      case MouseReportMode.utf:
        final limit =
            event.state.mouseReportMode == MouseReportMode.normal ? 223 : 2015;
        final col = x > limit ? '\x00' : String.fromCharCode(32 + x);
        final row = y > limit ? '\x00' : String.fromCharCode(32 + y);
        return '\x1b[M${String.fromCharCode(32 + button)}$col$row';
      case MouseReportMode.sgr:
        return '\x1b[<$button;$x;${y}M';
      case MouseReportMode.urxvt:
        return '\x1b[${32 + button};$x;${y}M';
    }
  }
}

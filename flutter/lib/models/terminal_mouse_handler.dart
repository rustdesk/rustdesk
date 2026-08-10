import 'package:xterm/xterm.dart';

/// xterm 4.0.0 encodes wheel buttons as 68..71; the extra bit reads as a Shift
/// modifier, so strict full-screen apps ignore the report and never scroll.
/// Upstream fix: TerminalStudio/xterm.dart#238.
class WheelButtonFixMouseHandler implements TerminalMouseHandler {
  const WheelButtonFixMouseHandler();

  @override
  String? call(TerminalMouseEvent event) {
    final report = defaultMouseHandler(event);
    if (report == null || !event.button.isWheel) {
      return report;
    }
    return _reportWheel(event);
  }

  String _reportWheel(TerminalMouseEvent event) {
    final button = _wheelButtonId(event.button);
    final x = event.position.x + 1;
    final y = event.position.y + 1;
    switch (event.state.mouseReportMode) {
      case MouseReportMode.normal:
      case MouseReportMode.utf:
        final limit =
            event.state.mouseReportMode == MouseReportMode.normal ? 223 : 2015;
        final col = x > limit ? '\x00' : String.fromCharCode(32 + x);
        final row = y > limit ? '\x00' : String.fromCharCode(32 + y + 1);
        return '\x1b[M${String.fromCharCode(32 + button)}$col$row';
      case MouseReportMode.sgr:
        return '\x1b[<$button;$x;${y}M';
      case MouseReportMode.urxvt:
        return '\x1b[${32 + button};$x;${y}M';
    }
  }

  int _wheelButtonId(TerminalMouseButton button) {
    switch (button) {
      case TerminalMouseButton.wheelUp:
        return 64;
      case TerminalMouseButton.wheelDown:
        return 65;
      case TerminalMouseButton.wheelLeft:
        return 66;
      case TerminalMouseButton.wheelRight:
        return 67;
      default:
        return button.id;
    }
  }
}

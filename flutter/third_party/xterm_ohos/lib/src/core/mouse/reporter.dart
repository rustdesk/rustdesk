import 'package:xterm/src/core/buffer/cell_offset.dart';
import 'package:xterm/src/core/mouse/mode.dart';
import 'package:xterm/src/core/mouse/button.dart';
import 'package:xterm/src/core/mouse/button_state.dart';

abstract class MouseReporter {
  static String report(
    TerminalMouseButton button,
    TerminalMouseButtonState state,
    CellOffset position,
    MouseReportMode reportMode,
  ) {
    // x and y offsets have to be incremented by 1 as the offset if 0-based,
    // The position has to be reported using 1-based coordinates.
    final x = position.x + 1;
    final y = position.y + 1;
    switch (reportMode) {
      case MouseReportMode.normal:
      case MouseReportMode.utf:
        // Button ID 3 is used to signal a button release.
        final buttonID = state == TerminalMouseButtonState.up ? 3 : button.id;
        // The button ID is reported as shifted by 32 to produce a printable
        // character.
        final btn = String.fromCharCode(32 + buttonID);
        // Normal mode only supports a maximum position of 223, while utf
        // supports positions up to 2015. Both modes send a null byte if the
        // position exceeds that limit.
        final col = (reportMode == MouseReportMode.normal && x > 223) ||
                (reportMode == MouseReportMode.utf && x > 2015)
            ? '\x00'
            : String.fromCharCode(32 + x);
        final row = (reportMode == MouseReportMode.normal && y > 223) ||
                (reportMode == MouseReportMode.utf && y > 2015)
            ? '\x00'
            : String.fromCharCode(32 + y + 1);
        return "\x1b[M$btn$col$row";
      case MouseReportMode.sgr:
        final buttonID = button.id;
        final upDown = state == TerminalMouseButtonState.down ? 'M' : 'm';
        return "\x1b[<$buttonID;$x;$y$upDown";
      case MouseReportMode.urxvt:
        // The button ID uses the same id as to report it as in normal mode.
        final buttonID =
            32 + (state == TerminalMouseButtonState.up ? 3 : button.id);
        return "\x1b[$buttonID;$x;${y}M";
    }
  }
}

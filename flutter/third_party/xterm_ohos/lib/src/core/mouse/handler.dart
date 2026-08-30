import 'package:xterm/src/core/buffer/cell_offset.dart';
import 'package:xterm/src/core/mouse/button_state.dart';
import 'package:xterm/src/core/mouse/mode.dart';
import 'package:xterm/src/core/mouse/button.dart';
import 'package:xterm/src/core/mouse/reporter.dart';
import 'package:xterm/src/core/platform.dart';
import 'package:xterm/src/core/state.dart';

class TerminalMouseEvent {
  /// The button that is pressed or released.
  final TerminalMouseButton button;

  /// The current state of the button.
  final TerminalMouseButtonState buttonState;

  /// The position of button state change.
  final CellOffset position;

  /// The state of the terminal.
  final TerminalState state;

  /// The platform of the terminal.
  final TerminalTargetPlatform platform;

  TerminalMouseEvent({
    required this.button,
    required this.buttonState,
    required this.position,
    required this.state,
    required this.platform,
  });
}

const defaultMouseHandler = CascadeMouseHandler([
  ClickMouseHandler(),
  UpDownMouseHandler(),
]);

abstract class TerminalMouseHandler {
  const TerminalMouseHandler();

  String? call(TerminalMouseEvent event);
}

class CascadeMouseHandler implements TerminalMouseHandler {
  final List<TerminalMouseHandler> _handlers;

  const CascadeMouseHandler(this._handlers);

  @override
  String? call(TerminalMouseEvent event) {
    for (var handler in _handlers) {
      final result = handler(event);
      if (result != null) {
        return result;
      }
    }
    return null;
  }
}

class ClickMouseHandler implements TerminalMouseHandler {
  const ClickMouseHandler();

  @override
  String? call(TerminalMouseEvent event) {
    switch (event.state.mouseMode) {
      case MouseMode.clickOnly:
        // Only clicks and only the first 3 buttons are reported.
        if (event.buttonState == TerminalMouseButtonState.down &&
            (event.button.id < 3)) {
          return MouseReporter.report(
            event.button,
            event.buttonState,
            event.position,
            event.state.mouseReportMode,
          );
        }
        return null;
      case MouseMode.none:
      case MouseMode.upDownScroll:
      case MouseMode.upDownScrollDrag:
      case MouseMode.upDownScrollMove:
        return null;
    }
  }
}

class UpDownMouseHandler implements TerminalMouseHandler {
  const UpDownMouseHandler();

  @override
  String? call(TerminalMouseEvent event) {
    switch (event.state.mouseMode) {
      case MouseMode.none:
      case MouseMode.clickOnly:
        return null;
      case MouseMode.upDownScroll:
      case MouseMode.upDownScrollDrag:
      case MouseMode.upDownScrollMove:
        // Up events are never reported for mouse wheel buttons.
        if (event.button.isWheel &&
            event.buttonState == TerminalMouseButtonState.up) {
          return null;
        }
        return MouseReporter.report(
          event.button,
          event.buttonState,
          event.position,
          event.state.mouseReportMode,
        );
    }
  }
}

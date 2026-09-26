import 'package:flutter/widgets.dart';
import 'package:xterm/xterm.dart';

class TerminalScrollController extends ScrollController {
  TerminalScrollController(this.terminal);

  final Terminal Function() terminal;

  @override
  ScrollPosition createScrollPosition(ScrollPhysics physics,
      ScrollContext context, ScrollPosition? oldPosition) {
    return super.createScrollPosition(
      _TerminalScrollPhysics(terminal, parent: physics),
      context,
      oldPosition,
    );
  }
}

class _TerminalScrollPhysics extends ScrollPhysics {
  const _TerminalScrollPhysics(this.terminal, {super.parent});

  final Terminal Function() terminal;

  @override
  _TerminalScrollPhysics applyTo(ScrollPhysics? ancestor) =>
      _TerminalScrollPhysics(terminal, parent: buildParent(ancestor));

  @override
  bool shouldAcceptUserOffset(ScrollMetrics position) {
    // A retained main-screen offset can make this inner scrollable win the
    // trackpad gesture, starving xterm's outer alternate-screen scroll handler.
    if (terminal().isUsingAltBuffer) return false;
    return super.shouldAcceptUserOffset(position);
  }
}

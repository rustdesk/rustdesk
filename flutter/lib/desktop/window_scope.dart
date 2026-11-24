import 'package:flutter/widgets.dart';

import '../models/state_model.dart';
import '../utils/multi_window_manager.dart';

/// The identity and chrome state of a desktop session window.
class DesktopWindowScope extends InheritedWidget {
  const DesktopWindowScope({
    super.key,
    required this.windowId,
    required this.windowType,
    required this.windowState,
    required super.child,
  });

  final int windowId;
  final WindowType windowType;
  final StateGlobal windowState;

  // A window's identity is fixed throughout its subtree's lifetime.
  static DesktopWindowScope? read(BuildContext context) =>
      context.getInheritedWidgetOfExactType<DesktopWindowScope>();

  @override
  bool updateShouldNotify(DesktopWindowScope oldWidget) =>
      windowId != oldWidget.windowId ||
      windowType != oldWidget.windowType ||
      windowState != oldWidget.windowState;
}

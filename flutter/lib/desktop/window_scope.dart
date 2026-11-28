import 'dart:async';

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
    this.zone,
    this.nativeTitleBar = false,
    required super.child,
  });

  final int windowId;
  final WindowType windowType;
  final StateGlobal windowState;
  final Zone? zone;
  final bool nativeTitleBar;

  T run<T>(T Function() action) {
    final windowZone = zone;
    return windowZone == null ? action() : windowZone.run(action);
  }

  // A window's identity is fixed throughout its subtree's lifetime.
  static DesktopWindowScope? read(BuildContext context) =>
      context.getInheritedWidgetOfExactType<DesktopWindowScope>();

  @override
  bool updateShouldNotify(DesktopWindowScope oldWidget) =>
      windowId != oldWidget.windowId ||
      windowType != oldWidget.windowType ||
      windowState != oldWidget.windowState ||
      zone != oldWidget.zone ||
      nativeTitleBar != oldWidget.nativeTitleBar;
}

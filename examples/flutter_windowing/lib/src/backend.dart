import 'package:flutter/services.dart';

import 'window_controller.dart';
import 'window_listener.dart';

typedef WindowMethodHandler = Future<dynamic> Function(
    MethodCall call, int fromWindowId);

/// Only installed by an explicit experimental bootstrap. Null keeps legacy code.
DesktopWindowBackend? experimentalWindowBackend;

abstract class DesktopWindowBackend {
  bool acceptsWindow(String? arguments);
  bool ownsWindow(int id);
  bool get keepsLegacyWindows;
  int get currentWindowId;
  Future<WindowController> createWindow(String? arguments);
  WindowController controller(int id);
  Future<dynamic> invokeMethod(int target, String method, dynamic arguments);
  void setMethodHandler(WindowMethodHandler? handler);
  Future<List<int>> getAllSubWindowIds();
  List<MultiWindowListener> get listeners;
  void addListener(MultiWindowListener listener);
  void removeListener(MultiWindowListener listener);
}

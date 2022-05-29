import 'dart:convert';
import 'dart:ui';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/services.dart';

/// must keep the order
enum WindowType { Main, RemoteDesktop, FileTransfer, PortForward, Unknown }

extension Index on int {
  WindowType get windowType {
    switch (this) {
      case 0:
        return WindowType.Main;
      case 1:
        return WindowType.RemoteDesktop;
      case 2:
        return WindowType.FileTransfer;
      case 3:
        return WindowType.PortForward;
      default:
        return WindowType.Unknown;
    }
  }
}

/// Window Manager
/// mainly use it in `Main Window`
/// use it in sub window is not recommended
class RustDeskMultiWindowManager {
  RustDeskMultiWindowManager._();

  static final instance = RustDeskMultiWindowManager._();

  int? _remoteDesktopWindowId;

  Future<dynamic> new_remote_desktop(String remote_id) async {
    final msg =
        jsonEncode({"type": WindowType.RemoteDesktop.index, "id": remote_id});

    try {
      final ids = await DesktopMultiWindow.getAllSubWindowIds();
      if (!ids.contains(_remoteDesktopWindowId)) {
        _remoteDesktopWindowId = null;
      }
    } on Error {
      _remoteDesktopWindowId = null;
    }
    if (_remoteDesktopWindowId == null) {
      final remoteDesktopController =
          await DesktopMultiWindow.createWindow(msg);
      remoteDesktopController
        ..setFrame(const Offset(0, 0) & const Size(1280, 720))
        ..center()
        ..setTitle("rustdesk - remote desktop")
        ..show();
      _remoteDesktopWindowId = remoteDesktopController.windowId;
    } else {
      return call(WindowType.RemoteDesktop, "new_remote_desktop", msg);
    }
  }

  Future<dynamic> call(WindowType type, String methodName, dynamic args) async {
    int? windowId = findWindowByType(type);
    if (windowId == null) {
      return;
    }
    return await DesktopMultiWindow.invokeMethod(windowId, methodName, args);
  }

  int? findWindowByType(WindowType type) {
    switch (type) {
      case WindowType.Main:
        break;
      case WindowType.RemoteDesktop:
        return _remoteDesktopWindowId;
      case WindowType.FileTransfer:
        break;
      case WindowType.PortForward:
        break;
      case WindowType.Unknown:
        break;
    }
    return null;
  }

  void setMethodHandler(
      Future<dynamic> Function(MethodCall call, int fromWindowId)? handler) {
    DesktopMultiWindow.setMethodHandler(handler);
  }
}

final rustDeskWinManager = RustDeskMultiWindowManager.instance;

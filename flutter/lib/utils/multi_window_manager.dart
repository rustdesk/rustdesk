import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';

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
  int? _fileTransferWindowId;
  int? _portForwardWindowId;

  Future<dynamic> newRemoteDesktop(String remoteId) async {
    final msg =
        jsonEncode({"type": WindowType.RemoteDesktop.index, "id": remoteId});

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

  Future<dynamic> newFileTransfer(String remoteId) async {
    final msg =
        jsonEncode({"type": WindowType.FileTransfer.index, "id": remoteId});

    try {
      final ids = await DesktopMultiWindow.getAllSubWindowIds();
      if (!ids.contains(_fileTransferWindowId)) {
        _fileTransferWindowId = null;
      }
    } on Error {
      _fileTransferWindowId = null;
    }
    if (_fileTransferWindowId == null) {
      final fileTransferController = await DesktopMultiWindow.createWindow(msg);
      fileTransferController
        ..setFrame(const Offset(0, 0) & const Size(1280, 720))
        ..center()
        ..setTitle("rustdesk - file transfer")
        ..show();
      _fileTransferWindowId = fileTransferController.windowId;
    } else {
      return call(WindowType.FileTransfer, "new_file_transfer", msg);
    }
  }

  Future<dynamic> newPortForward(String remoteId, bool isRDP) async {
    final msg = jsonEncode(
        {"type": WindowType.PortForward.index, "id": remoteId, "isRDP": isRDP});

    try {
      final ids = await DesktopMultiWindow.getAllSubWindowIds();
      if (!ids.contains(_portForwardWindowId)) {
        _portForwardWindowId = null;
      }
    } on Error {
      _portForwardWindowId = null;
    }
    if (_portForwardWindowId == null) {
      final portForwardController = await DesktopMultiWindow.createWindow(msg);
      portForwardController
        ..setFrame(const Offset(0, 0) & const Size(1280, 720))
        ..center()
        ..setTitle("rustdesk - port forward")
        ..show();
      _portForwardWindowId = portForwardController.windowId;
    } else {
      return call(WindowType.PortForward, "new_port_forward", msg);
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
        return 0;
      case WindowType.RemoteDesktop:
        return _remoteDesktopWindowId;
      case WindowType.FileTransfer:
        return _fileTransferWindowId;
      case WindowType.PortForward:
        return _portForwardWindowId;
      case WindowType.Unknown:
        break;
    }
    return null;
  }

  void setMethodHandler(
      Future<dynamic> Function(MethodCall call, int fromWindowId)? handler) {
    DesktopMultiWindow.setMethodHandler(handler);
  }

  Future<void> closeAllSubWindows() async {
    await Future.wait(WindowType.values.map((e) => closeWindows(e)));
  }

  Future<void> closeWindows(WindowType type) async {
    if (type == WindowType.Main) {
      // skip main window, use window manager instead
      return;
    }
    int? wId = findWindowByType(type);
    if (wId != null) {
      debugPrint("closing multi window: ${type.toString()}");
      await saveWindowPosition(type, windowId: wId);
      try {
        final ids = await DesktopMultiWindow.getAllSubWindowIds();
        if (!ids.contains(wId)) {
          // no such window already
          return;
        }
        await WindowController.fromWindowId(wId).close();
      } on Error {
        return;
      }
    }
  }
}

final rustDeskWinManager = RustDeskMultiWindowManager.instance;

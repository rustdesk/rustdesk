import 'dart:io';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../consts.dart';

class StateGlobal {
  int _windowId = -1;
  bool _fullscreen = false;
  bool _maximize = false;
  bool grabKeyboard = false;
  final RxBool _showTabBar = true.obs;
  final RxBool _showResizeEdge = true.obs;
  final RxDouble _resizeEdgeSize = RxDouble(kWindowEdgeSize);
  final RxDouble _windowBorderWidth = RxDouble(kWindowBorderWidth);
  final RxBool showRemoteMenuBar = false.obs;
  final RxInt displaysCount = 0.obs;

  int get windowId => _windowId;
  bool get fullscreen => _fullscreen;
  bool get maximize => _maximize;
  double get tabBarHeight => fullscreen ? 0 : kDesktopRemoteTabBarHeight;
  RxBool get showTabBar => _showTabBar;
  RxDouble get resizeEdgeSize => _resizeEdgeSize;
  RxDouble get windowBorderWidth => _windowBorderWidth;

  setWindowId(int id) => _windowId = id;
  setMaximize(bool v) {
    if (_maximize != v) {
      _maximize = v;
      _resizeEdgeSize.value =
          _maximize ? kMaximizeEdgeSize : kWindowEdgeSize;
    }
  }
  setFullscreen(bool v) {
    if (_fullscreen != v) {
      _fullscreen = v;
      _showTabBar.value = !_fullscreen;
      _resizeEdgeSize.value =
          fullscreen ? kFullScreenEdgeSize : kWindowEdgeSize;
      _windowBorderWidth.value = fullscreen ? 0 : kWindowBorderWidth;
      WindowController.fromWindowId(windowId)
          .setFullscreen(_fullscreen)
          .then((_) {
        // https://github.com/leanflutter/window_manager/issues/131#issuecomment-1111587982
        if (Platform.isWindows && !v) {
          Future.delayed(Duration.zero, () async {
            final frame =
                await WindowController.fromWindowId(windowId).getFrame();
            final newRect = Rect.fromLTWH(
                frame.left, frame.top, frame.width + 1, frame.height + 1);
            await WindowController.fromWindowId(windowId).setFrame(newRect);
          });
        }
      });
    }
  }

  StateGlobal._();

  static final StateGlobal instance = StateGlobal._();
}

final stateGlobal = StateGlobal.instance;

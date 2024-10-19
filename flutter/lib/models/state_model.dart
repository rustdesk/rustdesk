import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';

import '../consts.dart';
import './platform_model.dart';

enum SvcStatus { notReady, connecting, ready }

class StateGlobal {
  int _windowId = -1;
  final RxBool _fullscreen = false.obs;
  bool _isMinimized = false;
  final RxBool isMaximized = false.obs;
  final RxBool _showTabBar = true.obs;
  final RxDouble _resizeEdgeSize = RxDouble(windowResizeEdgeSize);
  final RxDouble _windowBorderWidth = RxDouble(kWindowBorderWidth);
  final RxBool showRemoteToolBar = false.obs;
  final svcStatus = SvcStatus.notReady.obs;
  final RxBool isFocused = false.obs;
  // for mobile and web
  bool isInMainPage = true;
  bool isWebVisible = true;

  final isPortrait = false.obs;

  String _inputSource = '';

  // Use for desktop -> remote toolbar -> resolution
  final Map<String, Map<int, String?>> _lastResolutionGroupValues = {};

  int get windowId => _windowId;
  RxBool get fullscreen => _fullscreen;
  bool get isMinimized => _isMinimized;
  double get tabBarHeight => fullscreen.isTrue ? 0 : kDesktopRemoteTabBarHeight;
  RxBool get showTabBar => _showTabBar;
  RxDouble get resizeEdgeSize => _resizeEdgeSize;
  RxDouble get windowBorderWidth => _windowBorderWidth;

  resetLastResolutionGroupValues(String peerId) {
    _lastResolutionGroupValues[peerId] = {};
  }

  setLastResolutionGroupValue(
      String peerId, int currentDisplay, String? value) {
    if (!_lastResolutionGroupValues.containsKey(peerId)) {
      _lastResolutionGroupValues[peerId] = {};
    }
    _lastResolutionGroupValues[peerId]![currentDisplay] = value;
  }

  String? getLastResolutionGroupValue(String peerId, int currentDisplay) {
    return _lastResolutionGroupValues[peerId]?[currentDisplay];
  }

  setWindowId(int id) => _windowId = id;
  setMaximized(bool v) {
    if (!_fullscreen.isTrue) {
      if (isMaximized.value != v) {
        isMaximized.value = v;
        refreshResizeEdgeSize();
      }
      if (!isMacOS) {
        _windowBorderWidth.value = v ? 0 : kWindowBorderWidth;
      }
    }
  }

  setMinimized(bool v) => _isMinimized = v;

  setFullscreen(bool v, {bool procWnd = true}) {
    if (_fullscreen.value != v) {
      _fullscreen.value = v;
      _showTabBar.value = !_fullscreen.value;
      if (isWebDesktop) {
        procFullscreenWeb();
      } else {
        procFullscreenNative(procWnd);
      }
    }
  }

  procFullscreenWeb() {
    final isFullscreen = ffiGetByName('fullscreen') == 'Y';
    String fullscreenValue = '';
    if (isFullscreen && _fullscreen.isFalse) {
      fullscreenValue = 'N';
    } else if (!isFullscreen && fullscreen.isTrue) {
      fullscreenValue = 'Y';
    }
    if (fullscreenValue.isNotEmpty) {
      ffiSetByName('fullscreen', fullscreenValue);
    }
  }

  procFullscreenNative(bool procWnd) {
    refreshResizeEdgeSize();
    print("fullscreen: $fullscreen, resizeEdgeSize: ${_resizeEdgeSize.value}");
    _windowBorderWidth.value = fullscreen.isTrue ? 0 : kWindowBorderWidth;
    if (procWnd) {
      final wc = WindowController.fromWindowId(windowId);
      wc.setFullscreen(_fullscreen.isTrue).then((_) {
        // We remove the redraw (width + 1, height + 1), because this issue cannot be reproduced.
        // https://github.com/rustdesk/rustdesk/issues/9675
      });
    }
  }

  refreshResizeEdgeSize() => _resizeEdgeSize.value = fullscreen.isTrue
      ? kFullScreenEdgeSize
      : isMaximized.isTrue
          ? kMaximizeEdgeSize
          : windowResizeEdgeSize;

  String getInputSource({bool force = false}) {
    if (force || _inputSource.isEmpty) {
      _inputSource = bind.mainGetInputSource();
    }
    return _inputSource;
  }

  setInputSource(SessionID sessionId, String v) async {
    await bind.mainSetInputSource(sessionId: sessionId, value: v);
    _inputSource = bind.mainGetInputSource();
  }

  StateGlobal._() {
    if (isWebDesktop) {
      platformFFI.setFullscreenCallback((v) {
        _fullscreen.value = v;
      });
    }
  }

  static final StateGlobal instance = StateGlobal._();
}

// This final variable is initialized when the first time it is accessed.
final stateGlobal = StateGlobal.instance;

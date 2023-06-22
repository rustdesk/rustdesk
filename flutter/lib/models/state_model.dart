import 'dart:convert';
import 'dart:io';
import 'dart:async';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../consts.dart';
import '../common.dart';

import './platform_model.dart';
import './user_model.dart';

enum SvcStatus { notReady, connecting, ready }

class StateGlobal {
  int _windowId = -1;
  bool _fullscreen = false;
  bool _maximize = false;
  bool grabKeyboard = false;
  final RxBool _showTabBar = true.obs;
  final RxDouble _resizeEdgeSize = RxDouble(kWindowEdgeSize);
  final RxDouble _windowBorderWidth = RxDouble(kWindowBorderWidth);
  final RxBool showRemoteToolBar = false.obs;
  final RxInt displaysCount = 0.obs;

  final svcStatus = SvcStatus.notReady.obs;
  final svcIsUsingPublicServer = true.obs;
  Timer? _svcStatusTimer;

  // Use for desktop -> remote toolbar -> resolution
  final Map<String, Map<int, String?>> _lastResolutionGroupValues = {};

  int get windowId => _windowId;
  bool get fullscreen => _fullscreen;
  bool get maximize => _maximize;
  double get tabBarHeight => fullscreen ? 0 : kDesktopRemoteTabBarHeight;
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
  setMaximize(bool v) {
    if (_maximize != v && !_fullscreen) {
      _maximize = v;
      _resizeEdgeSize.value = _maximize ? kMaximizeEdgeSize : kWindowEdgeSize;
    }
  }

  setFullscreen(bool v) {
    if (_fullscreen != v) {
      _fullscreen = v;
      _showTabBar.value = !_fullscreen;
      _resizeEdgeSize.value = fullscreen
          ? kFullScreenEdgeSize
          : _maximize
              ? kMaximizeEdgeSize
              : kWindowEdgeSize;
      print(
          "fullscreen: ${fullscreen}, resizeEdgeSize: ${_resizeEdgeSize.value}");
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

  startSvcStatusTimer() {
    _svcStatusTimer = periodic_immediate(Duration(seconds: 1), () async {
      _updateSvcStatus();
    });
  }

  cancelSvcStatusTimer() {
    _svcStatusTimer?.cancel();
    _svcStatusTimer = null;
  }

  _updateSvcStatus() async {
    final status =
        jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
    final statusNum = status['status_num'] as int;
    final preStatus = stateGlobal.svcStatus.value;
    if (statusNum == 0) {
      stateGlobal.svcStatus.value = SvcStatus.connecting;
    } else if (statusNum == -1) {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    } else if (statusNum == 1) {
      stateGlobal.svcStatus.value = SvcStatus.ready;
      if (preStatus != SvcStatus.ready) {
        gFFI.userModel.refreshCurrentUser();
      }
    } else {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    }
    if (stateGlobal.svcStatus.value != SvcStatus.ready) {
      gFFI.userModel.isAdmin.value = false;
      gFFI.groupModel.reset();
    }
    stateGlobal.svcIsUsingPublicServer.value =
        await bind.mainIsUsingPublicServer();
  }

  StateGlobal._();

  static final StateGlobal instance = StateGlobal._();
}

final stateGlobal = StateGlobal.instance;

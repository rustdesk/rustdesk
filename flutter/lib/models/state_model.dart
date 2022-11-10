import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:get/get.dart';

import '../consts.dart';

class StateGlobal {
  int _windowId = -1;
  bool _fullscreen = false;
  final RxBool _showTabBar = true.obs;
  final RxDouble _resizeEdgeSize = 8.0.obs;
  final RxBool showRemoteMenuBar = false.obs;

  int get windowId => _windowId;
  bool get fullscreen => _fullscreen;
  double get tabBarHeight => fullscreen ? 0 : kDesktopRemoteTabBarHeight;
  double get windowBorderWidth => fullscreen ? 0 : kWindowBorderWidth;
  RxBool get showTabBar => _showTabBar;
  RxDouble get resizeEdgeSize => _resizeEdgeSize;

  setWindowId(int id) => _windowId = id;
  setFullscreen(bool v) {
    if (_fullscreen != v) {
      _fullscreen = v;
      _showTabBar.value = !_fullscreen;
      _resizeEdgeSize.value =
          fullscreen ? kFullScreenEdgeSize : kWindowEdgeSize;
      WindowController.fromWindowId(windowId).setFullscreen(_fullscreen);
    }
  }

  StateGlobal._();

  static final StateGlobal instance = StateGlobal._();
}

final stateGlobal = StateGlobal.instance;

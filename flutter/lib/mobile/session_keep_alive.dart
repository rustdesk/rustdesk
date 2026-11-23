import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';

import '../common.dart';
import '../models/platform_model.dart';

/// Android only, opt-in: holds the keep-alive foreground service for a remote page, and
/// stops the video while the app is off screen.
class SessionKeepAlive {
  // A replaced page is disposed after its successor has started.
  static int _pages = 0;
  static const _repauseInterval = Duration(seconds: 15);

  // Read once per page, so a setting changed mid-session can't unbalance _pages.
  bool _enabled = false;
  Timer? _repause;

  void attach() {
    _enabled = isAndroid &&
        bind.mainGetLocalOption(key: kOptionKeepSessionsInBackground) == 'Y';
    if (!_enabled) return;
    if (_pages++ == 0) _invoke('session_keep_alive_start');
  }

  void detach() {
    if (!_enabled) return;
    _repause?.cancel();
    _repause = null;
    if (--_pages == 0) _invoke('session_keep_alive_stop');
  }

  void onLifecycle(AppLifecycleState state) {
    if (!_enabled) return;
    switch (state) {
      case AppLifecycleState.paused:
        if (_repause != null) return;
        _pauseVideo();
        // A reconnect or a display change on the peer subscribes to video again.
        _repause = Timer.periodic(_repauseInterval, (_) => _pauseVideo());
        break;
      case AppLifecycleState.inactive: // visible without focus, as in split screen
      case AppLifecycleState.resumed:
        if (_repause == null) return;
        _repause?.cancel();
        _repause = null;
        unawaited(_resumeVideo());
        break;
      default:
        break;
    }
  }

  void _invoke(String method) {
    unawaited(gFFI.invokeMethod(method).catchError((Object e) {
      debugPrint('$method failed: $e');
      return false;
    }));
  }

  void _pauseVideo() {
    final model = gFFI.ffiModel;
    // Auto Login waits for the first frame. Peers without multi-display support can't
    // subscribe again after an empty capture set.
    if (model.waitForFirstImage.isTrue || !model.pi.isSupportMultiUiSession) {
      return;
    }
    unawaited(bind.sessionSwitchDisplay(
      isDesktop: false,
      sessionId: gFFI.sessionId,
      value: Int32List(0),
    ));
  }

  Future<void> _resumeVideo() async {
    final pi = gFFI.ffiModel.pi;
    if (pi.displays.isEmpty) return;
    final current = pi.currentDisplay;
    final displays = current == kAllDisplayValue
        ? List.generate(pi.displays.length, (i) => i)
        : [current >= 0 && current < pi.displays.length ? current : 0];
    final sessionId = gFFI.sessionId;
    await bind.sessionSwitchDisplay(
      isDesktop: false,
      sessionId: sessionId,
      value: Int32List.fromList(displays),
    );
    for (final display in displays) {
      await bind.sessionRefresh(sessionId: sessionId, display: display);
    }
  }
}

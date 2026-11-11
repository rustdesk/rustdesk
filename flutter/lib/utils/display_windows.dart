// Per-peer, per-display remote window memory.
//
// Peer flutter option `kPeerOptionDisplayWindows` holds a JSON object keyed by the remote display index
// (`kAllDisplayValue` included) with one `DisplayWindowState` per display that was ever shown in its own window.
// An entry is added or updated whenever a remote window showing that display moves, resizes, changes maximized or
// fullscreen state, switches display or closes; entries are never removed, so closing the windows one at a time keeps
// every display.
//
// Writes MUST go through the main window isolate (`kWindowEventSaveDisplayWindow`): every remote window is its own
// isolate, and the option is a read-modify-write of one string, so concurrent writers would drop each other's
// entries.

import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_size/window_size.dart' as window_size;

const String kPeerOptionDisplayWindows = 'display_windows';
const String kWindowEventSaveDisplayWindow = 'save_display_window';

/// Sent to every sub window with a peer id: the window closes its tab of that peer (the toolbar's close-session
/// button closes the peer's session in all windows).
const String kWindowEventClosePeerSessions = 'close_peer_sessions';

/// Closes the tab of `peerId` in every other sub window; the caller closes its own.
Future<void> closePeerSessionsInOtherWindows(String peerId) async {
  List<int> ids = [];
  try {
    ids = await DesktopMultiWindow.getAllSubWindowIds();
  } catch (e) {
    displayWindowsLog('closePeerSessions: getAllSubWindowIds failed: $e');
  }
  for (final id in ids) {
    if (id == kWindowId) continue;
    try {
      await DesktopMultiWindow.invokeMethod(
          id, kWindowEventClosePeerSessions, peerId);
    } catch (e) {
      displayWindowsLog('closePeerSessions: window $id did not answer: $e');
    }
  }
}

/// Restore frame of a window whose target local screen no longer exists; the window is maximized on the primary
/// screen over it.
const Size kDisplayWindowFallbackSize = Size(600, 400);
const double kDisplayWindowFallbackOffset = 30;

class DisplayWindowState {
  /// Restore (non-maximized, non-fullscreen) frame.
  final Rect frame;

  /// Frame of the local screen that contained the center of `frame` when it was saved.
  final Rect? screen;
  final bool isMaximized;
  final bool isFullscreen;

  /// View style of the window (`kRemoteViewStyleOriginal` / `Adaptive` / `Custom`) and its custom scale percent.
  final String? viewStyle;
  final int? customScalePercent;

  /// Scroll offsets in pixels (scrollbar / edge scroll modes).
  final double? scrollX;
  final double? scrollY;

  DisplayWindowState(
      this.frame, this.screen, this.isMaximized, this.isFullscreen,
      {this.viewStyle, this.customScalePercent, this.scrollX, this.scrollY});

  Map<String, dynamic> toJson() => {
        'l': frame.left,
        't': frame.top,
        'w': frame.width,
        'h': frame.height,
        if (screen != null)
          'screen': {
            'l': screen!.left,
            't': screen!.top,
            'r': screen!.right,
            'b': screen!.bottom,
          },
        'isMaximized': isMaximized,
        'isFullscreen': isFullscreen,
        if (viewStyle != null) 'viewStyle': viewStyle,
        if (customScalePercent != null) 'customScalePercent': customScalePercent,
        if (scrollX != null) 'scrollX': scrollX,
        if (scrollY != null) 'scrollY': scrollY,
      };

  static DisplayWindowState? fromJson(dynamic m) {
    try {
      final s = m['screen'];
      return DisplayWindowState(
        Rect.fromLTWH((m['l'] as num).toDouble(), (m['t'] as num).toDouble(),
            (m['w'] as num).toDouble(), (m['h'] as num).toDouble()),
        s == null
            ? null
            : Rect.fromLTRB(
                (s['l'] as num).toDouble(),
                (s['t'] as num).toDouble(),
                (s['r'] as num).toDouble(),
                (s['b'] as num).toDouble()),
        m['isMaximized'] == true,
        m['isFullscreen'] == true,
        viewStyle: m['viewStyle'] as String?,
        customScalePercent: (m['customScalePercent'] as num?)?.toInt(),
        scrollX: (m['scrollX'] as num?)?.toDouble(),
        scrollY: (m['scrollY'] as num?)?.toDouble(),
      );
    } catch (e) {
      debugPrint('Failed to parse DisplayWindowState $m: $e');
      return null;
    }
  }
}

/// Returns the stored windows of `peerId`, keyed by remote display index.
Map<int, DisplayWindowState> loadDisplayWindows(String peerId) {
  final res = <int, DisplayWindowState>{};
  final raw =
      bind.mainGetPeerFlutterOptionSync(id: peerId, k: kPeerOptionDisplayWindows);
  if (raw.isEmpty) {
    return res;
  }
  try {
    final m = jsonDecode(raw) as Map<String, dynamic>;
    m.forEach((k, v) {
      final display = int.tryParse(k);
      final state = DisplayWindowState.fromJson(v);
      if (display != null && state != null) {
        res[display] = state;
      }
    });
  } catch (e) {
    debugPrint('Failed to parse $kPeerOptionDisplayWindows of $peerId: $e');
  }
  return res;
}

/// Main window isolate only: handler of `kWindowEventSaveDisplayWindow`.
void storeDisplayWindow(String arguments) {
  try {
    final args = jsonDecode(arguments);
    final peerId = args['peer_id'] as String;
    final display = args['display'] as int;
    final state = DisplayWindowState.fromJson(args['state']);
    if (state == null) {
      return;
    }
    final raw = bind.mainGetPeerFlutterOptionSync(
        id: peerId, k: kPeerOptionDisplayWindows);
    Map<String, dynamic> m = {};
    if (raw.isNotEmpty) {
      try {
        m = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {}
    }
    m['$display'] = state.toJson();
    bind.mainSetPeerFlutterOptionSync(
        id: peerId, k: kPeerOptionDisplayWindows, v: jsonEncode(m));
  } catch (e) {
    debugPrint('Failed to store display window "$arguments": $e');
  }
}

/// Returns the frame of a screen in `screens` that equals `rect` within 1 px, else null.
Rect? _findScreen(List<window_size.Screen> screens, Rect rect) {
  for (final s in screens) {
    final f = s.frame;
    if ((f.left - rect.left).abs() <= 1 &&
        (f.top - rect.top).abs() <= 1 &&
        (f.right - rect.right).abs() <= 1 &&
        (f.bottom - rect.bottom).abs() <= 1) {
      return f;
    }
  }
  return null;
}

/// Primary screen: the screen whose frame starts at the virtual-screen origin (Windows), else the first one.
Rect? _primaryScreen(List<window_size.Screen> screens) {
  if (screens.isEmpty) {
    return null;
  }
  for (final s in screens) {
    if (s.frame.left == 0 && s.frame.top == 0) {
      return s.frame;
    }
  }
  return screens.first.frame;
}

/// Returns `frame` moved onto `target`: same offset from the top-left of the screen it was on (centered when it was
/// on none), size capped at `target`'s size, and kept inside `target`.
Rect _moveOntoScreen(
    List<window_size.Screen> screens, Rect frame, Rect target) {
  final w = frame.width < target.width ? frame.width : target.width;
  final h = frame.height < target.height ? frame.height : target.height;
  final from = _screenContaining(screens, frame.center);
  var dx = from == null ? (target.width - w) / 2 : frame.left - from.left;
  var dy = from == null ? (target.height - h) / 2 : frame.top - from.top;
  dx = dx.clamp(0.0, target.width - w);
  dy = dy.clamp(0.0, target.height - h);
  return Rect.fromLTWH(target.left + dx, target.top + dy, w, h);
}

/// Writes `message` to the RustDesk log (`main_display_windows_log`), so window placement can be traced on a user's
/// PC: Flutter `debugPrint` output is not written to the log in a release build.
void displayWindowsLog(String message) {
  try {
    bind.mainDisplayWindowsLog(message: '[display_windows] window=$kWindowId $message');
  } catch (_) {}
}

String _r(Rect? r) => r == null
    ? 'null'
    : '${r.left.toStringAsFixed(0)},${r.top.toStringAsFixed(0)} ${r.width.toStringAsFixed(0)}x${r.height.toStringAsFixed(0)}';

Rect? _screenContaining(List<window_size.Screen> screens, Offset point) {
  for (final s in screens) {
    if (s.frame.contains(point)) {
      return s.frame;
    }
  }
  return null;
}

/// Restore frame of the current window isolate's window, tracked while the window is neither maximized nor
/// fullscreen.
Rect? _normalFrame;

/// Applies `state` to window `windowId`.
///
/// The target screen is `state.screen`. A window without a stored state (`state == null`) or whose target screen no
/// current local screen equals MUST open maximized, never fullscreen, on the primary screen, with the restore frame
/// `kDisplayWindowFallbackSize` at `kDisplayWindowFallbackOffset`.
Future<void> applyDisplayWindowState(
    int windowId, DisplayWindowState? state) async {
  Rect? frame = state?.frame;
  var isMaximized = state?.isMaximized ?? true;
  var isFullscreen = state?.isFullscreen ?? false;
  List<window_size.Screen> screens = [];
  try {
    screens = await window_size.getScreenList();
  } catch (e) {
    debugPrint('Failed to get the screen list: $e');
  }
  final targetExists = state != null &&
      (screens.isEmpty ||
          (state.screen != null &&
              _findScreen(screens, state.screen!) != null));
  if (!targetExists) {
    final primary = _primaryScreen(screens);
    frame = primary == null
        ? null
        : Rect.fromLTWH(
            primary.left + kDisplayWindowFallbackOffset,
            primary.top + kDisplayWindowFallbackOffset,
            kDisplayWindowFallbackSize.width,
            kDisplayWindowFallbackSize.height);
    isMaximized = true;
    isFullscreen = false;
  }
  displayWindowsLog('apply target=$windowId state=${state == null ? null : jsonEncode(state.toJson())} '
      'targetExists=$targetExists frame=${_r(frame)} max=$isMaximized fs=$isFullscreen '
      'screens=${screens.map((s) => _r(s.frame)).toList()}');
  final wc = WindowController.fromWindowId(windowId);
  if (windowId == kWindowId && stateGlobal.fullscreen.isTrue && !isFullscreen) {
    stateGlobal.setFullscreen(false);
  }
  if (frame != null) {
    if (kWindowId == windowId) {
      _normalFrame = frame;
    }
    await wc.setFrame(frame);
  }
  // A delay is needed, or the window is restored after maximize/fullscreen (same as `restoreWindowPosition()`).
  Future.delayed(const Duration(milliseconds: 300), () async {
    if (isFullscreen) {
      if (kWindowId == windowId) {
        stateGlobal.setFullscreen(true);
      } else {
        DesktopMultiWindow.invokeMethod(
            windowId, kWindowEventSetFullscreen, 'true');
      }
    } else if (isMaximized) {
      await wc.maximize();
    }
  });
}

/// One remote session shown in the current window.
class DisplayWindowSession {
  final String peerId;
  final int display;
  final String? viewStyle;
  final int? customScalePercent;
  final double? scrollX;
  final double? scrollY;
  DisplayWindowSession(this.peerId, this.display,
      {this.viewStyle, this.customScalePercent, this.scrollX, this.scrollY});
}

/// Returns every remote session shown in the current window.
typedef DisplayWindowSessions = List<DisplayWindowSession> Function();

DisplayWindowSessions? _sessions;
bool _armed = false;
Timer? _reportTimer;

/// Registered by the remote window page of the current window isolate.
void registerDisplayWindowSessions(DisplayWindowSessions sessions) {
  _sessions = sessions;
}

/// Reports MUST start only after the window reached its restored frame and display: a report taken earlier would
/// store a transient frame under the display that is being replaced.
void armDisplayWindowReports() {
  if (_armed) {
    return;
  }
  Future.delayed(const Duration(milliseconds: 1500), () {
    _armed = true;
    reportDisplayWindowSoon();
  });
}

/// Debounced `reportDisplayWindowNow()`.
void reportDisplayWindowSoon() {
  if (!_armed) {
    return;
  }
  _reportTimer?.cancel();
  _reportTimer =
      Timer(const Duration(milliseconds: 500), () => reportDisplayWindowNow());
}

/// Sends the current window's state for every `(peerId, display)` it shows to the main window isolate.
Future<void> reportDisplayWindowNow() async {
  _reportTimer?.cancel();
  _reportTimer = null;
  final windowId = kWindowId;
  if (!_armed || _sessions == null || windowId == null) {
    return;
  }
  if (stateGlobal.isMinimized) {
    return;
  }
  final wc = WindowController.fromWindowId(windowId);
  final isFullscreen = stateGlobal.fullscreen.isTrue;
  Rect current;
  bool isMaximized;
  try {
    current = await wc.getFrame();
    isMaximized = !isFullscreen && await wc.isMaximized();
  } catch (e) {
    debugPrint('Failed to get the frame of window $windowId, it may be hidden');
    return;
  }
  // A minimized window on Windows reports an offset of -32000.
  if (current.left < -10000 || current.top < -10000) {
    return;
  }
  if (!isFullscreen && !isMaximized) {
    _normalFrame = current;
  }
  List<window_size.Screen> screens = [];
  try {
    screens = await window_size.getScreenList();
  } catch (e) {
    debugPrint('Failed to get the screen list: $e');
  }
  // A maximized / fullscreen window MUST be stored on the screen it is maximized on (`current`): Windows moves a
  // maximized window between screens (Win+Shift+Arrow, snapping) without a normal-state frame event, so the tracked
  // restore frame can still lie on the previous screen. It is moved onto that screen, keeping its offset from its own
  // screen's top-left.
  final currentScreen = (isMaximized || isFullscreen)
      ? _screenContaining(screens, current.center)
      : null;
  if (currentScreen != null &&
      _normalFrame != null &&
      !currentScreen.contains(_normalFrame!.center)) {
    _normalFrame = _moveOntoScreen(screens, _normalFrame!, currentScreen);
  }
  for (final session in _sessions!()) {
    final peerId = session.peerId;
    final display = session.display;
    final stored = loadDisplayWindows(peerId)[display];
    var frame = _normalFrame ?? stored?.frame ?? current;
    if (currentScreen != null && !currentScreen.contains(frame.center)) {
      frame = _moveOntoScreen(screens, frame, currentScreen);
    }
    final screen = currentScreen ??
        _screenContaining(screens, frame.center) ??
        _screenContaining(screens, current.center);
    displayWindowsLog(
        'report window=$windowId peer=$peerId display=$display frame=${_r(frame)} screen=${_r(screen)} '
        'max=$isMaximized fs=$isFullscreen current=${_r(current)} style=${session.viewStyle} '
        'scale=${session.customScalePercent} scroll=${session.scrollX},${session.scrollY}');
    final state = DisplayWindowState(frame, screen, isMaximized, isFullscreen,
        viewStyle: session.viewStyle ?? stored?.viewStyle,
        customScalePercent:
            session.customScalePercent ?? stored?.customScalePercent,
        scrollX: session.scrollX ?? stored?.scrollX,
        scrollY: session.scrollY ?? stored?.scrollY);
    await DesktopMultiWindow.invokeMethod(
        kMainWindowId,
        kWindowEventSaveDisplayWindow,
        jsonEncode({
          'peer_id': peerId,
          'display': display,
          'state': state.toJson(),
        }));
  }
}

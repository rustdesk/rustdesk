import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/relative_mouse_accumulator.dart';
import 'package:get/get.dart';

import '../common.dart';
import '../consts.dart';
import 'platform_model.dart';

class RelativeMouseModel {
  final SessionID sessionId;
  final RxBool enabled;

  final bool Function() keyboardPerm;
  final bool Function() isViewCamera;
  final String Function() peerVersion;
  final String? Function() peerPlatform;

  final Map<String, dynamic> Function(Map<String, dynamic> msg) modify;

  final bool Function() getPointerInsideImage;
  final void Function(bool inside) setPointerInsideImage;

  RelativeMouseModel({
    required this.sessionId,
    required this.enabled,
    required this.keyboardPerm,
    required this.isViewCamera,
    required this.peerVersion,
    required this.peerPlatform,
    required this.modify,
    required this.getPointerInsideImage,
    required this.setPointerInsideImage,
  });

  final RelativeMouseAccumulator _accumulator = RelativeMouseAccumulator();

  // Native relative mouse mode support (macOS only)
  // Uses CGAssociateMouseAndMouseCursorPosition to lock cursor and NSEvent monitor for raw delta.
  static MethodChannel? _hostChannel;
  // The currently active model receiving native mouse delta events.
  // Note: Race condition between multiple sessions is not a concern here because
  // when relative mouse mode is active, the cursor is locked and the user cannot
  // switch to another session window. The user must first exit relative mouse mode
  // (via Cmd+G on macOS or Ctrl+Alt on Windows/Linux) before they can interact
  // with a different session.
  static RelativeMouseModel? _activeNativeModel;
  static bool _hostChannelInitialized = false;

  /// Initialize the host channel for native relative mouse mode.
  /// This should be called once when the app starts on macOS.
  static void initHostChannel() {
    if (!isMacOS) return;
    if (_hostChannelInitialized) return;
    _hostChannelInitialized = true;

    _hostChannel = const MethodChannel('org.rustdesk.rustdesk/host');
    _hostChannel!.setMethodCallHandler((call) async {
      if (call.method == 'onMouseDelta') {
        final args = call.arguments as Map<dynamic, dynamic>;
        final dx = args['dx'] as int;
        final dy = args['dy'] as int;
        _activeNativeModel?._onNativeMouseDelta(dx, dy);
      }
      return null;
    });
  }

  // TODO(perf): Consider routing native delta through RelativeMouseAccumulator/throttle
  // if high-polling mice (e.g. 1000Hz+) cause message flooding on the network.
  void _onNativeMouseDelta(int dx, int dy) {
    if (!enabled.value) return;
    // Send directly to remote without accumulator (native already provides integer deltas)
    _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$dx',
      'y': '$dy',
    });
  }

  Future<bool> _enableNativeRelativeMouseMode() async {
    if (!isMacOS) return false;
    if (_hostChannel == null) {
      initHostChannel();
      if (_hostChannel == null) return false;
    }

    // Defensive guard: prevent overwriting an already-active native session.
    // In practice, this should not happen because when relative mouse mode is active,
    // the cursor is locked and the user cannot switch to another session window.
    // The user must first exit relative mouse mode (via Cmd+G on macOS or Ctrl+Alt on
    // Windows/Linux) before interacting with a different session.
    if (_activeNativeModel != null && _activeNativeModel != this) {
      debugPrint(
          '[RelMouse] Another model already has native relative mouse mode active');
      return false;
    }

    try {
      final result =
          await _hostChannel!.invokeMethod('enableNativeRelativeMouseMode');
      if (result == true) {
        _activeNativeModel = this;
        return true;
      }
    } catch (e) {
      debugPrint('[RelMouse] Failed to enable native relative mouse mode: $e');
    }
    return false;
  }

  Future<void> _disableNativeRelativeMouseMode() async {
    if (!isMacOS) return;
    if (_hostChannel == null) return;

    // Only the owning model should disable native mode to avoid
    // one session inadvertently disrupting another's native relative mouse state.
    if (_activeNativeModel != this) {
      return;
    }

    try {
      await _hostChannel!.invokeMethod('disableNativeRelativeMouseMode');
    } catch (e) {
      debugPrint('[RelMouse] Failed to disable native relative mouse mode: $e');
    } finally {
      if (_activeNativeModel == this) {
        _activeNativeModel = null;
      }
    }
  }

  // Whether native relative mouse mode is currently active for this model
  bool get _isNativeRelativeMouseModeActive =>
      isMacOS && _activeNativeModel == this;

  // Pointer lock center in LOCAL widget coordinates (for delta calculation)
  Offset? _pointerLockCenterLocal;
  // Pointer lock center in SCREEN coordinates (for OS cursor re-centering)
  Offset? _pointerLockCenterScreen;
  // Pointer region top-left in Flutter view coordinates.
  // Computed from PointerEvent.position - PointerEvent.localPosition.
  Offset? _pointerRegionTopLeftGlobal;
  // Last pointer position in LOCAL widget coordinates (fallback when center is not ready).
  Offset? _lastPointerLocalPos;

  // Track whether we currently have an OS-level cursor clip active (Windows only).
  // TODO(accuracy): Revisit window/client/border clipping math if users report misaligned
  // clipping on custom or maximized window decorations. Consider using platform APIs
  // (e.g. GetClientRect on Windows) instead of Flutter's window coordinates.
  bool _cursorClipApplied = false;

  // Track whether a recenter operation is in progress to prevent overlapping calls.
  bool _recenterInProgress = false;

  // Request token for async enable operation to prevent stale callbacks.
  // Incremented on each enable attempt, callbacks check if token still matches.
  int _enableRequestId = 0;

  // Throttle buffer for batching mouse move messages (reduces network flooding).
  int _pendingDeltaX = 0;
  int _pendingDeltaY = 0;
  Timer? _throttleTimer;
  static const Duration _throttleInterval = Duration(milliseconds: 16);

  // Size of the remote image widget (for center calculation)
  Size? _imageWidgetSize;

  // Debounce timestamp for relative mouse mode toggle to prevent race conditions
  // between Rust rdev grab loop and Flutter keyboard handling.
  DateTime? _lastToggle;

  // Track key down state for exit shortcut.
  // macOS: Cmd+G - track G key
  // Windows/Linux: Ctrl+Alt - track whichever modifier was pressed last
  // When key down is blocked (shortcut triggered), we also need to block
  // the corresponding key up to avoid orphan key up events being sent to remote.
  bool _exitShortcutKeyDown = false;

  // Callback to cancel external throttle timer when relative mouse mode is disabled.
  VoidCallback? onDisabled;

  bool get isSupported {
    // On Linux/Wayland, cursor warping is not supported, hide the option entirely.
    if (isDesktop && isLinux && bind.mainCurrentIsWayland()) {
      return false;
    }
    // Relative mouse mode is unsupported on remote Linux:
    // 1. Long-press key events are unsupported.
    // 2. The Wayland display server lacks cursor warping support.
    final platform = peerPlatform();
    if (platform == kPeerPlatformLinux) {
      return false;
    }
    final v = peerVersion();
    if (v.isEmpty) return false;
    return versionCmp(v, kMinVersionForRelativeMouseMode) >= 0;
  }

  Size? get imageWidgetSize => _imageWidgetSize;

  void updateImageWidgetSize(Size size) {
    _imageWidgetSize = size;
    if (enabled.value) {
      _pointerLockCenterLocal = Offset(size.width / 2, size.height / 2);
    }
  }

  void updatePointerRegionTopLeftGlobal(PointerEvent e) {
    _pointerRegionTopLeftGlobal = e.position - e.localPosition;
  }

  /// Shared helper for handling exit shortcut for relative mouse mode.
  /// Returns true if the event was handled and should not be forwarded.
  ///
  /// Exit shortcuts (only work when relative mouse mode is active):
  /// - macOS: Cmd+G
  /// - Windows/Linux: Ctrl+Alt (any order - triggered when both are pressed)
  ///
  /// [logicalKey] - the logical key of the event
  /// [isKeyUp] - whether the event is a key up event
  /// [isKeyDown] - whether the event is a key down event
  /// [ctrlPressed], [altPressed], [commandPressed] - modifier states
  bool _handleExitShortcut({
    required LogicalKeyboardKey logicalKey,
    required bool isKeyUp,
    required bool isKeyDown,
    required bool ctrlPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    if (!isDesktop || !keyboardPerm() || isViewCamera()) return false;

    // Only handle exit shortcuts when relative mouse mode is active
    if (!enabled.value) return false;

    // Block key up if key down was blocked (to avoid orphan key up event on remote).
    if (isKeyUp && _exitShortcutKeyDown) {
      _exitShortcutKeyDown = false;
      return true;
    }

    if (!isKeyDown) return false;

    // macOS: Cmd+G to exit
    if (isMacOS) {
      final isGKey = logicalKey == LogicalKeyboardKey.keyG;
      if (isGKey && commandPressed) {
        _exitShortcutKeyDown = true;
        setRelativeMouseMode(false);
        return true;
      }
      return false;
    }

    // Windows/Linux: Ctrl+Alt to exit
    // Triggered when both modifiers are pressed (check on either Ctrl or Alt key down)
    final isCtrlKey = logicalKey == LogicalKeyboardKey.controlLeft ||
        logicalKey == LogicalKeyboardKey.controlRight;
    final isAltKey = logicalKey == LogicalKeyboardKey.altLeft ||
        logicalKey == LogicalKeyboardKey.altRight;

    // When Ctrl is pressed and Alt is already down, or vice versa
    if ((isCtrlKey && altPressed) || (isAltKey && ctrlPressed)) {
      _exitShortcutKeyDown = true;
      setRelativeMouseMode(false);
      return true;
    }

    return false;
  }

  bool handleKeyEvent(
    KeyEvent e, {
    required bool ctrlPressed,
    required bool shiftPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is KeyUpEvent,
      isKeyDown: e is KeyDownEvent,
      ctrlPressed: ctrlPressed,
      altPressed: altPressed,
      commandPressed: commandPressed,
    );
  }

  /// Handle raw key events for relative mouse mode.
  /// Returns true if the event was handled and should not be forwarded.
  bool handleRawKeyEvent(RawKeyEvent e) {
    final modifiers = e.data;
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is RawKeyUpEvent,
      isKeyDown: e is RawKeyDownEvent,
      ctrlPressed: modifiers.isControlPressed,
      altPressed: modifiers.isAltPressed,
      commandPressed: modifiers.isMetaPressed,
    );
  }

  void onEnterOrLeaveImage(bool enter) {
    if (!enabled.value) return;

    // Keep the shared pointer-in-image flag in sync.
    setPointerInsideImage(enter);

    // macOS native mode: cursor is locked by CGAssociateMouseAndMouseCursorPosition,
    // no need for recenter logic.
    if (_isNativeRelativeMouseModeActive) {
      return;
    }

    if (!enter) {
      _releaseCursorClip();
      return;
    }

    // Windows: clip cursor to window rect
    // Linux: use recenter method
    updatePointerLockCenter().then((_) {
      _recenterMouse();
    });
  }

  void onWindowBlur() {
    if (!enabled.value) return;

    // Focus can change while the pointer is outside the window (e.g. taskbar activation).
    // Do not rely on the previous "pointer inside" state across focus boundaries.
    setPointerInsideImage(false);
    // macOS native mode: don't call _releaseCursorClip as it would break CGAssociateMouseAndMouseCursorPosition
    if (!_isNativeRelativeMouseModeActive) {
      _releaseCursorClip();
    }
  }

  void onWindowFocus() {
    if (!enabled.value) return;

    // macOS native mode: cursor is already locked
    if (_isNativeRelativeMouseModeActive) {
      setPointerInsideImage(false);
      return;
    }

    // Guard: image widget size must be available for proper center calculation.
    if (_imageWidgetSize == null) {
      _disableWithCleanup();
      return;
    }

    // Fail-safe: keep cursor usable on focus gain. Pointer lock will be re-engaged
    // on the next pointer enter/move/hover inside the remote image.
    setPointerInsideImage(false);
    _releaseCursorClip();

    // Best-effort: refresh center so the next engage is immediate.
    updatePointerLockCenter();
  }

  void toggleRelativeMouseMode() {
    final now = DateTime.now();
    if (_lastToggle != null &&
        now.difference(_lastToggle!).inMilliseconds <
            kRelativeMouseModeToggleDebounceMs) {
      return;
    }
    _lastToggle = now;
    setRelativeMouseMode(!enabled.value);
  }

  bool setRelativeMouseMode(bool value) {
    // Web is not supported due to Pointer Lock API integration complexity with Flutter's input system
    if (isWeb) {
      return false;
    }

    if (value) {
      if (!keyboardPerm() || isViewCamera()) {
        return false;
      }

      if (isDesktop && _imageWidgetSize == null) {
        // Desktop only: Ensure image widget size is available for proper center calculation.
        showToast(translate('rel-mouse-not-ready-tip'));
        return false;
      }

      if (!isSupported) {
        // Check server version support before enabling.
        showToast(translate('rel-mouse-not-supported-peer-tip'));
        return false;
      }
    }

    if (value) {
      try {
        if (isDesktop) {
          final requestId = ++_enableRequestId;
          if (isMacOS) {
            // macOS: Use native relative mouse mode with CGAssociateMouseAndMouseCursorPosition
            // This locks the cursor in place and provides raw delta via NSEvent monitor.
            _enableNativeRelativeMouseMode().then((success) {
              // Guard against stale callback: user may have toggled off relative mode
              // while the async enable was in progress.
              if (_enableRequestId != requestId) {
                return;
              }
              if (success) {
                _completeEnableRelativeMouseMode();
              }
              // Note: _enableNativeRelativeMouseMode already handles its own cleanup on failure
            });
          } else {
            // Windows/Linux: Use Flutter-based cursor recenter approach
            if (!getPointerInsideImage()) {
              _releaseCursorClip();
            }

            updatePointerLockCenter().then((_) => _recenterMouse()).then((_) {
              if (_enableRequestId != requestId) {
                return;
              }
              _completeEnableRelativeMouseMode();
            }).catchError((e) {
              if (_enableRequestId != requestId) {
                return;
              }
              debugPrint('[RelMouse] Platform setup failed: $e');
              _resetState();
            });
          }
        } else {
          // Mobile: enable immediately (no platform-specific setup needed)
          _completeEnableRelativeMouseMode();
        }
      } catch (e) {
        _disableWithCleanup();
        return false;
      }
    } else {
      // Best-effort marker for Rust rdev grab loop (ESC behavior).
      // Bypass keyboardPerm check to ensure Rust state is always synced,
      // even if permission was revoked while relative mode was active.
      _sendMouseMessageToSession(
        {
          'relative_mouse_mode': '0',
        },
        disableRelativeOnError: false,
        bypassKeyboardPerm: true,
      );

      // Desktop only: cursor manipulation
      if (isDesktop) {
        if (isMacOS) {
          // macOS: Disable native relative mouse mode
          // This already calls CGAssociateMouseAndMouseCursorPosition(1) to re-associate mouse
          _disableNativeRelativeMouseMode();
        } else {
          _releaseCursorClip();
        }
      }
      enabled.value = false;
      _resetState();
      onDisabled?.call();
    }

    return true;
  }

  /// Called when platform setup completes successfully to finalize enabling relative mouse mode.
  void _completeEnableRelativeMouseMode() {
    enabled.value = true;

    // Show toast notification so user knows how to exit relative mouse mode (desktop only).
    if (isDesktop) {
      showToast(
          translate('rel-mouse-exit-{${isMacOS ? "Cmd+G" : "Ctrl+Alt"}}-tip'),
          alignment: Alignment.center);
    }

    // Best-effort marker for Rust rdev grab loop (ESC behavior) and peer/server state.
    // This uses a no-op delta so it does not move the remote cursor.
    // Intentionally fire-and-forget: we don't block enabling on this marker message.
    // Failures are logged but do not disable relative mouse mode.
    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '1',
        'type': 'move_relative',
        'x': '0',
        'y': '0',
      },
      disableRelativeOnError: false,
    ).catchError((e) {
      debugPrint('[RelMouse] Failed to send enable marker: $e');
      return false;
    });
  }

  // Flag to skip the first mouse move event after recenter (it's the recenter itself).
  bool _skipNextMouseMove = false;

  /// Handle relative mouse movement based on current local pointer position.
  /// Returns true if the event was handled in relative mode, false otherwise.
  bool handleRelativeMouseMove(Offset localPosition) {
    if (!enabled.value) return false;

    // macOS: Native mode handles delta via callback, skip Flutter-based handling.
    if (_isNativeRelativeMouseModeActive) {
      return true;
    }

    // Pointer move/hover implies we're inside the remote image.
    _ensurePointerLockEngaged();

    // Skip the mouse move event triggered by recenter operation itself.
    if (_skipNextMouseMove) {
      _skipNextMouseMove = false;
      _lastPointerLocalPos = localPosition;
      return true;
    }

    final lastLocal = _lastPointerLocalPos;
    _lastPointerLocalPos = localPosition;

    // Linux-specific: Proactive recenter check before processing delta.
    // On Linux, we don't have clip_cursor, so if the cursor moves too fast
    // it may escape the window before _recenterIfNearEdge can catch it.
    // Check now and recenter immediately if needed.
    if (isLinux) {
      _recenterIfNearEdgeLinux(localPosition);
    }

    // Calculate delta from last position (not from center).
    // This avoids issues with CGWarpMouseCursorPosition integer rounding.
    if (lastLocal != null) {
      final delta = localPosition - lastLocal;
      if (delta.dx != 0 || delta.dy != 0) {
        sendRelativeMouseMove(delta.dx, delta.dy);
      }
    }

    return true;
  }

  /// Linux-specific: More aggressive recenter check to prevent cursor escape.
  /// Called synchronously before processing mouse delta to ensure cursor stays within bounds.
  void _recenterIfNearEdgeLinux(Offset localPosition) {
    final size = _imageWidgetSize;
    if (size == null) return;

    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = localPosition.dx < edgeThreshold;
    final nearRight = localPosition.dx > size.width - edgeThreshold;
    final nearTop = localPosition.dy < edgeThreshold;
    final nearBottom = localPosition.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  void sendRelativeMouseMove(double dx, double dy) {
    if (!isDesktop) return;

    final delta = _accumulator.add(dx, dy, maxDelta: kMaxRelativeMouseDelta);
    if (delta == null) return;

    // Buffer the delta for throttled sending.
    _pendingDeltaX += delta.x;
    _pendingDeltaY += delta.y;

    // Start or refresh the throttle timer.
    if (_throttleTimer == null || !_throttleTimer!.isActive) {
      _throttleTimer = Timer(_throttleInterval, () => _flushPendingDelta());
    }
  }

  Future<void> _flushPendingDelta() async {
    if (!isDesktop) return;
    if (_pendingDeltaX == 0 && _pendingDeltaY == 0) return;

    final x = _pendingDeltaX;
    final y = _pendingDeltaY;
    _pendingDeltaX = 0;
    _pendingDeltaY = 0;

    final ok = await _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$x',
      'y': '$y',
    });
    if (!ok) return;

    // Only recenter when mouse is near the edge of the image widget.
    // This allows smooth mouse movement without constant recentering.
    _recenterIfNearEdge();
  }

  // Edge threshold parameters for recenter detection.
  // Threshold is calculated as: min(maxThreshold, min(width, height) * fraction)
  static const double _edgeThresholdFraction = 0.1; // 10% of smaller dimension
  static const double _edgeThresholdMax =
      100.0; // Maximum threshold in logical pixels
  static const double _edgeThresholdMin =
      20.0; // Minimum threshold for very small widgets

  // Linux-specific edge threshold parameters (more aggressive to prevent cursor escape).
  // On Linux, we don't have clip_cursor capability, so we need to recenter earlier
  // to prevent the cursor from escaping the window when moving fast.
  static const double _edgeThresholdFractionLinux =
      0.25; // 25% of smaller dimension
  static const double _edgeThresholdMaxLinux =
      200.0; // Larger maximum threshold for Linux
  static const double _edgeThresholdMinLinux =
      50.0; // Larger minimum threshold for Linux

  /// Calculate dynamic edge threshold based on widget size.
  double _calculateEdgeThreshold(Size size) {
    final smallerDimension = math.min(size.width, size.height);
    if (isLinux) {
      // Use more aggressive thresholds on Linux to prevent cursor escape.
      final dynamicThreshold = smallerDimension * _edgeThresholdFractionLinux;
      return dynamicThreshold.clamp(
          _edgeThresholdMinLinux, _edgeThresholdMaxLinux);
    }
    final dynamicThreshold = smallerDimension * _edgeThresholdFraction;
    // Clamp between min and max thresholds
    return dynamicThreshold.clamp(_edgeThresholdMin, _edgeThresholdMax);
  }

  /// Recenter the cursor only if it's near the edge of the image widget.
  void _recenterIfNearEdge() {
    final lastPos = _lastPointerLocalPos;
    final size = _imageWidgetSize;
    if (lastPos == null || size == null) return;

    // Dynamic threshold based on widget size
    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = lastPos.dx < edgeThreshold;
    final nearRight = lastPos.dx > size.width - edgeThreshold;
    final nearTop = lastPos.dy < edgeThreshold;
    final nearBottom = lastPos.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  /// Send mouse button event without position (for relative mouse mode).
  Future<void> sendRelativeMouseButton(Map<String, dynamic> evt) async {
    if (!enabled.value) return;
    _ensurePointerLockEngaged();

    final rawType = evt['type'];
    final rawButtons = evt['buttons'];
    if (rawType is! String || rawButtons is! int) return;

    final type = _mouseEventTypeToPeer(rawType);
    if (type.isEmpty) return;

    final buttons = mouseButtonsToPeer(rawButtons);
    if (buttons.isEmpty) return;

    await _sendMouseMessageToSession({
      'type': type,
      'buttons': buttons,
    });
  }

  static String _mouseEventTypeToPeer(String type) {
    switch (type) {
      case 'mousedown':
        return kMouseEventTypeDown;
      case 'mouseup':
        return kMouseEventTypeUp;
      default:
        return '';
    }
  }

  Future<bool> _sendMouseMessageToSession(
    Map<String, dynamic> msg, {
    bool disableRelativeOnError = true,
    bool bypassKeyboardPerm = false,
  }) async {
    if (!bypassKeyboardPerm && !keyboardPerm()) return false;
    if (isViewCamera()) return false;

    try {
      await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify(msg)),
      );
      return true;
    } catch (e) {
      debugPrint('[RelMouse] Error sending mouse message: $e');
      if (disableRelativeOnError && enabled.value) {
        _disableWithCleanup();
      }
      return false;
    }
  }

  /// Retry parameters for cursor re-centering.
  static const int _recenterMaxRetries = 3;
  static const Duration _recenterRetryDelay = Duration(milliseconds: 100);

  /// Recenter the cursor to the pointer lock center.
  /// Fire-and-forget safe: prevents overlapping calls and catches errors internally.
  Future<void> _recenterMouse() async {
    // Prevent overlapping recenter operations under high-frequency mouse moves.
    if (_recenterInProgress) return;
    _recenterInProgress = true;

    try {
      if (!enabled.value) return;
      if (!getPointerInsideImage()) return;

      final center = _pointerLockCenterScreen;
      if (center == null) {
        return;
      }

      for (int attempt = 0; attempt < _recenterMaxRetries; attempt++) {
        // Check preconditions before each attempt.
        if (!enabled.value || !getPointerInsideImage()) return;

        final ok = bind.mainSetCursorPosition(
          x: center.dx.toInt(),
          y: center.dy.toInt(),
        );
        if (ok) {
          // Skip the next mouse move event - it's triggered by the recenter itself.
          _skipNextMouseMove = true;
          return;
        }

        // Wait before retrying (except on the last attempt).
        if (attempt < _recenterMaxRetries - 1) {
          await Future.delayed(_recenterRetryDelay);
        }
      }

      // All attempts failed.
      _disableWithCleanup();
      showToast(translate('rel-mouse-lock-failed-tip'));
    } catch (e, st) {
      debugPrint('[RelMouse] Unexpected error in _recenterMouse: $e\n$st');
    } finally {
      _recenterInProgress = false;
    }
  }

  Future<void> updatePointerLockCenter({Offset? localCenter}) async {
    if (!isDesktop) return;

    // Null safety check for kWindowId.
    if (kWindowId == null) {
      if (enabled.value) {
        _disableWithCleanup();
      }
      return;
    }

    try {
      final wc = WindowController.fromWindowId(kWindowId!);
      final frame = await wc.getFrame();

      if (frame.width <= 0 || frame.height <= 0) {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      if (localCenter != null) {
        _pointerLockCenterLocal = localCenter;
      } else if (_imageWidgetSize != null) {
        _pointerLockCenterLocal = Offset(
          _imageWidgetSize!.width / 2,
          _imageWidgetSize!.height / 2,
        );
      } else {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      // Calculate screen coordinates for OS cursor positioning.
      // Use PlatformDispatcher instead of deprecated ui.window.
      final view = ui.PlatformDispatcher.instance.views.firstOrNull;
      if (view == null) {
        debugPrint('[RelMouse] No view available for coordinate calculation');
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }
      final scale = view.devicePixelRatio;

      if (_pointerRegionTopLeftGlobal != null && scale > 0) {
        // On macOS, window frame and CGWarpMouseCursorPosition use points (not pixels).
        // On Windows, they use pixels.
        // Flutter's logical coordinates are in points on macOS.
        final centerInView =
            _pointerRegionTopLeftGlobal! + _pointerLockCenterLocal!;

        // Calculate client area offset (excluding title bar and borders)
        final clientPhysical = view.physicalSize;

        // macOS: Window frame and CGWarpMouseCursorPosition both use points (not pixels).
        // We convert clientPhysical (pixels) to points via `/ scale` to compute titleBarHeight,
        // which is the difference between the total window height and the Flutter view height.
        if (isMacOS) {
          final clientHeightPoints = clientPhysical.height / scale;
          final titleBarHeight = frame.height - clientHeightPoints;

          _pointerLockCenterScreen = Offset(
            frame.left + centerInView.dx,
            frame.top + titleBarHeight + centerInView.dy,
          );
        } else {
          // Windows/Linux: Use pixel coordinates. We estimate the client-area offset using
          // a heuristic based on the difference between frame size and client physical size.
          // This assumes symmetric horizontal borders (extraW / 2) and that the remaining
          // vertical space (extraH - borderBottom) is the title bar height.
          // Limitation: This heuristic may be inaccurate for maximized windows, custom window
          // decorations, or when the OS uses different border styles.
          // TODO: Replace this heuristic with platform API calls (e.g., GetClientRect on Windows)
          //       if precise client-area offsets are required.
          final extraW = frame.width - clientPhysical.width;
          final extraH = frame.height - clientPhysical.height;
          final borderX = extraW > 0 ? extraW / 2 : 0.0;
          final borderBottom = borderX;
          final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;
          final clientTopLeftScreen =
              Offset(frame.left + borderX, frame.top + borderTop);

          // Calculate tentative center, then validate it's within frame bounds.
          // This guards against heuristic inaccuracies (e.g., maximized windows).
          final tentativeCenter = Offset(
            clientTopLeftScreen.dx + centerInView.dx * scale,
            clientTopLeftScreen.dy + centerInView.dy * scale,
          );
          final withinFrame = tentativeCenter.dx >= frame.left &&
              tentativeCenter.dx <= frame.left + frame.width &&
              tentativeCenter.dy >= frame.top &&
              tentativeCenter.dy <= frame.top + frame.height;
          _pointerLockCenterScreen = withinFrame
              ? tentativeCenter
              : Offset(
                  frame.left + frame.width / 2, frame.top + frame.height / 2);
        }
      } else {
        _pointerLockCenterScreen = Offset(
          frame.left + frame.width / 2,
          frame.top + frame.height / 2,
        );
      }

      if (enabled.value && isWindows && getPointerInsideImage()) {
        _applyCursorClipForFrame(frame);
      } else if (enabled.value && isWindows && _cursorClipApplied) {
        // Only release if we actually have a clip applied to avoid redundant FFI calls.
        _releaseCursorClip();
      }
      // macOS: no clip_cursor (CGAssociateMouseAndMouseCursorPosition stops mouse events)
      // Instead, we use recenter method like other platforms.
    } catch (e) {
      if (enabled.value) {
        _disableWithCleanup();
      } else {
        _pointerLockCenterLocal = null;
        _pointerLockCenterScreen = null;
      }
    }
  }

  void _ensurePointerLockEngaged() {
    if (!enabled.value) return;
    if (!isDesktop) return;

    setPointerInsideImage(true);

    final needsCenter =
        _pointerLockCenterLocal == null || _pointerLockCenterScreen == null;
    // Windows only: cursor clip
    final needsClip = isWindows && !_cursorClipApplied;
    if (needsCenter || needsClip) {
      updatePointerLockCenter()
          .then((_) => _recenterMouse())
          .catchError((Object e, StackTrace st) {
        debugPrint('[RelMouse] updatePointerLockCenter failed: $e\n$st');
        _disableWithCleanup();
      });
    }
  }

  void _applyCursorClipForFrame(Rect frame) {
    if (!isWindows) return;

    // Use PlatformDispatcher to get the device pixel ratio for proper scaling.
    final view = ui.PlatformDispatcher.instance.views.firstOrNull;
    final scale = view?.devicePixelRatio ?? 1.0;

    // Get the Flutter view's physical size (client area in pixels).
    final clientPhysical = view?.physicalSize ?? ui.Size.zero;

    // Calculate the non-client area (OS window title bar, borders).
    // frame includes the entire window (title bar + borders + client area).
    final extraW = frame.width - clientPhysical.width;
    final extraH = frame.height - clientPhysical.height;

    // Assume symmetric horizontal borders.
    final borderX = extraW > 0 ? extraW / 2 : 0.0;
    // Bottom border is typically the same as side borders.
    final borderBottom = borderX;
    // OS window title bar height is the remaining vertical non-client space.
    final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;

    // Calculate client area top-left in screen coordinates.
    final clientTopLeftScreen =
        Offset(frame.left + borderX, frame.top + borderTop);

    int left, top, right, bottom;

    // If we have precise image widget info, clip to the remote image area.
    // This excludes the Flutter app's internal title bar and toolbar.
    if (_pointerRegionTopLeftGlobal != null &&
        _imageWidgetSize != null &&
        scale > 0) {
      // _pointerRegionTopLeftGlobal is in Flutter logical coordinates (relative to client area).
      // Convert to screen physical coordinates.
      left = (clientTopLeftScreen.dx + _pointerRegionTopLeftGlobal!.dx * scale)
          .toInt();
      top = (clientTopLeftScreen.dy + _pointerRegionTopLeftGlobal!.dy * scale)
          .toInt();
      right = (left + _imageWidgetSize!.width * scale).toInt();
      bottom = (top + _imageWidgetSize!.height * scale).toInt();
    } else {
      // Fallback: clip to client area (excluding OS window decorations).
      left = clientTopLeftScreen.dx.toInt();
      top = clientTopLeftScreen.dy.toInt();
      right = (frame.left + frame.width - borderX).toInt();
      bottom = (frame.top + frame.height - borderBottom).toInt();
    }

    _cursorClipApplied = bind.mainClipCursor(
      left: left,
      top: top,
      right: right,
      bottom: bottom,
      enable: true,
    );
  }

  void _releaseCursorClip() {
    if (!_cursorClipApplied) return;
    _cursorClipApplied = false;
    if (!isWindows) return;

    bind.mainClipCursor(
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
      enable: false,
    );
  }

  void _resetState() {
    // Flush any pending delta before clearing state.
    // This ensures the last buffered movement is sent before values are zeroed.
    // Fire-and-forget: we don't wait for the async send to complete.
    if (_throttleTimer != null || _pendingDeltaX != 0 || _pendingDeltaY != 0) {
      _throttleTimer?.cancel();
      _throttleTimer = null;
      if (_pendingDeltaX != 0 || _pendingDeltaY != 0) {
        final x = _pendingDeltaX;
        final y = _pendingDeltaY;
        _pendingDeltaX = 0;
        _pendingDeltaY = 0;
        // Send without awaiting; skip recenter since we're disabling.
        _sendMouseMessageToSession({
          'type': 'move_relative',
          'x': '$x',
          'y': '$y',
        }, disableRelativeOnError: false);
      }
    }
    _accumulator.reset();
    _pointerLockCenterLocal = null;
    _pointerLockCenterScreen = null;
    _pointerRegionTopLeftGlobal = null;
    _lastPointerLocalPos = null;
    _skipNextMouseMove = false;
    setPointerInsideImage(false);
    _cursorClipApplied = false;
    _exitShortcutKeyDown = false;
  }

  /// Core cleanup logic shared by [_disableWithCleanup] and [dispose].
  /// Sends disable message to Rust, releases platform resources, and resets state.
  void _performCleanupCore() {
    // Best-effort marker for Rust rdev grab loop (ESC behavior).
    // Bypass keyboardPerm check to ensure Rust state is always synced.
    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '0',
      },
      disableRelativeOnError: false,
      bypassKeyboardPerm: true,
    );

    // macOS: Disable native relative mouse mode
    // This already calls CGAssociateMouseAndMouseCursorPosition(1) to re-associate mouse
    if (isMacOS) {
      _disableNativeRelativeMouseMode();
    } else {
      _releaseCursorClip();
    }

    _resetState();
  }

  void _disableWithCleanup() {
    _performCleanupCore();
    enabled.value = false;
    onDisabled?.call();
  }

  bool _disposed = false;

  void dispose() {
    if (_disposed) return;
    _disposed = true;

    _performCleanupCore();
    _imageWidgetSize = null;
    _lastToggle = null;
    // Set enabled to false BEFORE calling onDisabled, consistent with _disableWithCleanup().
    enabled.value = false;
    // Trigger callback before clearing it, so external cleanup can run.
    onDisabled?.call();
    onDisabled = null;
  }
}

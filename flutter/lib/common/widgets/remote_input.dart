import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';

import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/input_model.dart';

import './gestures.dart';

class RawKeyFocusScope extends StatelessWidget {
  final FocusNode? focusNode;
  final ValueChanged<bool>? onFocusChange;
  final InputModel inputModel;
  final Widget child;

  RawKeyFocusScope({
    this.focusNode,
    this.onFocusChange,
    required this.inputModel,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    // https://github.com/flutter/flutter/issues/154053
    final useRawKeyEvents = isLinux && !isWeb;
    // FIXME: On Windows, `AltGr` will generate `Alt` and `Control` key events,
    // while `Alt` and `Control` are separated key events for en-US input method.
    return FocusScope(
        autofocus: true,
        child: Focus(
            autofocus: true,
            canRequestFocus: true,
            focusNode: focusNode,
            onFocusChange: onFocusChange,
            onKey: useRawKeyEvents
                ? (FocusNode data, RawKeyEvent event) =>
                    inputModel.handleRawKeyEvent(event)
                : null,
            onKeyEvent: useRawKeyEvents
                ? null
                : (FocusNode node, KeyEvent event) =>
                    inputModel.handleKeyEvent(event),
            child: child));
  }
}

// For virtual mouse when using the mouse mode on mobile.
// Special hold-drag mode: one finger holds a button (left/right button), another finger pans.
// This flag is to override the scale gesture to a pan gesture.
bool isSpecialHoldDragActive = false;
// Cache the last focal point to calculate deltas in special hold-drag mode.
Offset _lastSpecialHoldDragFocalPoint = Offset.zero;

class RawTouchGestureDetectorRegion extends StatefulWidget {
  final Widget child;
  final FFI ffi;
  final bool isCamera;
  late final InputModel inputModel = ffi.inputModel;
  late final FfiModel ffiModel = ffi.ffiModel;

  RawTouchGestureDetectorRegion({
    required this.child,
    required this.ffi,
    this.isCamera = false,
  });

  @override
  State<RawTouchGestureDetectorRegion> createState() =>
      _RawTouchGestureDetectorRegionState();
}

/// touchMode only:
///   LongPress -> right click
///   OneFingerPan -> start/end -> left down start/end
///   onDoubleTapDown -> move to
///   onLongPressDown => move to
///
/// mouseMode only:
///   DoubleFiner -> right click
///   HoldDrag -> left drag
class _RawTouchGestureDetectorRegionState
    extends State<RawTouchGestureDetectorRegion> {
  Offset _cacheLongPressPosition = Offset(0, 0);
  // Timestamp of the last long press event.
  int _cacheLongPressPositionTs = 0;
  double _mouseScrollIntegral = 0; // mouse scroll speed controller
  Timer? _scrollPacerTimer;
  double _scrollVelocity = 0;
  bool _isScrollDragging = false;
  int _lastScrollUpdateMs = 0;
  double _appSwitchDeltaX = 0;
  bool _isMultiFingerTouch = false;
  double _scale = 1;

  // Workaround tap down event when two fingers are used to scale(mobile)
  TapDownDetails? _lastTapDownDetails;

  PointerDeviceKind? lastDeviceKind;

  // For touch mode, onDoubleTap
  // `onDoubleTap()` does not provide the position of the tap event.
  Offset _lastPosOfDoubleTapDown = Offset.zero;
  bool _touchModePanStarted = false;
  Offset _doubleFinerTapPosition = Offset.zero;

  // For mouse mode, we need to block the events when the cursor is in a blocked area.
  // So we need to cache the last tap down position.
  Offset? _lastTapDownPositionForMouseMode;
  // Cache global position for onTap (which lacks position info).
  Offset? _lastTapDownGlobalPosition;

  FFI get ffi => widget.ffi;
  FfiModel get ffiModel => widget.ffiModel;
  InputModel get inputModel => widget.inputModel;
  bool get handleTouch => (isDesktop || isWebDesktop) || ffiModel.touchMode;
  SessionID get sessionId => ffi.sessionId;
  bool get canvasLocked => isMobile && ffi.canvasModel.locked;

  @override
  Widget build(BuildContext context) {
    return RawGestureDetector(
      child: widget.child,
      gestures: makeGestures(context),
    );
  }

  bool isNotTouchBasedDevice() {
    return !kTouchBasedDeviceKinds.contains(lastDeviceKind);
  }

  // Mobile, mouse mode.
  // Check if should block the mouse tap event (`_lastTapDownPositionForMouseMode`).
  bool shouldBlockMouseModeEvent() {
    return _lastTapDownPositionForMouseMode != null &&
        ffi.cursorModel.shouldBlock(_lastTapDownPositionForMouseMode!.dx,
            _lastTapDownPositionForMouseMode!.dy);
  }

  onTapDown(TapDownDetails d) async {
    lastDeviceKind = d.kind;
    _lastTapDownGlobalPosition = d.globalPosition;
    _isMultiFingerTouch = false;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      _lastPosOfDoubleTapDown = d.localPosition;
      // Desktop or mobile "Touch mode"
      _lastTapDownDetails = d;
    } else {
      _lastTapDownPositionForMouseMode = d.localPosition;
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    }
  }

  onTapUp(TapUpDetails d) async {
    final TapDownDetails? lastTapDownDetails = _lastTapDownDetails;
    _lastTapDownDetails = null;
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    // Filter duplicate touch tap events on iOS (Magic Mouse issue).
    if (inputModel.shouldIgnoreTouchTap(d.globalPosition)) {
      return;
    }
    if (handleTouch) {
      final isMoved =
          await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
      if (isMoved) {
        // If pan already handled 'down', don't send it again.
        if (lastTapDownDetails != null && !_touchModePanStarted) {
          await inputModel.tapDown(MouseButtons.left);
        }
        await inputModel.tapUp(MouseButtons.left);
      }
    }
  }

  onTap() async {
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    // Filter duplicate touch tap events on iOS (Magic Mouse issue).
    final lastPos = _lastTapDownGlobalPosition;
    if (lastPos != null && inputModel.shouldIgnoreTouchTap(lastPos)) {
      return;
    }
    if (!handleTouch) {
      // Cannot use `_lastTapDownDetails` because Flutter calls `onTapUp` before `onTap`, clearing the cached details.
      // Using `_lastTapDownPositionForMouseMode` instead.
      if (shouldBlockMouseModeEvent()) {
        return;
      }
      if (_lastTapDownPositionForMouseMode != null) {
        await ffi.cursorModel.move(_lastTapDownPositionForMouseMode!.dx,
            _lastTapDownPositionForMouseMode!.dy);
      }
      // Mobile, "Mouse mode"
      await inputModel.tap(MouseButtons.left);
    }
  }

  onDoubleTapDown(TapDownDetails d) async {
    lastDeviceKind = d.kind;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      _lastPosOfDoubleTapDown = d.localPosition;
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    } else {
      _lastTapDownPositionForMouseMode = d.localPosition;
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    }
  }

  onDoubleTap() async {
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    if (ffiModel.touchMode && ffi.cursorModel.lastIsBlocked) {
      return;
    }
    if (handleTouch &&
        !ffi.cursorModel.isInRemoteRect(_lastPosOfDoubleTapDown)) {
      return;
    }
    // Check if the position is in a blocked area when using the mouse mode.
    if (!handleTouch) {
      if (shouldBlockMouseModeEvent()) {
        return;
      }
    }
    await inputModel.tap(MouseButtons.left);
    await inputModel.tap(MouseButtons.left);
  }

  onLongPressDown(LongPressDownDetails d) async {
    lastDeviceKind = d.kind;
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      _lastPosOfDoubleTapDown = d.localPosition;
      _cacheLongPressPosition = d.localPosition;
      if (!ffi.cursorModel.isInRemoteRect(d.localPosition)) {
        return;
      }
      _cacheLongPressPositionTs = DateTime.now().millisecondsSinceEpoch;
      if (ffiModel.isPeerMobile) {
        await ffi.cursorModel
            .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
        await inputModel.tapDown(MouseButtons.left);
      }
    } else {
      _lastTapDownPositionForMouseMode = d.localPosition;
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    }
  }

  onLongPressUp() async {
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      await inputModel.tapUp(MouseButtons.left);
    }
  }

  // for mobiles
  onLongPress() async {
    if (_isMultiFingerTouch || isNotTouchBasedDevice()) {
      return;
    }
    if (!ffi.ffiModel.isPeerMobile) {
      if (handleTouch) {
        final isMoved = await ffi.cursorModel
            .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
        if (!isMoved) {
          return;
        }
      } else {
        if (shouldBlockMouseModeEvent()) {
          return;
        }
        if (_lastTapDownPositionForMouseMode != null) {
          await ffi.cursorModel.move(_lastTapDownPositionForMouseMode!.dx,
              _lastTapDownPositionForMouseMode!.dy);
        }
      }
      await inputModel.tap(MouseButtons.right);
    } else {
      // It's better to send a message to tell the controlled device that the long press event is triggered.
      // We're now using a `TimerTask` in `InputService.kt` to decide whether to trigger the long press event.
      // It's not accurate and it's better to use the same detection logic in the controlling side.
    }
  }

  onLongPressMoveUpdate(LongPressMoveUpdateDetails d) async {
    if (!ffiModel.isPeerMobile || isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      if (!ffi.cursorModel.isInRemoteRect(d.localPosition)) {
        return;
      }
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    }
  }

  onDoubleFinerTapDown(TapDownDetails d) async {
    lastDeviceKind = d.kind;
    if (isNotTouchBasedDevice()) {
      return;
    }
    _doubleFinerTapPosition = d.localPosition;
    // ignore for desktop and mobile
  }

  onDoubleFinerTap(TapDownDetails d) async {
    lastDeviceKind = d.kind;
    if (isNotTouchBasedDevice()) {
      return;
    }

    // mobile mouse mode or desktop touch screen
    final isMobileMouseMode = isMobile && !ffiModel.touchMode;
    // We can't use `d.localPosition` here because it's always (0, 0) on desktop.
    final isDesktopInRemoteRect = (isDesktop || isWebDesktop) &&
        ffi.cursorModel.isInRemoteRect(_doubleFinerTapPosition);
    if (isMobileMouseMode || isDesktopInRemoteRect) {
      await inputModel.tap(MouseButtons.right);
    }
  }

  onHoldDragStart(DragStartDetails d) async {
    lastDeviceKind = d.kind;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (!handleTouch) {
      if (isSpecialHoldDragActive) return;
      await inputModel.sendMouse('down', MouseButtons.left);
    }
  }

  onHoldDragUpdate(DragUpdateDetails d) async {
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (!handleTouch) {
      if (isSpecialHoldDragActive) return;
      await ffi.cursorModel.updatePan(d.delta, d.localPosition, handleTouch);
    }
  }

  onHoldDragEnd(DragEndDetails d) async {
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (!handleTouch) {
      await inputModel.sendMouse('up', MouseButtons.left);
    }
  }

  onOneFingerPanStart(BuildContext context, DragStartDetails d) async {
    final TapDownDetails? lastTapDownDetails = _lastTapDownDetails;
    _lastTapDownDetails = null;
    lastDeviceKind = d.kind ?? lastDeviceKind;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (handleTouch) {
      if (lastTapDownDetails != null) {
        await ffi.cursorModel.move(lastTapDownDetails.localPosition.dx,
            lastTapDownDetails.localPosition.dy);
      }
      if (ffi.cursorModel.shouldBlock(d.localPosition.dx, d.localPosition.dy)) {
        return;
      }
      if (!ffi.cursorModel.isInRemoteRect(d.localPosition)) {
        return;
      }

      _touchModePanStarted = true;
      if (isDesktop || isWebDesktop) {
        ffi.cursorModel.trySetRemoteWindowCoords();
      }

      // Workaround for the issue that the first pan event is sent a long time after the start event.
      // If the time interval between the start event and the first pan event is less than 500ms,
      // we consider to use the long press position as the start position.
      //
      // TODO: We should find a better way to send the first pan event as soon as possible.
      if (DateTime.now().millisecondsSinceEpoch - _cacheLongPressPositionTs <
          500) {
        await ffi.cursorModel
            .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
      }
      // In relative mouse mode, skip mouse down - only send movement via sendMobileRelativeMouseMove
      if (!inputModel.relativeMouseMode.value) {
        await inputModel.sendMouse('down', MouseButtons.left);
      }
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    } else {
      final isMoved =
          await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
      if (!isMoved) {
        final offset = ffi.cursorModel.offset;
        final cursorX = offset.dx;
        final cursorY = offset.dy;
        final visible =
            ffi.cursorModel.getVisibleRect().inflate(1); // extend edges
        final size = MediaQueryData.fromView(View.of(context)).size;
        if (!visible.contains(Offset(cursorX, cursorY))) {
          await ffi.cursorModel.move(size.width / 2, size.height / 2);
        }
      }
    }
  }

  onOneFingerPanUpdate(DragUpdateDetails d) async {
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (ffi.cursorModel.shouldBlock(d.localPosition.dx, d.localPosition.dy)) {
      return;
    }
    if (handleTouch && !_touchModePanStarted) {
      return;
    }
    // In relative mouse mode, send delta directly without position tracking.
    if (inputModel.relativeMouseMode.value) {
      await inputModel.sendMobileRelativeMouseMove(d.delta.dx, d.delta.dy);
    } else if (!handleTouch) {
      await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    } else {
      await ffi.cursorModel.updatePan(d.delta, d.localPosition, handleTouch);
    }
  }

  onOneFingerPanEnd(DragEndDetails d) async {
    _touchModePanStarted = false;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (isDesktop || isWebDesktop) {
      ffi.cursorModel.clearRemoteWindowCoords();
    }
    if (handleTouch) {
      // In relative mouse mode, skip mouse up - matches the skipped mouse down in onOneFingerPanStart
      if (!inputModel.relativeMouseMode.value) {
        await inputModel.sendMouse('up', MouseButtons.left);
      }
    }
  }

  // Reset `_touchModePanStarted` if the one-finger pan gesture is cancelled
  // or rejected by the gesture arena. Without this, the flag can remain
  // stuck in the "started" state and cause issues such as the Magic Mouse
  // double-click problem on iPad with magic mouse.
  onOneFingerPanCancel() {
    if (_touchModePanStarted &&
        handleTouch &&
        !inputModel.relativeMouseMode.value) {
      inputModel.sendMouse('up', MouseButtons.left);
    }
    _touchModePanStarted = false;
  }

  @override
  void dispose() {
    _stopScrollMomentum();
    super.dispose();
  }

  void _stopScrollMomentum() {
    _scrollPacerTimer?.cancel();
    _scrollPacerTimer = null;
    _scrollVelocity = 0;
    _mouseScrollIntegral = 0;
    _isScrollDragging = false;
  }

  void _ensureScrollPacerRunning() {
    if (_scrollPacerTimer != null && _scrollPacerTimer!.isActive) {
      return;
    }
    _scrollPacerTimer =
        Timer.periodic(const Duration(milliseconds: 10), (timer) {
      if (!_isScrollDragging) {
        final elapsedSinceDrag =
            DateTime.now().millisecondsSinceEpoch - _lastScrollUpdateMs;
        if (_scrollVelocity.abs() > 0.35 && elapsedSinceDrag < 900) {
          _scrollVelocity *= 0.88;
          final coastGain = 0.16 * pow(_scrollVelocity.abs(), 1.45);
          _mouseScrollIntegral += _scrollVelocity.sign * coastGain;
        } else {
          _scrollVelocity = 0;
        }
      }

      if (_mouseScrollIntegral.abs() >= 1.0) {
        int step = (_mouseScrollIntegral * 0.35).truncate();
        if (step == 0) {
          step = _mouseScrollIntegral > 0 ? 1 : -1;
        }
        _mouseScrollIntegral -= step;
        inputModel.scroll(step);
      } else if (!_isScrollDragging && _scrollVelocity.abs() <= 0.35) {
        _stopScrollMomentum();
      }
    });
  }

  void onSingleFingerTouchStart(PointerDownEvent event) {
    _stopScrollMomentum();
    _isMultiFingerTouch = false;
    lastDeviceKind = event.kind;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (!handleTouch) {
      _lastTapDownPositionForMouseMode = event.localPosition;
      ffi.cursorModel.move(event.localPosition.dx, event.localPosition.dy);
    }
  }

  void onMultiFingerTouchStart(Offset localPosition) {
    _stopScrollMomentum();
    _isMultiFingerTouch = true;
    _lastTapDownDetails = null;
    lastDeviceKind = PointerDeviceKind.touch;
    ffi.cursorModel.move(localPosition.dx, localPosition.dy);
  }

  void onTwoFingerActivatedScrollStart(DragStartDetails d) async {
    _stopScrollMomentum();
    _isScrollDragging = true;
    _lastTapDownDetails = null;
    lastDeviceKind = d.kind ?? PointerDeviceKind.touch;
    await ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
  }

  void onTwoFingerActivatedScrollUpdate(DragUpdateDetails d) {
    if (ffi.ffiModel.isPeerAndroid) {
      return;
    }
    final dy = d.delta.dy;
    if (dy == 0) return;
    _isScrollDragging = true;
    _lastScrollUpdateMs = DateTime.now().millisecondsSinceEpoch;
    _scrollVelocity = _scrollVelocity * 0.65 + dy * 0.35;
    final expGain = (0.20 * pow(dy.abs(), 1.55)).clamp(0.12, 16.0);
    _mouseScrollIntegral += dy.sign * expGain;
    _ensureScrollPacerRunning();
  }

  void onTwoFingerActivatedScrollEnd(DragEndDetails d) {
    _isScrollDragging = false;
    final elapsedSinceDrag =
        DateTime.now().millisecondsSinceEpoch - _lastScrollUpdateMs;
    if (elapsedSinceDrag > 70 || _scrollVelocity.abs() < 1.2) {
      _scrollVelocity = 0;
    }
  }

  void onThreeFingerTap() {
    if (ffi.ffiModel.isPeerAndroid) {
      return;
    }
    inputModel.triggerShowAllApps();
  }

  void onThreeFingerAppSwitchStart(DragStartDetails d) {
    _lastTapDownDetails = null;
    _appSwitchDeltaX = 0;
  }

  void onThreeFingerAppSwitchUpdate(DragUpdateDetails d) async {
    if (ffi.ffiModel.isPeerAndroid) {
      return;
    }
    _appSwitchDeltaX += d.delta.dx;
    const double stepPx = 36.0;
    while (_appSwitchDeltaX.abs() >= stepPx) {
      if (_appSwitchDeltaX > 0) {
        _appSwitchDeltaX -= stepPx;
        await inputModel.stepAppSwitch(forward: true);
      } else {
        _appSwitchDeltaX += stepPx;
        await inputModel.stepAppSwitch(forward: false);
      }
    }
  }

  void onThreeFingerAppSwitchEnd(DragEndDetails d) {
    _appSwitchDeltaX = 0;
    inputModel.endAppSwitch();
  }

  // scale + pan event
  onTwoFingerScaleStart(ScaleStartDetails d) {
    _lastTapDownDetails = null;
    if (isNotTouchBasedDevice()) {
      return;
    }
    if (isSpecialHoldDragActive) {
      // Initialize the last focal point to calculate deltas manually.
      _lastSpecialHoldDragFocalPoint = d.focalPoint;
    }
  }

  onTwoFingerScaleUpdate(ScaleUpdateDetails d) async {
    if (isNotTouchBasedDevice()) {
      return;
    }

    // If in special drag mode, perform a pan instead of a scale.
    if (isSpecialHoldDragActive) {
      // Calculate delta manually to avoid the jumpy behavior.
      final delta = d.focalPoint - _lastSpecialHoldDragFocalPoint;
      _lastSpecialHoldDragFocalPoint = d.focalPoint;
      await ffi.cursorModel.updatePan(delta * 2.0, d.focalPoint, handleTouch);
      return;
    }

    if (canvasLocked) {
      onTwoFingerActivatedScrollUpdate(DragUpdateDetails(
        globalPosition: d.focalPoint,
        localPosition: d.localFocalPoint,
        delta: d.focalPointDelta,
      ));
      return;
    }

    if ((isDesktop || isWebDesktop)) {
      final scale = ((d.scale - _scale) * 1000).toInt();
      _scale = d.scale;

      if (scale != 0) {
        if (widget.isCamera) return;
        await bind.sessionSendPointer(
            sessionId: sessionId,
            msg: json.encode(
                PointerEventToRust(kPointerEventKindTouch, 'scale', scale)
                    .toJson()));
      }
    } else {
      // mobile
      ffi.canvasModel.updateScale(d.scale / _scale, d.focalPoint);
      _scale = d.scale;
      ffi.canvasModel.panX(d.focalPointDelta.dx);
      ffi.canvasModel.panY(d.focalPointDelta.dy);
    }
  }

  onTwoFingerScaleEnd(ScaleEndDetails d) async {
    if (isNotTouchBasedDevice()) {
      return;
    }
    if ((isDesktop || isWebDesktop)) {
      if (widget.isCamera) return;
      await bind.sessionSendPointer(
          sessionId: sessionId,
          msg: json.encode(
              PointerEventToRust(kPointerEventKindTouch, 'scale', 0).toJson()));
    } else {
      // mobile
      _scale = 1;
      // No idea why we need to set the view style to "" here.
      // bind.sessionSetViewStyle(sessionId: sessionId, value: "");
    }
    if (!isSpecialHoldDragActive) {
      await inputModel.sendMouse('up', MouseButtons.left);
    }
  }

  get onHoldDragCancel => null;
  get onThreeFingerVerticalDragUpdate => ffi.ffiModel.isPeerAndroid
      ? null
      : (DragUpdateDetails d) => onTwoFingerActivatedScrollUpdate(d);

  makeGestures(BuildContext context) {
    return <Type, GestureRecognizerFactory>{
      // Official
      TapGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<TapGestureRecognizer>(
              () => TapGestureRecognizer(), (instance) {
        instance
          ..onTapDown = onTapDown
          ..onTapUp = onTapUp
          ..onTap = onTap;
      }),
      DoubleTapGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<DoubleTapGestureRecognizer>(
              () => DoubleTapGestureRecognizer(), (instance) {
        instance
          ..onDoubleTapDown = onDoubleTapDown
          ..onDoubleTap = onDoubleTap;
      }),
      LongPressGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<LongPressGestureRecognizer>(
              () => LongPressGestureRecognizer(), (instance) {
        instance
          ..onLongPressDown = onLongPressDown
          ..onLongPressUp = onLongPressUp
          ..onLongPress = onLongPress
          ..onLongPressMoveUpdate = onLongPressMoveUpdate;
      }),
      // Customized
      HoldTapMoveGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<HoldTapMoveGestureRecognizer>(
              () => HoldTapMoveGestureRecognizer(),
              (instance) => instance
                ..onHoldDragStart = onHoldDragStart
                ..onHoldDragUpdate = onHoldDragUpdate
                ..onHoldDragCancel = onHoldDragCancel
                ..onHoldDragEnd = onHoldDragEnd),
      DoubleFinerTapGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<DoubleFinerTapGestureRecognizer>(
              () => DoubleFinerTapGestureRecognizer(), (instance) {
        instance
          ..onDoubleFinerTap = onDoubleFinerTap
          ..onDoubleFinerTapDown = onDoubleFinerTapDown;
      }),
      CustomTouchGestureRecognizer:
          GestureRecognizerFactoryWithHandlers<CustomTouchGestureRecognizer>(
              () => CustomTouchGestureRecognizer(), (instance) {
        instance.onOneFingerPanStart =
            (DragStartDetails d) => onOneFingerPanStart(context, d);
        instance
          ..onOneFingerPanUpdate = onOneFingerPanUpdate
          ..onOneFingerPanEnd = onOneFingerPanEnd
          ..onOneFingerPanCancel = onOneFingerPanCancel
          ..onSingleFingerTouchStart = onSingleFingerTouchStart
          ..onMultiFingerTouchStart = onMultiFingerTouchStart
          ..onTwoFingerScaleStart = onTwoFingerScaleStart
          ..onTwoFingerScaleUpdate = onTwoFingerScaleUpdate
          ..onTwoFingerScaleEnd = onTwoFingerScaleEnd
          ..onTwoFingerActivatedScrollStart = onTwoFingerActivatedScrollStart
          ..onTwoFingerActivatedScrollUpdate = onTwoFingerActivatedScrollUpdate
          ..onTwoFingerActivatedScrollEnd = onTwoFingerActivatedScrollEnd
          ..onThreeFingerTap = onThreeFingerTap
          ..onThreeFingerAppSwitchStart = onThreeFingerAppSwitchStart
          ..onThreeFingerAppSwitchUpdate = onThreeFingerAppSwitchUpdate
          ..onThreeFingerAppSwitchEnd = onThreeFingerAppSwitchEnd
          ..onThreeFingerVerticalDragUpdate = onThreeFingerVerticalDragUpdate;
      }),
    };
  }
}

class RawPointerMouseRegion extends StatelessWidget {
  final InputModel inputModel;
  final Widget child;
  final MouseCursor? cursor;
  final PointerEnterEventListener? onEnter;
  final PointerExitEventListener? onExit;
  final PointerDownEventListener? onPointerDown;
  final PointerUpEventListener? onPointerUp;

  RawPointerMouseRegion({
    this.onEnter,
    this.onExit,
    this.cursor,
    this.onPointerDown,
    this.onPointerUp,
    required this.inputModel,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerHover: inputModel.onPointHoverImage,
      onPointerDown: (evt) {
        onPointerDown?.call(evt);
        inputModel.onPointDownImage(evt);
      },
      onPointerUp: (evt) {
        onPointerUp?.call(evt);
        inputModel.onPointUpImage(evt);
      },
      onPointerMove: inputModel.onPointMoveImage,
      onPointerSignal: inputModel.onPointerSignalImage,
      onPointerPanZoomStart: inputModel.onPointerPanZoomStart,
      onPointerPanZoomUpdate: inputModel.onPointerPanZoomUpdate,
      onPointerPanZoomEnd: inputModel.onPointerPanZoomEnd,
      child: MouseRegion(
        cursor: inputModel.isViewOnly
            ? MouseCursor.defer
            : (cursor ?? MouseCursor.defer),
        onEnter: onEnter,
        onExit: onExit,
        child: child,
      ),
    );
  }
}

class CameraRawPointerMouseRegion extends StatelessWidget {
  final InputModel inputModel;
  final Widget child;
  final PointerEnterEventListener? onEnter;
  final PointerExitEventListener? onExit;
  final PointerDownEventListener? onPointerDown;
  final PointerUpEventListener? onPointerUp;

  CameraRawPointerMouseRegion({
    this.onEnter,
    this.onExit,
    this.onPointerDown,
    this.onPointerUp,
    required this.inputModel,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerHover: (evt) {
        final offset = evt.position;
        double x = offset.dx;
        double y = max(0.0, offset.dy);
        inputModel.handlePointerDevicePos(
            kPointerEventKindMouse, x, y, true, kMouseEventTypeDefault);
      },
      onPointerDown: (evt) {
        onPointerDown?.call(evt);
      },
      onPointerUp: (evt) {
        onPointerUp?.call(evt);
      },
      child: MouseRegion(
        cursor: MouseCursor.defer,
        onEnter: onEnter,
        onExit: onExit,
        child: child,
      ),
    );
  }
}

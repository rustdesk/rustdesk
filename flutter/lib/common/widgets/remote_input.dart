import 'dart:convert';

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
    return FocusScope(
        autofocus: true,
        child: Focus(
            autofocus: true,
            canRequestFocus: true,
            focusNode: focusNode,
            onFocusChange: onFocusChange,
            onKey: (FocusNode data, RawKeyEvent e) =>
                inputModel.handleRawKeyEvent(e),
            child: child));
  }
}

class RawTouchGestureDetectorRegion extends StatefulWidget {
  final Widget child;
  final FFI ffi;

  late final InputModel inputModel = ffi.inputModel;
  late final FfiModel ffiModel = ffi.ffiModel;

  RawTouchGestureDetectorRegion({
    required this.child,
    required this.ffi,
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
  double _scale = 1;

  PointerDeviceKind? lastDeviceKind;

  FFI get ffi => widget.ffi;
  FfiModel get ffiModel => widget.ffiModel;
  InputModel get inputModel => widget.inputModel;
  bool get handleTouch => (isDesktop || isWebDesktop) || ffiModel.touchMode;
  SessionID get sessionId => ffi.sessionId;

  @override
  Widget build(BuildContext context) {
    return RawGestureDetector(
      child: widget.child,
      gestures: makeGestures(context),
    );
  }

  onTapDown(TapDownDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      // Desktop or mobile "Touch mode"
      if (ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy)) {
        inputModel.tapDown(MouseButtons.left);
      }
    }
  }

  onTapUp(TapUpDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      if (ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy)) {
        inputModel.tapUp(MouseButtons.left);
      }
    }
  }

  onTap() {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (!handleTouch) {
      // Mobile, "Mouse mode"
      inputModel.tap(MouseButtons.left);
    }
  }

  onDoubleTapDown(TapDownDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    }
  }

  onDoubleTap() {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (ffiModel.touchMode && ffi.cursorModel.lastIsBlocked) {
      return;
    }
    inputModel.tap(MouseButtons.left);
    inputModel.tap(MouseButtons.left);
  }

  onLongPressDown(LongPressDownDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
      _cacheLongPressPosition = d.localPosition;
      _cacheLongPressPositionTs = DateTime.now().millisecondsSinceEpoch;
    }
  }

  onLongPressUp() {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      inputModel.tapUp(MouseButtons.left);
    }
  }

  // for mobiles
  onLongPress() {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      ffi.cursorModel
          .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
    }
    inputModel.tap(MouseButtons.right);
  }

  onDoubleFinerTapDown(TapDownDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    // ignore for desktop and mobile
  }

  onDoubleFinerTap(TapDownDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if ((isDesktop || isWebDesktop) || !ffiModel.touchMode) {
      inputModel.tap(MouseButtons.right);
    }
  }

  onHoldDragStart(DragStartDetails d) {
    lastDeviceKind = d.kind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (!handleTouch) {
      inputModel.sendMouse('down', MouseButtons.left);
    }
  }

  onHoldDragUpdate(DragUpdateDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (!handleTouch) {
      ffi.cursorModel.updatePan(d.delta, d.localPosition, handleTouch);
    }
  }

  onHoldDragEnd(DragEndDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (!handleTouch) {
      inputModel.sendMouse('up', MouseButtons.left);
    }
  }

  onOneFingerPanStart(BuildContext context, DragStartDetails d) {
    lastDeviceKind = d.kind ?? lastDeviceKind;
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (handleTouch) {
      if (ffi.cursorModel.shouldBlock(d.localPosition.dx, d.localPosition.dy)) {
        return;
      }
      if (isDesktop) {
        ffi.cursorModel.trySetRemoteWindowCoords();
      }
      // Workaround for the issue that the first pan event is sent a long time after the start event.
      // If the time interval between the start event and the first pan event is less than 500ms,
      // we consider to use the long press position as the start position.
      //
      // TODO: We should find a better way to send the first pan event as soon as possible.
      if (DateTime.now().millisecondsSinceEpoch - _cacheLongPressPositionTs <
          500) {
        ffi.cursorModel
            .move(_cacheLongPressPosition.dx, _cacheLongPressPosition.dy);
      }
      inputModel.sendMouse('down', MouseButtons.left);
      ffi.cursorModel.move(d.localPosition.dx, d.localPosition.dy);
    } else {
      final offset = ffi.cursorModel.offset;
      final cursorX = offset.dx;
      final cursorY = offset.dy;
      final visible =
          ffi.cursorModel.getVisibleRect().inflate(1); // extend edges
      final size = MediaQueryData.fromView(View.of(context)).size;
      if (!visible.contains(Offset(cursorX, cursorY))) {
        ffi.cursorModel.move(size.width / 2, size.height / 2);
      }
    }
  }

  onOneFingerPanUpdate(DragUpdateDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (ffi.cursorModel.shouldBlock(d.localPosition.dx, d.localPosition.dy)) {
      return;
    }
    ffi.cursorModel.updatePan(d.delta, d.localPosition, handleTouch);
  }

  onOneFingerPanEnd(DragEndDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if (isDesktop) {
      ffi.cursorModel.clearRemoteWindowCoords();
    }
    inputModel.sendMouse('up', MouseButtons.left);
  }

  // scale + pan event
  onTwoFingerScaleStart(ScaleStartDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
  }

  onTwoFingerScaleUpdate(ScaleUpdateDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if ((isDesktop || isWebDesktop)) {
      final scale = ((d.scale - _scale) * 1000).toInt();
      _scale = d.scale;

      if (scale != 0) {
        bind.sessionSendPointer(
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

  onTwoFingerScaleEnd(ScaleEndDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    if ((isDesktop || isWebDesktop)) {
      bind.sessionSendPointer(
          sessionId: sessionId,
          msg: json.encode(
              PointerEventToRust(kPointerEventKindTouch, 'scale', 0).toJson()));
    } else {
      // mobile
      _scale = 1;
      bind.sessionSetViewStyle(sessionId: sessionId, value: "");
    }
    inputModel.sendMouse('up', MouseButtons.left);
  }

  get onHoldDragCancel => null;
  get onThreeFingerVerticalDragUpdate => ffi.ffiModel.isPeerAndroid
      ? null
      : (d) {
          _mouseScrollIntegral += d.delta.dy / 4;
          if (_mouseScrollIntegral > 1) {
            inputModel.scroll(1);
            _mouseScrollIntegral = 0;
          } else if (_mouseScrollIntegral < -1) {
            inputModel.scroll(-1);
            _mouseScrollIntegral = 0;
          }
        };

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
          ..onLongPress = onLongPress;
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
          ..onTwoFingerScaleStart = onTwoFingerScaleStart
          ..onTwoFingerScaleUpdate = onTwoFingerScaleUpdate
          ..onTwoFingerScaleEnd = onTwoFingerScaleEnd
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

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';

import 'package:flutter_hbb/web/common.dart';
import 'package:flutter_hbb/web/models/model.dart';
import 'package:flutter_hbb/web/models/input_model.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';


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
  double _mouseScrollIntegral = 0; // mouse scroll speed controller
  double _scale = 1;

  PointerDeviceKind? lastDeviceKind;

  FFI get ffi => widget.ffi;
  FfiModel get ffiModel => widget.ffiModel;
  InputModel get inputModel => widget.inputModel;
  bool get handleTouch => isDesktop || ffiModel.touchMode;
  SessionID get sessionId => ffi.sessionId;

  @override
  Widget build(BuildContext context) {
    return RawGestureDetector(
      child: widget.child,
      gestures: makeGestures(context),
    );
  }

  onTapDown(TapDownDetails d) {
  }

  onTapUp(TapUpDetails d) {
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
  }

  onDoubleTap() {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
    }
    inputModel.tap(MouseButtons.left);
    inputModel.tap(MouseButtons.left);
  }

  onLongPressDown(LongPressDownDetails d) {
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
    if (isDesktop || !ffiModel.touchMode) {
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
  }

  onOneFingerPanUpdate(DragUpdateDetails d) {
  }

  onOneFingerPanEnd(DragEndDetails d) {
    if (lastDeviceKind != PointerDeviceKind.touch) {
      return;
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
  }

  onTwoFingerScaleEnd(ScaleEndDetails d) {
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
        cursor: cursor ?? MouseCursor.defer,
        onEnter: onEnter,
        onExit: onExit,
        child: child,
      ),
    );
  }
}

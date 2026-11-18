import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';

class _FfiModel extends Fake implements FfiModel {
  @override
  late VirtualMouseMode virtualMouseMode = VirtualMouseMode(this);

  @override
  bool get isPeerAndroid => false;

  @override
  Rect? displaysRect() => null;
}

class _Canvas extends Fake implements CanvasModel {
  @override
  double get scale => 1.0;

  @override
  double getAdjustY() => 0;
}

class _Input extends Fake implements InputModel {
  final mouseOffsets = <Offset>[];

  @override
  Map<String, dynamic>? handleMouse(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    mouseOffsets.add(offset);
    return null;
  }
}

class _Cursor extends Fake implements CursorModel {
  @override
  set blockEvents(bool v) {}

  @override
  addBlockedRect(Rect rect) {}

  @override
  removeBlockedRect(Rect rect) {}

  @override
  double get hotx => 0;

  @override
  double get hoty => 0;

  @override
  ui.Image? get image => null;
}

class _Ffi extends Fake implements FFI {
  @override
  final ffiModel = _FfiModel();

  @override
  final canvasModel = _Canvas();

  @override
  InputModel get inputModel => input;

  final input = _Input();

  @override
  final cursorModel = _Cursor();
}

void main() {
  // The cursor model is a ChangeNotifier in production, but a fake here, so the
  // provider check that asks for ChangeNotifierProvider would reject it.
  Provider.debugCheckInvalidValueType = null;

  // The handle draws its own arrow, and the canvas draws the peer cursor from
  // the offsets this widget reports. The two only line up while the reported
  // point is the handle position inside the canvas instead of the window.
  testWidgets('virtual mouse reports the handle in canvas coordinates',
      (tester) async {
    const safeArea = Offset(20, 40);
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    tester.view.padding = const FakeViewPadding(left: 20, top: 40);
    addTearDown(tester.view.reset);

    final ffi = _Ffi();
    ffi.ffiModel.virtualMouseMode.setShowVirtualMouse(true);
    await tester.pumpWidget(MaterialApp(
      home: Provider<CursorModel>.value(
        value: ffi.cursorModel,
        child: Padding(
          padding: EdgeInsets.only(left: safeArea.dx, top: safeArea.dy),
          child: Stack(children: [FloatingMouse(ffi: ffi)]),
        ),
      ),
    ));
    await tester.pump();

    final handle = find.byType(CursorPaint);
    expect(handle, findsOneWidget);
    final handleTopLeft = tester.getTopLeft(handle);

    final gesture = await tester.startGesture(
        tester.getCenter(find.byIcon(Icons.drag_indicator)));
    await tester.pump();
    await gesture.moveBy(const Offset(0, -20));
    await tester.pump();

    final reported = ffi.input.mouseOffsets.single;
    expect(reported.dx, moreOrLessEquals(handleTopLeft.dx - safeArea.dx));
    expect(reported.dy, moreOrLessEquals(handleTopLeft.dy - safeArea.dy));

    await gesture.up();
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
  });
}

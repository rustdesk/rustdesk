@TestOn('browser')

import 'dart:convert';
import 'dart:js' as js;

import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Input extends Fake implements InputModel {
  final scrollDirections = <int>[];
  final pressedButtons = <MouseButtons>[];

  @override
  Future<void> scroll(int y) async {
    scrollDirections.add(y);
  }

  @override
  Future<void> tapDown(MouseButtons button) async {
    pressedButtons.add(button);
  }

  @override
  Future<void> tapUp(MouseButtons button) async {}
}

class _Cursor extends Fake implements CursorModel {
  @override
  final blockedRects = <Rect>[];

  @override
  set blockEvents(bool value) {}

  @override
  void addBlockedRect(Rect rect) => blockedRects.add(rect);

  @override
  void removeBlockedRect(Rect rect) => blockedRects.remove(rect);
}

class _VirtualMouseMode extends ChangeNotifier implements VirtualMouseMode {
  @override
  bool get showVirtualMouse => true;

  @override
  bool get showVirtualJoystick => false;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Peer extends Fake implements FfiModel {
  @override
  final virtualMouseMode = _VirtualMouseMode();
}

class _FFI extends Fake implements FFI {
  @override
  final ffiModel = _Peer();

  @override
  final inputModel = _Input();

  @override
  final cursorModel = _Cursor();
}

void _expectInside(Rect child, Rect viewport) {
  expect(child.left, greaterThanOrEqualTo(viewport.left));
  expect(child.top, greaterThanOrEqualTo(viewport.top));
  expect(child.right, lessThanOrEqualTo(viewport.right));
  expect(child.bottom, lessThanOrEqualTo(viewport.bottom));
}

void main() {
  final savedPositions = <String, String>{};

  setUp(() {
    savedPositions.clear();
    js.context['getByName'] =
        js.allowInterop((String type, String key) => savedPositions[key] ?? '');
    js.context['setByName'] = js.allowInterop((String type, String value) {
      final option = jsonDecode(value) as Map<String, dynamic>;
      savedPositions[option['name'] as String] = option['value'] as String;
    });
  });

  tearDown(() {
    js.context.deleteProperty('getByName');
    js.context.deleteProperty('setByName');
  });

  Future<void> pumpMouseWidgets(WidgetTester tester, _FFI ffi,
      {double bottomBarHeight = 64, double rightInset = 60}) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        bottomNavigationBar: SizedBox(height: bottomBarHeight),
        body: Padding(
          padding: EdgeInsets.only(right: rightInset),
          child: SizedBox.expand(
            key: const Key('mouse viewport'),
            child: FloatingMouseWidgets(ffi: ffi),
          ),
        ),
      ),
    ));
    await tester.pump();
  }

  testWidgets('wheel and click buttons fit the available viewport',
      (tester) async {
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await pumpMouseWidgets(tester, _FFI());

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    _expectInside(tester.getRect(find.byType(FloatingWheel)), viewport);
    for (final button in find.byType(FloatingLeftRightButton).evaluate()) {
      _expectInside(tester.getRect(find.byWidget(button.widget)), viewport);
    }
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  testWidgets('saved click buttons remain separate and tappable after rotation',
      (tester) async {
    savedPositions['ll-mouse-btn-pos'] = '{"x":900.0,"y":500.0}';
    savedPositions['rl-mouse-btn-pos'] = '{"x":950.0,"y":520.0}';
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi);
    tester.view.physicalSize = const Size(800, 400);
    await tester.pump();

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    _expectInside(tester.getRect(find.byType(FloatingWheel)), viewport);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final rightButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    final leftRect = tester.getRect(leftButton);
    final rightRect = tester.getRect(rightButton);
    _expectInside(leftRect, viewport);
    _expectInside(rightRect, viewport);
    expect(leftRect.overlaps(rightRect), isFalse);
    await tester.tap(leftButton);
    await tester.tap(rightButton);
    expect((ffi.inputModel as _Input).pressedButtons,
        [MouseButtons.left, MouseButtons.right]);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
  });

  testWidgets('controls stay visible when the body shrinks without rotation',
      (tester) async {
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 400, rightInset: 0);

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    _expectInside(tester.getRect(find.byType(FloatingWheel)), viewport);
    for (final button in find.byType(FloatingLeftRightButton).evaluate()) {
      _expectInside(tester.getRect(find.byWidget(button.widget)), viewport);
    }
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  testWidgets('wheel starts at the viewport origin when space is too small',
      (tester) async {
    tester.view.physicalSize = const Size(200, 200);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 80, rightInset: 130);

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    final wheel = tester.getRect(find.byType(FloatingWheel));
    _expectInside(wheel, viewport);
    _expectInside(
        tester.getRect(find.byIcon(Icons.keyboard_arrow_down)), viewport);
    expect((ffi.cursorModel as _Cursor).blockedRects,
        contains(const Rect.fromLTWH(0, 0, 50, 120)));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  testWidgets('lower wheel control works in a 120 pixel viewport',
      (tester) async {
    tester.view.physicalSize = const Size(400, 200);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 80, rightInset: 0);

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    final lowerControl = find.byIcon(Icons.keyboard_arrow_down);
    _expectInside(tester.getRect(lowerControl), viewport);
    await tester.tap(lowerControl);
    expect((ffi.inputModel as _Input).scrollDirections, contains(-1));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  testWidgets('clamped saved buttons remain separate from the wheel',
      (tester) async {
    savedPositions['lp-mouse-btn-pos'] = '{"x":900.0,"y":180.0}';
    savedPositions['rp-mouse-btn-pos'] = '{"x":125.0,"y":345.0}';
    tester.view.physicalSize = const Size(460, 464);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi);

    final wheel = find.byType(FloatingWheel);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final rightButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    expect(tester.getRect(leftButton).overlaps(tester.getRect(rightButton)),
        isFalse);
    expect(tester.getRect(leftButton).overlaps(tester.getRect(wheel)), isFalse);
    expect(
        tester.getRect(rightButton).overlaps(tester.getRect(wheel)), isFalse);
    await tester.tap(leftButton);
    await tester.tap(rightButton);
    await tester.tap(wheel);
    expect((ffi.inputModel as _Input).pressedButtons,
        [MouseButtons.left, MouseButtons.right, MouseButtons.wheel]);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
  });

  testWidgets('saved button stays clear of wheel after viewport narrows',
      (tester) async {
    savedPositions['rl-mouse-btn-pos'] = '{"x":300.0,"y":180.0}';
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final rightButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    expect(tester.getRect(rightButton).left, 300);

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 400);
    final wheel = find.byType(FloatingWheel);
    expect(
        tester.getRect(rightButton).overlaps(tester.getRect(wheel)), isFalse);
    await tester.tap(wheel);
    expect((ffi.inputModel as _Input).pressedButtons, [MouseButtons.wheel]);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
  });

  testWidgets('viewport shrink preserves an active button drag',
      (tester) async {
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final gesture = await tester.startGesture(tester.getCenter(leftButton));
    try {
      await gesture.moveBy(const Offset(30, -40));
      await tester.pump();
      expect(tester.getRect(leftButton).left, 355);
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 100, rightInset: 0);
      expect(tester.getRect(leftButton).left, 355);
      expect(tester.getRect(leftButton).top, 245);
    } finally {
      await gesture.up();
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(milliseconds: 60));
    }
  });

  testWidgets('rotation during a drag preserves the landscape position',
      (tester) async {
    const original = '{"x":600.0,"y":120.0}';
    savedPositions['ll-mouse-btn-pos'] = original;
    tester.view.physicalSize = const Size(400, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final gesture = await tester.startGesture(tester.getCenter(leftButton));
    try {
      await gesture.moveBy(const Offset(30, -40));
      await tester.pump();
      tester.view.physicalSize = const Size(800, 400);
      await tester.pump();
      expect(tester.getRect(leftButton).topLeft, const Offset(600, 120));
    } finally {
      await gesture.up();
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(milliseconds: 60));
    }
    expect(savedPositions['ll-mouse-btn-pos'], original);
  });

  testWidgets('temporary viewport shrink preserves saved button positions',
      (tester) async {
    const original = '{"x":700.0,"y":150.0}';
    savedPositions['ll-mouse-btn-pos'] = original;
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 250);

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    _expectInside(tester.getRect(leftButton), viewport);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
    expect(savedPositions['ll-mouse-btn-pos'], original);
  });

  testWidgets('temporary viewport shrink does not save an untouched default',
      (tester) async {
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 180, rightInset: 0);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));

    expect(savedPositions.containsKey('ll-mouse-btn-pos'), isFalse);
    expect(savedPositions.containsKey('rl-mouse-btn-pos'), isFalse);
  });

  testWidgets('dragging a click button saves its new position', (tester) async {
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await pumpMouseWidgets(tester, _FFI(), bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    await tester.drag(leftButton, const Offset(30, -40));
    await tester.pump(const Duration(milliseconds: 1));

    final saved =
        jsonDecode(savedPositions['ll-mouse-btn-pos']!) as Map<String, dynamic>;
    expect(saved['x'], greaterThan(325));
    expect(saved['y'], lessThan(345));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });
}

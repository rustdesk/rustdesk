@TestOn('browser')

import 'dart:convert';
import 'dart:js' as js;

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart' as common;
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';

class _Input extends Fake implements InputModel {
  final scrollDirections = <int>[];
  final pressedButtons = <MouseButtons>[];
  final releasedButtons = <MouseButtons>[];

  @override
  final relativeMouseMode = false.obs;

  @override
  Future<void> sendMobileRelativeMouseMove(double dx, double dy) async {}

  @override
  Future<void> scroll(int y) async {
    scrollDirections.add(y);
  }

  @override
  Future<void> tapDown(MouseButtons button) async {
    pressedButtons.add(button);
  }

  @override
  Future<void> tapUp(MouseButtons button) async {
    releasedButtons.add(button);
  }
}

class _Cursor extends CursorModel {
  _Cursor(FFI ffi) : super(WeakReference(ffi));

  @override
  Future<void> syncCursorPosition() async {}
}

class _VirtualMouseMode extends ChangeNotifier implements VirtualMouseMode {
  bool _showVirtualMouse = true;

  @override
  bool get showVirtualMouse => _showVirtualMouse;

  void hide() {
    _showVirtualMouse = false;
    notifyListeners();
  }

  @override
  bool showVirtualJoystick = false;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Peer extends Fake implements FfiModel {
  @override
  bool get touchMode => false;

  @override
  final _VirtualMouseMode virtualMouseMode = _VirtualMouseMode();
}

class _FFI extends Fake implements FFI {
  @override
  final _Peer ffiModel = _Peer();

  @override
  final inputModel = _Input();

  @override
  late final cursorModel = _Cursor(this);
}

void _expectInside(Rect child, Rect viewport) {
  expect(child.left, greaterThanOrEqualTo(viewport.left));
  expect(child.top, greaterThanOrEqualTo(viewport.top));
  expect(child.right, lessThanOrEqualTo(viewport.right));
  expect(child.bottom, lessThanOrEqualTo(viewport.bottom));
}

void main() {
  final savedPositions = <String, String>{};
  final inputKeys = <String>[];

  setUp(() {
    savedPositions.clear();
    inputKeys.clear();
    KeyboardVisibilityTesting.setVisibilityForTesting(false);
    js.context['isMobile'] = js.allowInterop(() => true);
    js.context['getByName'] = js.allowInterop((String type, [String? key]) {
      if (type == 'translate') return jsonDecode(key!)['text'];
      return savedPositions[key] ?? '';
    });
    js.context['setByName'] = js.allowInterop((String type, dynamic value) {
      if (type == 'option:flutter:local') {
        final option = jsonDecode(value as String) as Map<String, dynamic>;
        savedPositions[option['name'] as String] = option['value'] as String;
      } else if (type == 'input_key') {
        inputKeys.add(jsonDecode(value as String)['name'] as String);
      }
    });
  });

  tearDown(() {
    js.context.deleteProperty('isMobile');
    js.context.deleteProperty('getByName');
    js.context.deleteProperty('setByName');
  });

  void setViewSize(WidgetTester tester, Size size) {
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
  }

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

  testWidgets('lower wheel control remains tappable in a narrow viewport',
      (tester) async {
    setViewSize(tester, const Size(240, 200));
    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);

    final lowerControl = find.byIcon(Icons.keyboard_arrow_down);
    await tester.tap(lowerControl, warnIfMissed: false);
    expect((ffi.inputModel as _Input).scrollDirections, [-1]);
    expect((ffi.inputModel as _Input).pressedButtons, isEmpty);
    final wheel = tester.getRect(find.byType(FloatingWheel));
    for (final button in find.byType(FloatingLeftRightButton).evaluate()) {
      expect(tester.getRect(find.byWidget(button.widget)).overlaps(wheel),
          isFalse);
    }
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
  });

  for (final size in [
    const Size(120, 400),
    const Size(240, 100),
    const Size(220, 100),
    const Size(215, 100),
    const Size(210, 100),
  ]) {
    testWidgets('all three controls stay separate and usable at $size',
        (tester) async {
      setViewSize(tester, size);
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      expect(find.byType(FloatingLeftRightButton), findsNWidgets(2));
      final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
      final wheel = tester.getRect(find.byType(FloatingWheel));
      final left = find.byWidgetPredicate(
          (widget) => widget is FloatingLeftRightButton && widget.isLeft);
      final right = find.byWidgetPredicate(
          (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
      final leftRect = tester.getRect(left);
      final rightRect = tester.getRect(right);
      for (final rect in [wheel, leftRect, rightRect]) {
        _expectInside(rect, viewport);
      }
      expect(leftRect.overlaps(rightRect), isFalse);
      expect(leftRect.overlaps(wheel), isFalse);
      expect(rightRect.overlaps(wheel), isFalse);
      await tester.tap(left);
      await tester.tap(right);
      await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
      await tester.pump(const Duration(milliseconds: 60));
      expect((ffi.inputModel as _Input).pressedButtons,
          [MouseButtons.left, MouseButtons.right]);
      expect((ffi.inputModel as _Input).releasedButtons,
          [MouseButtons.left, MouseButtons.right]);
      expect((ffi.inputModel as _Input).scrollDirections, [-1]);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets('an undersized viewport hides click targets until they fit',
      (tester) async {
    const savedLeft = '{"x":120.0,"y":300.0}';
    const savedRight = '{"x":220.0,"y":300.0}';
    savedPositions['ll-mouse-btn-pos'] = savedLeft;
    savedPositions['rl-mouse-btn-pos'] = savedRight;
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 730);

    expect(find.byType(FloatingLeftRightButton).hitTestable(), findsNothing);
    expect(ffi.cursorModel.blockedRects, [const Rect.fromLTWH(0, 0, 50, 120)]);
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    expect((ffi.inputModel as _Input).scrollDirections, [-1]);
    expect((ffi.inputModel as _Input).pressedButtons, isEmpty);

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final left = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final right = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    expect(tester.getRect(left).topLeft, const Offset(120, 300));
    expect(tester.getRect(right).topLeft, const Offset(220, 300));
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('keyboard shortcuts remain tappable above floating controls',
      (tester) async {
    final wasTest = common.isTest;
    common.isTest = true;
    addTearDown(() => common.isTest = wasTest);
    await common.initGlobalFFI();
    common.gFFI.ffiModel.pi.platform = 'Windows';
    KeyboardVisibilityTesting.setVisibilityForTesting(true);
    addTearDown(() => KeyboardVisibilityTesting.setVisibilityForTesting(false));
    setViewSize(tester, const Size(800, 120));
    final ffi = _FFI();
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Stack(children: [
          KeyHelpTools(keyboardIsVisible: true, showGestureHelp: false),
          FloatingMouseWidgets(ffi: ffi),
        ]),
      ),
    ));
    await tester.pump();

    await tester.tap(find.text('PgDn'), warnIfMissed: false);
    await tester.pump(const Duration(milliseconds: 1));
    expect(inputKeys, ['VK_NEXT']);
    expect((ffi.inputModel as _Input).pressedButtons, isEmpty);
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('closing the keyboard restores saved floating controls',
      (tester) async {
    const savedLeft = '{"x":120.0,"y":300.0}';
    const savedRight = '{"x":220.0,"y":300.0}';
    savedPositions['ll-mouse-btn-pos'] = savedLeft;
    savedPositions['rl-mouse-btn-pos'] = savedRight;
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);

    KeyboardVisibilityTesting.setVisibilityForTesting(true);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 0);
    expect(find.byType(FloatingWheel), findsNothing);
    expect(find.byType(FloatingLeftRightButton), findsNothing);
    expect((ffi.cursorModel as _Cursor).blockedRects, isEmpty);

    KeyboardVisibilityTesting.setVisibilityForTesting(false);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final rightButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    expect(tester.getRect(leftButton).topLeft, const Offset(120, 300));
    expect(tester.getRect(rightButton).topLeft, const Offset(220, 300));
    expect(find.byType(FloatingWheel), findsOneWidget);
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  for (final button in [
    MouseButtons.left,
    MouseButtons.right,
    MouseButtons.wheel
  ]) {
    testWidgets('opening the keyboard releases a held $button', (tester) async {
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      final control = button == MouseButtons.wheel
          ? find.byType(FloatingWheel)
          : find.byWidgetPredicate((widget) =>
              widget is FloatingLeftRightButton &&
              widget.isLeft == (button == MouseButtons.left));
      final gesture = await tester.startGesture(tester.getCenter(control));
      await tester.pump(const Duration(milliseconds: 250));
      expect((ffi.inputModel as _Input).pressedButtons, [button]);

      KeyboardVisibilityTesting.setVisibilityForTesting(true);
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 0);
      expect((ffi.inputModel as _Input).releasedButtons, [button]);
      await gesture.up();
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(milliseconds: 1));
    });
  }

  testWidgets('opening the keyboard releases a held button after movement',
      (tester) async {
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final gesture = await tester.startGesture(tester.getCenter(leftButton));
    await tester.pump(const Duration(milliseconds: 250));
    await gesture.moveBy(const Offset(1, 0));
    expect((ffi.inputModel as _Input).pressedButtons, [MouseButtons.left]);

    KeyboardVisibilityTesting.setVisibilityForTesting(true);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 0);
    expect((ffi.inputModel as _Input).releasedButtons, [MouseButtons.left]);
    await gesture.up();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 1));
  });

  for (final button in [MouseButtons.left, MouseButtons.right]) {
    testWidgets('hiding an early $button drag sends no unmatched release',
        (tester) async {
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      final control = find.byWidgetPredicate((widget) =>
          widget is FloatingLeftRightButton &&
          widget.isLeft == (button == MouseButtons.left));
      final gesture = await tester.startGesture(tester.getCenter(control));
      await tester.pump(const Duration(milliseconds: 100));
      await gesture.moveBy(const Offset(5, 0));
      expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);

      KeyboardVisibilityTesting.setVisibilityForTesting(true);
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 0);
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 250));
      expect((ffi.inputModel as _Input).pressedButtons, isEmpty);
      expect((ffi.inputModel as _Input).releasedButtons, isEmpty);
      expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('hiding after a completed $button hold does not release twice',
        (tester) async {
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      final control = find.byWidgetPredicate((widget) =>
          widget is FloatingLeftRightButton &&
          widget.isLeft == (button == MouseButtons.left));
      final gesture = await tester.startGesture(tester.getCenter(control));
      await tester.pump(const Duration(milliseconds: 250));
      await gesture.up();
      ffi.ffiModel.virtualMouseMode.hide();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 1));
      expect((ffi.inputModel as _Input).pressedButtons, [button]);
      expect((ffi.inputModel as _Input).releasedButtons, [button]);
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final hold in [false, true]) {
    testWidgets('disabling virtual mouse clears a drag (held: $hold)',
        (tester) async {
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      final left = find.byWidgetPredicate(
          (widget) => widget is FloatingLeftRightButton && widget.isLeft);
      final gesture = await tester.startGesture(tester.getCenter(left));
      await tester.pump(Duration(milliseconds: hold ? 250 : 100));
      await gesture.moveBy(const Offset(5, 0));
      expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
      expect(isSpecialHoldDragActive, hold);

      ffi.ffiModel.virtualMouseMode.hide();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 1));
      expect(find.byType(FloatingWheel), findsNothing);
      expect(find.byType(FloatingLeftRightButton), findsNothing);
      expect(ffi.cursorModel.blockedRects, isEmpty);
      expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
      expect(isSpecialHoldDragActive, isFalse);
      await gesture.up();
      expect((ffi.inputModel as _Input).pressedButtons,
          hold ? [MouseButtons.left] : isEmpty);
      expect((ffi.inputModel as _Input).releasedButtons,
          hold ? [MouseButtons.left] : isEmpty);
      await tester.pumpWidget(const SizedBox());
    });
  }

  for (final hold in [false, true]) {
    testWidgets('shrinking away a button releases input blocking (held: $hold)',
        (tester) async {
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      final left = find.byWidgetPredicate(
          (widget) => widget is FloatingLeftRightButton && widget.isLeft);
      final gesture = await tester.startGesture(tester.getCenter(left));
      await tester.pump(Duration(milliseconds: hold ? 250 : 100));
      await gesture.moveBy(const Offset(5, 0));
      expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
      expect(isSpecialHoldDragActive, hold);

      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 730);
      expect(find.byType(FloatingLeftRightButton), findsNothing);
      expect(ffi.cursorModel.shouldBlock(60, 1), isFalse);
      expect(isSpecialHoldDragActive, isFalse);
      expect((ffi.inputModel as _Input).pressedButtons,
          hold ? [MouseButtons.left] : isEmpty);
      expect((ffi.inputModel as _Input).releasedButtons,
          hold ? [MouseButtons.left] : isEmpty);
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 1));
      expect((ffi.inputModel as _Input).releasedButtons,
          hold ? [MouseButtons.left] : isEmpty);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets('joystick blocking survives click buttons disappearing',
      (tester) async {
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();
    ffi.ffiModel.virtualMouseMode.showVirtualJoystick = true;
    ffi.inputModel.relativeMouseMode.value = true;
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(find.byType(FloatingLeftRightButton), findsNWidgets(2));
    final joystick = find.byType(VirtualJoystick);
    final gesture = await tester.startGesture(tester.getCenter(joystick));
    await gesture.moveBy(const Offset(25, 0));
    await tester.pump(const Duration(milliseconds: 150));
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);

    try {
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 280, rightInset: 730);
      expect(find.byType(FloatingLeftRightButton), findsNothing);
      expect(joystick, findsOneWidget);
      expect(ffi.cursorModel.shouldBlock(60, 1), isTrue);
    } finally {
      await gesture.up();
    }
    await tester.pump();
    expect(ffi.cursorModel.shouldBlock(60, 1), isFalse);
    await tester.pumpWidget(const SizedBox());
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

  testWidgets(
      'a wheel collision resets both buttons until the viewport expands',
      (tester) async {
    const savedLeft = '{"x":320.0,"y":120.0}';
    const savedRight = '{"x":300.0,"y":300.0}';
    savedPositions['ll-mouse-btn-pos'] = savedLeft;
    savedPositions['rl-mouse-btn-pos'] = savedRight;
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final leftButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && widget.isLeft);
    final rightButton = find.byWidgetPredicate(
        (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
    expect(tester.getRect(leftButton).topLeft, const Offset(320, 120));
    expect(tester.getRect(rightButton).topLeft, const Offset(300, 300));

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 400);
    final wheelRect = tester.getRect(find.byType(FloatingWheel));
    final leftRect = tester.getRect(leftButton);
    final rightRect = tester.getRect(rightButton);
    expect(leftRect.topLeft, const Offset(125, 345));
    expect(rightRect.topLeft, const Offset(220, 345));
    expect(leftRect.overlaps(rightRect), isFalse);
    expect(leftRect.overlaps(wheelRect), isFalse);
    expect(rightRect.overlaps(wheelRect), isFalse);
    await tester.tap(leftButton);
    await tester.tap(rightButton);
    expect((ffi.inputModel as _Input).pressedButtons,
        [MouseButtons.left, MouseButtons.right]);
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(tester.getRect(leftButton).topLeft, const Offset(320, 120));
    expect(tester.getRect(rightButton).topLeft, const Offset(300, 300));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);
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

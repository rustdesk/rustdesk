import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart' as common;
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';

class _Binding extends Fake implements RustdeskImpl {
  final savedPositions = <String, String>{};
  final inputKeys = <String>[];

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) =>
      savedPositions[k] ?? '';

  @override
  Future<void> setLocalFlutterOption(
      {required String k, required String v, dynamic hint}) async {
    savedPositions[k] = v;
  }

  @override
  String mainGetLocalOption({required String key, dynamic hint}) => '';

  @override
  bool isDisableAb({dynamic hint}) => false;
  @override
  bool isDisableAccount({dynamic hint}) => false;
  @override
  bool isDisableGroupPanel({dynamic hint}) => false;

  @override
  Future<void> mainSetOption(
      {required String key, required String value, dynamic hint}) async {}

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<void> sessionInputKey(
      {required common.SessionID sessionId,
      required String name,
      required bool down,
      required bool press,
      required bool alt,
      required bool ctrl,
      required bool shift,
      required bool command,
      dynamic hint}) async {
    inputKeys.add(name);
  }
}

class _Input extends Fake implements InputModel {
  final movements = <Offset>[];
  final scrollDirections = <int>[];
  final pressedButtons = <MouseButtons>[];
  final releasedButtons = <MouseButtons>[];

  @override
  final relativeMouseMode = false.obs;

  @override
  Future<void> sendMobileRelativeMouseMove(double dx, double dy) async {
    movements.add(Offset(dx, dy));
  }

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

  void setJoystick(bool visible) {
    showVirtualJoystick = visible;
    notifyListeners();
  }

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
  final _Input inputModel = _Input();

  @override
  late final cursorModel = _Cursor(this);
}

void _expectInside(Rect child, Rect viewport) {
  expect(child.left, greaterThanOrEqualTo(viewport.left));
  expect(child.top, greaterThanOrEqualTo(viewport.top));
  expect(child.right, lessThanOrEqualTo(viewport.right));
  expect(child.bottom, lessThanOrEqualTo(viewport.bottom));
}

void _expectSeparate(WidgetTester tester, List<Finder> controls) {
  final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
  final rects = controls.map(tester.getRect).toList();
  for (var i = 0; i < rects.length; i++) {
    _expectInside(rects[i], viewport);
    for (var j = i + 1; j < rects.length; j++) {
      expect(rects[i].overlaps(rects[j]), isFalse);
    }
  }
}

final _left = find.byWidgetPredicate(
    (widget) => widget is FloatingLeftRightButton && widget.isLeft);
final _right = find.byWidgetPredicate(
    (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
final _wheel = find.byType(FloatingWheel);
final _joystick = find.byType(VirtualJoystick);

void main() {
  late _Binding binding;
  late Map<String, String> savedPositions;

  setUp(() {
    binding = _Binding();
    savedPositions = binding.savedPositions;
    platformFFI.setBindingForTesting(binding);
    KeyboardVisibilityTesting.setVisibilityForTesting(false);
    final wasMobile = common.isMobile;
    final wasTest = common.isTest;
    common.isMobile = true;
    common.isTest = true;
    addTearDown(() {
      common.isMobile = wasMobile;
      common.isTest = wasTest;
      KeyboardVisibilityTesting.setVisibilityForTesting(false);
    });
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

  Future<void> removeControls(WidgetTester tester) async {
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 60));
  }

  testWidgets('saved click buttons remain separate and tappable after rotation',
      (tester) async {
    savedPositions['ll-mouse-btn-pos'] = '{"x":900.0,"y":500.0}';
    savedPositions['rl-mouse-btn-pos'] = '{"x":950.0,"y":520.0}';
    setViewSize(tester, const Size(400, 800));
    final ffi = _FFI();

    await pumpMouseWidgets(tester, ffi);
    tester.view.physicalSize = const Size(800, 400);
    await tester.pump();

    _expectSeparate(tester, [_left, _right, _wheel]);
    await tester.tap(_left);
    await tester.tap(_right);
    expect(
        ffi.inputModel.pressedButtons, [MouseButtons.left, MouseButtons.right]);
    await removeControls(tester);
  });

  testWidgets('wheel starts at the viewport origin when space is too small',
      (tester) async {
    setViewSize(tester, const Size(200, 200));

    final ffi = _FFI();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 80, rightInset: 130);

    final viewport = tester.getRect(find.byKey(const Key('mouse viewport')));
    final wheel = tester.getRect(find.byType(FloatingWheel));
    _expectInside(wheel, viewport);
    _expectInside(
        tester.getRect(find.byIcon(Icons.keyboard_arrow_down)), viewport);
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    expect(ffi.inputModel.scrollDirections, [-1]);
    expect(ffi.cursorModel.blockedRects,
        contains(const Rect.fromLTWH(0, 0, 50, 120)));
    await removeControls(tester);
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
      _expectSeparate(tester, [_left, _right, _wheel]);
      await tester.tap(_left);
      await tester.tap(_right);
      await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
      await tester.pump(const Duration(milliseconds: 60));
      expect(ffi.inputModel.pressedButtons,
          [MouseButtons.left, MouseButtons.right]);
      expect(ffi.inputModel.releasedButtons,
          [MouseButtons.left, MouseButtons.right]);
      expect(ffi.inputModel.scrollDirections, [-1]);
      await tester.pumpWidget(const SizedBox());
      expect(savedPositions, isEmpty);
    });
  }

  for (final keyboard in [false, true]) {
    testWidgets(
        'temporary hiding restores saved controls (keyboard: $keyboard)',
        (tester) async {
      const savedLeft = '{"x":120.0,"y":300.0}';
      const savedRight = '{"x":220.0,"y":300.0}';
      savedPositions.addAll(
          {'ll-mouse-btn-pos': savedLeft, 'rl-mouse-btn-pos': savedRight});
      setViewSize(tester, const Size(800, 400));
      final ffi = _FFI();
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      KeyboardVisibilityTesting.setVisibilityForTesting(keyboard);
      await pumpMouseWidgets(tester, ffi,
          bottomBarHeight: 280, rightInset: keyboard ? 0 : 730);
      expect(find.byType(FloatingLeftRightButton), findsNothing);
      if (keyboard) {
        expect(_wheel, findsNothing);
        expect(ffi.cursorModel.blockedRects, isEmpty);
      } else {
        expect(
            ffi.cursorModel.blockedRects, [const Rect.fromLTWH(0, 0, 50, 120)]);
        await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
        expect(ffi.inputModel.scrollDirections, [-1]);
      }
      KeyboardVisibilityTesting.setVisibilityForTesting(false);
      await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
      expect(tester.getRect(_left).topLeft, const Offset(120, 300));
      expect(tester.getRect(_right).topLeft, const Offset(220, 300));
      await removeControls(tester);
      expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
      expect(savedPositions['rl-mouse-btn-pos'], savedRight);
    });
  }

  testWidgets('keyboard shortcuts remain tappable above floating controls',
      (tester) async {
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
    expect(binding.inputKeys, ['VK_NEXT']);
    expect(ffi.inputModel.pressedButtons, isEmpty);
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pumpWidget(const SizedBox());
  });

  for (final target in ['keyboard', 'disabled', 'shrink']) {
    for (final hold in [false, true]) {
      testWidgets('$target releases only dispatched input (held: $hold)',
          (tester) async {
        setViewSize(tester, const Size(800, 400));
        final ffi = _FFI();
        await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
        final gesture = await tester.startGesture(tester.getCenter(_left));
        await tester.pump(Duration(milliseconds: hold ? 250 : 100));
        await gesture.moveBy(const Offset(5, 0));
        expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
        expect(isSpecialHoldDragActive, hold);
        if (target == 'keyboard') {
          KeyboardVisibilityTesting.setVisibilityForTesting(true);
        } else if (target == 'disabled') {
          ffi.ffiModel.virtualMouseMode.hide();
        }
        await pumpMouseWidgets(tester, ffi,
            bottomBarHeight: 280, rightInset: target == 'shrink' ? 730 : 0);
        expect(find.byType(FloatingLeftRightButton), findsNothing);
        expect(ffi.cursorModel.shouldBlock(60, 1), isFalse);
        expect(isSpecialHoldDragActive, isFalse);
        expect(ffi.inputModel.pressedButtons,
            hold ? [MouseButtons.left] : isEmpty);
        expect(ffi.inputModel.releasedButtons,
            hold ? [MouseButtons.left] : isEmpty);
        await gesture.up();
        await removeControls(tester);
        expect(ffi.inputModel.releasedButtons,
            hold ? [MouseButtons.left] : isEmpty);
      });
    }
  }

  for (final button in [
    MouseButtons.left,
    MouseButtons.right,
    MouseButtons.wheel
  ]) {
    for (final released in [false, true]) {
      testWidgets(
          'hiding $button sends one release (already released: $released)',
          (tester) async {
        setViewSize(tester, const Size(800, 400));
        final ffi = _FFI();
        await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
        final control = button == MouseButtons.left
            ? _left
            : button == MouseButtons.right
                ? _right
                : _wheel;
        final gesture = await tester.startGesture(tester.getCenter(control));
        await tester.pump(const Duration(milliseconds: 250));
        if (released) await gesture.up();
        KeyboardVisibilityTesting.setVisibilityForTesting(true);
        await pumpMouseWidgets(tester, ffi,
            bottomBarHeight: 280, rightInset: 0);
        if (!released) await gesture.up();
        await removeControls(tester);
        expect(ffi.inputModel.pressedButtons, [button]);
        expect(ffi.inputModel.releasedButtons, [button]);
      });
    }
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
      await pumpMouseWidgets(tester, ffi,
          bottomBarHeight: 300, rightInset: 600);
      expect(find.byType(FloatingLeftRightButton), findsNothing);
      expect(joystick, findsOneWidget);
      expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    } finally {
      await gesture.up();
    }
    await tester.pump();
    expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('compact controls avoid the joystick and retain saved positions',
      (tester) async {
    setViewSize(tester, const Size(240, 200));
    final ffi = _FFI();
    ffi.inputModel.relativeMouseMode.value = true;
    savedPositions['ll-mouse-btn-pos'] = '{"x":25.0,"y":20.0}';
    savedPositions['rl-mouse-btn-pos'] = '{"x":95.0,"y":125.0}';
    final saved = Map<String, String>.from(savedPositions);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(tester.getRect(_left).topLeft, const Offset(25, 20));
    ffi.ffiModel.virtualMouseMode.setJoystick(true);
    await tester.pump();
    await tester.pump();
    _expectSeparate(tester, [_left, _right, _wheel, _joystick]);
    expect(tester.getRect(_right).topLeft, const Offset(95, 145));
    await tester.tap(_left);
    await tester.tap(_right);
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    final gesture = await tester.startGesture(tester.getCenter(_joystick));
    await gesture.moveBy(const Offset(25, 0));
    await tester.pump(const Duration(milliseconds: 150));
    await gesture.up();
    expect(
        ffi.inputModel.pressedButtons, [MouseButtons.left, MouseButtons.right]);
    expect(ffi.inputModel.scrollDirections, [-1]);
    expect(ffi.inputModel.movements, isNotEmpty);
    ffi.ffiModel.virtualMouseMode.setJoystick(false);
    await tester.pump();
    await tester.pump();
    expect(tester.getRect(_left).topLeft, const Offset(25, 20));
    expect(tester.getRect(_right).topLeft, const Offset(95, 125));
    await removeControls(tester);
    expect(savedPositions, saved);
  });

  testWidgets('shrinking the viewport hides an active joystick until it fits',
      (tester) async {
    setViewSize(tester, const Size(800, 400));
    final ffi = _FFI();
    ffi.ffiModel.virtualMouseMode.setJoystick(true);
    ffi.inputModel.relativeMouseMode.value = true;
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final gesture = await tester.startGesture(tester.getCenter(_joystick));
    await gesture.moveBy(const Offset(25, 0));
    await tester.pump(const Duration(milliseconds: 150));
    expect(ffi.inputModel.movements, isNotEmpty);
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 300, rightInset: 580);
    expect(_joystick, findsNothing);
    expect(find.byType(FloatingLeftRightButton), findsNWidgets(2));
    expect(_wheel, findsOneWidget);
    expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
    final sent = ffi.inputModel.movements.length;
    await tester.pump(const Duration(milliseconds: 200));
    expect(ffi.inputModel.movements.length, sent);
    await gesture.up();
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(_joystick, findsOneWidget);
    await removeControls(tester);
  });

  testWidgets('idle joystick removal and restoration preserve a button drag',
      (tester) async {
    setViewSize(tester, const Size(240, 200));
    final ffi = _FFI();
    ffi.ffiModel.virtualMouseMode.setJoystick(true);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    final gesture = await tester.startGesture(tester.getCenter(_right));
    await gesture.moveBy(const Offset(3, 0));
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 100, rightInset: 20);
    expect(_joystick, findsNothing);
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(_joystick, findsOneWidget);
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    await gesture.up();
    expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
    await removeControls(tester);
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
    expect(tester.getRect(_left).topLeft, const Offset(320, 120));
    expect(tester.getRect(_right).topLeft, const Offset(300, 300));

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 400);
    final wheelRect = tester.getRect(find.byType(FloatingWheel));
    final leftRect = tester.getRect(_left);
    final rightRect = tester.getRect(_right);
    expect(leftRect.topLeft, const Offset(125, 345));
    expect(rightRect.topLeft, const Offset(220, 345));
    expect(leftRect.overlaps(rightRect), isFalse);
    expect(leftRect.overlaps(wheelRect), isFalse);
    expect(rightRect.overlaps(wheelRect), isFalse);
    await tester.tap(_left);
    await tester.tap(_right);
    expect(
        ffi.inputModel.pressedButtons, [MouseButtons.left, MouseButtons.right]);
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);

    await pumpMouseWidgets(tester, ffi, bottomBarHeight: 0, rightInset: 0);
    expect(tester.getRect(_left).topLeft, const Offset(320, 120));
    expect(tester.getRect(_right).topLeft, const Offset(300, 300));
    await removeControls(tester);
    expect(savedPositions['ll-mouse-btn-pos'], savedLeft);
    expect(savedPositions['rl-mouse-btn-pos'], savedRight);
  });

  testWidgets('dragging a click button saves its new position', (tester) async {
    setViewSize(tester, const Size(800, 400));

    await pumpMouseWidgets(tester, _FFI(), bottomBarHeight: 0, rightInset: 0);
    await tester.drag(_left, const Offset(30, -40));
    await tester.pump(const Duration(milliseconds: 1));

    final saved =
        jsonDecode(savedPositions['ll-mouse-btn-pos']!) as Map<String, dynamic>;
    expect(saved['x'], greaterThan(325));
    expect(saved['y'], lessThan(345));
    await removeControls(tester);
  });
}

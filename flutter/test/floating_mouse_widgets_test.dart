import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse_widgets.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final positions = <String, String>{};

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) =>
      positions[k] ?? '';

  @override
  Future<void> setLocalFlutterOption(
      {required String k, required String v, dynamic hint}) async {
    positions[k] = v;
  }
}

class _Input extends Fake implements InputModel {
  final events = <(String, MouseButtons)>[];
  final scrolls = <int>[];

  @override
  Future<void> tapDown(MouseButtons button) async =>
      events.add(('down', button));

  @override
  Future<void> tapUp(MouseButtons button) async => events.add(('up', button));

  @override
  Future<void> scroll(int direction) async => scrolls.add(direction);
}

class _Cursor extends CursorModel {
  _Cursor(FFI ffi) : super(WeakReference(ffi));

  @override
  Future<void> syncCursorPosition() async {}
}

class _Peer extends Fake implements FfiModel {
  @override
  bool get touchMode => false;

  @override
  bool get isPeerAndroid => false;

  @override
  late final virtualMouseMode = VirtualMouseMode(this)
    ..setShowVirtualMouse(true);
}

class _FFI extends Fake implements FFI {
  @override
  final _Peer ffiModel = _Peer();
  @override
  final _Input inputModel = _Input();
  @override
  late final cursorModel = _Cursor(this);
}

final _left = find.byWidgetPredicate(
    (widget) => widget is FloatingLeftRightButton && widget.isLeft);
final _right = find.byWidgetPredicate(
    (widget) => widget is FloatingLeftRightButton && !widget.isLeft);
final _wheel = find.byType(FloatingWheel);

void _setScreen(WidgetTester tester, Size size) {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

Future<void> _pumpControls(WidgetTester tester, _FFI ffi,
    {double bottom = 0, double right = 0, VoidCallback? onCanvasTap}) async {
  await tester.pumpWidget(MaterialApp(
    home: Scaffold(
      bottomNavigationBar: SizedBox(height: bottom),
      body: Padding(
        padding: EdgeInsets.only(right: right),
        child: SizedBox.expand(
          key: const Key('viewport'),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: onCanvasTap,
            onDoubleTap: onCanvasTap == null ? null : () {},
            child: FloatingMouseWidgets(ffi: ffi),
          ),
        ),
      ),
    ),
  ));
  await tester.pump();
}

void _expectControlsInside(WidgetTester tester) {
  expect(find.byType(FloatingLeftRightButton), findsNWidgets(2));
  expect(_wheel, findsOneWidget);
  final viewport = tester.getRect(find.byKey(const Key('viewport')));
  final rects = [_left, _right, _wheel].map(tester.getRect).toList();
  for (var i = 0; i < rects.length; i++) {
    expect(viewport.intersect(rects[i]), rects[i]);
    for (var j = i + 1; j < rects.length; j++) {
      expect(rects[i].overlaps(rects[j]), isFalse);
    }
  }
}

void main() {
  late _Binding binding;
  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    KeyboardVisibilityTesting.setVisibilityForTesting(false);
    isSpecialHoldDragActive = false;
  });
  tearDown(() {
    KeyboardVisibilityTesting.setVisibilityForTesting(false);
    isSpecialHoldDragActive = false;
  });

  testWidgets('wheel and buttons remain usable in both orientations',
      (tester) async {
    final ffi = _FFI();
    for (final size in [const Size(400, 800), const Size(800, 400)]) {
      _setScreen(tester, size);
      await _pumpControls(tester, ffi, bottom: 64, right: 60);
      _expectControlsInside(tester);
      for (final button in [_left, _right]) {
        await tester.tap(button);
        await tester.pump(const Duration(milliseconds: 60));
      }
      await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
      expect(ffi.inputModel.events, [
        ('down', MouseButtons.left),
        ('up', MouseButtons.left),
        ('down', MouseButtons.right),
        ('up', MouseButtons.right),
      ]);
      expect(ffi.inputModel.scrolls, [-1]);
      ffi.inputModel.events.clear();
      ffi.inputModel.scrolls.clear();
    }
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('body resizing preserves saved positions without rotation',
      (tester) async {
    _setScreen(tester, const Size(800, 400));
    const saved = {
      'll-mouse-btn-pos': '{"x":120.0,"y":300.0}',
      'rl-mouse-btn-pos': '{"x":220.0,"y":300.0}',
    };
    binding.positions.addAll(saved);
    final ffi = _FFI();
    var canvasTaps = 0;
    void onCanvasTap() => canvasTaps++;
    await _pumpControls(tester, ffi, onCanvasTap: onCanvasTap);
    await tester.tap(_right);
    await tester.pump(const Duration(milliseconds: 50));
    await _pumpControls(tester, ffi,
        bottom: 200, right: 400, onCanvasTap: onCanvasTap);
    await tester.pump(const Duration(milliseconds: 400));
    expect(canvasTaps, 0);
    expect(ffi.inputModel.events, [
      ('down', MouseButtons.right),
      ('up', MouseButtons.right),
    ]);
    _expectControlsInside(tester);
    expect(tester.getRect(_left).topLeft, const Offset(120, 145));
    expect(tester.getRect(_right).topLeft, const Offset(220, 145));
    await _pumpControls(tester, ffi);
    expect(tester.getRect(_left).topLeft, const Offset(120, 300));
    expect(tester.getRect(_right).topLeft, const Offset(220, 300));
    await tester.pumpWidget(const SizedBox());
    expect(binding.positions, saved);
  });

  testWidgets('keyboard hiding releases an active button exactly once',
      (tester) async {
    _setScreen(tester, const Size(800, 400));
    final ffi = _FFI();
    var canvasTaps = 0;
    void onCanvasTap() => canvasTaps++;
    await _pumpControls(tester, ffi, onCanvasTap: onCanvasTap);
    final gesture = await tester.startGesture(tester.getCenter(_left));
    await tester.pump(const Duration(milliseconds: 201));
    await gesture.moveBy(const Offset(5, 0));
    expect(ffi.inputModel.events, [('down', MouseButtons.left)]);
    expect(ffi.cursorModel.shouldBlock(1, 1), isTrue);
    expect(isSpecialHoldDragActive, isTrue);
    KeyboardVisibilityTesting.setVisibilityForTesting(true);
    await _pumpControls(tester, ffi, bottom: 280, onCanvasTap: onCanvasTap);
    expect(find.byType(FloatingLeftRightButton), findsNothing);
    expect(_wheel, findsNothing);
    expect(ffi.cursorModel.blockedRects, isEmpty);
    expect(ffi.cursorModel.shouldBlock(1, 1), isFalse);
    expect(isSpecialHoldDragActive, isFalse);
    expect(ffi.inputModel.events, [
      ('down', MouseButtons.left),
      ('up', MouseButtons.left),
    ]);
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 400));
    expect(canvasTaps, 0);
    await tester.tapAt(const Offset(50, 50));
    await tester.pump(const Duration(milliseconds: 400));
    expect(canvasTaps, 1);
    expect(ffi.inputModel.events, [
      ('down', MouseButtons.left),
      ('up', MouseButtons.left),
    ]);
    await tester.pumpWidget(const SizedBox());
  });
}

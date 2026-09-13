import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart' as cursor_manager;
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/desktop_render_texture.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:uuid/uuid.dart';

const _viewport = Size(1920, 540);
const _id = 'local-display-review';
const _side = 64;
const _hotspot = Offset(8, 12);

class _Display extends Display {
  _Display(this.density, double left, int w, int h) {
    x = left;
    width = w;
    height = h;
  }
  final double density;
  @override
  double get scale => density;
}

class _Peer extends Fake implements FfiModel {
  _Peer() {
    pi.platform = kPeerPlatformLinux;
    pi.currentDisplay = kAllDisplayValue;
    pi.displays.assignAll([
      _Display(2, 0, 3840, 2160),
      _Display(1, 1920, 1920, 1080),
    ]);
  }
  @override
  final pi = PeerInfo();
  @override
  bool get isPeerLinux => true;
  @override
  bool get keyboard => true;
  @override
  bool get viewOnly => false;
  @override
  Rect get rect => const Rect.fromLTWH(0, 0, 3840, 1080);
}

class _Image extends ChangeNotifier implements ImageModel {
  @override
  bool get useTextureRender => true;
  @override
  ui.Image? get image => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Texture extends Fake implements TextureModel {
  final ids = <int, RxInt>{};
  @override
  RxInt getTextureId(int display) =>
      ids.putIfAbsent(display, () => display.obs);
}

class _Canvas extends CanvasModel {
  _Canvas(FFI ffi, String style)
      : viewStyle = ViewStyle(
            style: style,
            width: 1920,
            height: 540,
            displayWidth: 3840,
            displayHeight: 1080),
        super(WeakReference(ffi)) {
    id = _id;
  }
  @override
  double get scale => 0.5;
  @override
  Size get size => _viewport;
  @override
  bool get cursorEmbedded => false;
  @override
  final ViewStyle viewStyle;
}

class _FFI extends Fake implements FFI {
  _FFI(String style) {
    canvasModel = _Canvas(this, style);
    cursorModel = CursorModel(WeakReference(this));
    inputModel = InputModel(WeakReference(this));
  }
  @override
  final sessionId = Uuid().v4obj();
  @override
  String get id => _id;
  @override
  final ffiModel = _Peer();
  @override
  final imageModel = _Image();
  @override
  final textureModel = _Texture();
  @override
  late final CanvasModel canvasModel;
  @override
  late final CursorModel cursorModel;
  @override
  late final InputModel inputModel;
  Offset? mappedPosition;
}

Future<void> _shape(_FFI ffi, String id) async {
  ffi.cursorModel.id = id;
  await ffi.cursorModel.updateCursorData({
    'id': id,
    'width': '$_side',
    'height': '$_side',
    'scale': '2',
    'hotx': '${_hotspot.dx}',
    'hoty': '${_hotspot.dy}',
    'colors': jsonEncode(List.generate(_side * _side * 4, (_) => 255)),
  });
}

Widget _widget(_FFI ffi, double dpr) => Directionality(
    textDirection: TextDirection.ltr,
    child: MediaQuery(
        data: MediaQueryData(devicePixelRatio: dpr),
        child: MultiProvider(
            providers: [
              ChangeNotifierProvider<ImageModel>.value(value: ffi.imageModel),
              ChangeNotifierProvider<CanvasModel>.value(value: ffi.canvasModel),
              ChangeNotifierProvider<CursorModel>.value(value: ffi.cursorModel),
            ],
            child: ImagePaint(
                ffi: ffi,
                id: _id,
                zoomCursor: true.obs,
                cursorOverImage: true.obs,
                keyboardEnabled: true.obs,
                remoteCursorMoved: RemoteCursorMovedState.find(_id),
                listenerBuilder: (child) => Listener(
                    child: child,
                    onPointerHover: (e) {
                      final point = ffi.inputModel.handlePointerDevicePos(
                          kPointerEventKindMouse,
                          e.position.dx,
                          e.position.dy,
                          true,
                          kMouseEventTypeDefault,
                          moveCanvas: false);
                      if (point != null) {
                        ffi.mappedPosition =
                            Offset(point.x.toDouble(), point.y.toDouble());
                      }
                    })))));

Future<void> _settle(WidgetTester tester, _FFI ffi) async {
  await tester.pump();
  for (final key in ffi.cursorModel.cachedKeys) {
    await cursor_manager.CursorManager.instance.ensureCursorRegistered(key);
  }
  await tester.pump();
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final registrations = <Map<dynamic, dynamic>>[];
  final activations = <String>[];
  final channel = Platform.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  setUp(() {
    registrations.clear();
    activations.clear();
    RemoteCursorMovedState.init(_id);
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      if (call.method.startsWith('setCustomCursor')) {
        activations.add(call.arguments['name'] as String);
      }
      if (!call.method.startsWith('createCustomCursor')) return null;
      final args = call.arguments as Map<dynamic, dynamic>;
      registrations.add(args);
      return args['name'];
    });
  });
  tearDown(() {
    RemoteCursorMovedState.delete(_id);
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });
  for (final (style, dpr) in [
    (kRemoteViewStyleAdaptive, 1.0),
    (kRemoteViewStyleCustom, 2.0),
  ]) {
    for (final mode in ['movement', 'shape refresh', 'host position control']) {
      testWidgets(
          'mixed displays: $style DPR=$dpr $mode',
          (tester) => tester.runAsync(
              () => _checkMovement(tester, (style, dpr, mode), (registrations, activations))));
    }
  }
}

Future<void> _checkMovement(
    WidgetTester tester,
    (String, double, String) testCase,
    (List<Map<dynamic, dynamic>>, List<String>) calls) async {
  final (style, dpr, mode) = testCase;
  final (registrations, activations) = calls;
  tester.view.devicePixelRatio = dpr;
  tester.view.physicalSize = _viewport * dpr;
  addTearDown(tester.view.reset);
  final ffi = _FFI(style);
  addTearDown(() => _dispose(ffi));
  await _shape(ffi, '$style-$dpr-$mode-1');
  await ffi.cursorModel.updateCursorPosition({'x': '100', 'y': '100'}, _id);
  ffi.canvasModel.updateLocalCursor(50, 50);
  await tester.pumpWidget(_widget(ffi, dpr));
  await _settle(tester, ffi);
  expect(registrations.last['width'], 16 * dpr);
  await _moveToB(tester, ffi, (mode, activations));
  final args = registrations.last;
  expect((args['width'], args['height']), (32 * dpr, 32 * dpr));
  expect((args['hotX'], args['hotY']), (4 * dpr, 6 * dpr));
}

Future<void> _moveToB(WidgetTester tester, _FFI ffi, (String, List<String>) testCase) async {
  final (mode, activations) = testCase;
  final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
  await mouse.addPointer(location: const Offset(50, 50));
  await _settle(tester, ffi);
  final before = activations.length;
  for (final x in [60.0, 70.0, 80.0]) {
    await mouse.moveTo(Offset(x, 50));
    await _settle(tester, ffi);
    expect(activations.length, before);
  }
  await mouse.moveTo(
      Offset(1100 + CanvasModel.leftToEdge, 50 + CanvasModel.topToEdge));
  await _settle(tester, ffi);
  expect(activations.length, before + 1);
  expect(ffi.mappedPosition, const Offset(2200, 100));
  expect(ffi.cursorModel.offset, const Offset(100, 100));
  if (mode == 'shape refresh') {
    await _shape(ffi, '$mode-2');
  } else if (mode == 'host position control') {
    await ffi.cursorModel.updateCursorPosition({'x': '2200', 'y': '100'}, _id);
    ffi.canvasModel.updateLocalCursor(1100, 50);
  }
  await _settle(tester, ffi);
  final after = activations.length;
  expect(after, before + (mode == 'shape refresh' ? 2 : 1));
  for (final x in [1110.0, 1120.0, 1130.0]) {
    await mouse.moveTo(Offset(x, 50));
    await _settle(tester, ffi);
    expect(activations.length, after);
  }
  final displayB = tester.widgetList<Positioned>(find.byType(Positioned)).last;
  expect(displayB.width! / 1920, 0.5);
  await mouse.removePointer();
}

Future<void> _dispose(_FFI ffi) async {
  for (final key in ffi.cursorModel.cachedKeys) {
    await deleteCustomCursor(key);
  }
  ffi.cursorModel.disposeImages();
  ffi.cursorModel.dispose();
  ffi.canvasModel.dispose();
  ffi.imageModel.dispose();
  // Relative mode is never started and this fixture has no Rust session.
  ffi.inputModel.disposeSideButtonTracking();
}

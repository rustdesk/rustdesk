import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart' show CursorManager;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

const _viewport = Size(200, 160);

class _Image extends ChangeNotifier implements ImageModel {
  @override
  bool get useTextureRender => false;
  @override
  ui.Image? get image => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Canvas extends ChangeNotifier implements CanvasModel {
  _Canvas(this.devicePixelRatio, {required this.style, required this.scale});

  final String style;

  @override
  final double devicePixelRatio;
  @override
  final imageOverflow = false.obs;
  @override
  late final viewStyle = ViewStyle(
    style: style,
    width: _viewport.width,
    height: _viewport.height,
    displayWidth: 400,
    displayHeight: 320,
  );
  @override
  bool get cursorEmbedded => false;
  @override
  ScrollStyle get scrollStyle => ScrollStyle.scrollauto;
  @override
  Size get size => _viewport;
  @override
  final double scale;
  @override
  double get x => 0;
  @override
  double get y => 0;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Cursor extends CursorModel {
  _Cursor(this.cache, this._ffi) : super(WeakReference(_ffi));

  final FFI _ffi;
  @override
  WeakReference<FFI> get parent => WeakReference(_ffi);

  @override
  CursorData cache;
  @override
  double get hotx => cache.hotxOrigin;
  @override
  double get hoty => cache.hotyOrigin;
}

class _Input extends Fake implements InputModel {
  @override
  final relativeMouseMode = false.obs;
}

class _Peer extends Fake implements FfiModel {
  @override
  final pi = PeerInfo();
  @override
  bool isPeerLinux = false;
}

class _LinuxDisplay extends Display {
  @override
  double get scale => 2;
}

class _FFI extends Fake implements FFI {
  _FFI(this.canvasModel);

  @override
  final CanvasModel canvasModel;
  @override
  final inputModel = _Input();
  @override
  final _Peer ffiModel = _Peer();
}

Future<CursorData> _data(int density, String id) async {
  final bitmapDensity = density == 0 ? 1 : density;
  final image = await createTestImage(
      width: 9 * bitmapDensity, height: 18 * bitmapDensity);
  return CursorData(
    peerId: 'dpi-policy',
    id: id,
    image: img.Image(width: image.width, height: image.height, numChannels: 4),
    nativeImage: image,
    scale: 1,
    data: Uint8List.fromList([1, 2]),
    hotxOrigin: 4.0 * bitmapDensity,
    hotyOrigin: 9.0 * bitmapDensity,
    width: image.width,
    height: image.height,
    pixelRatio: density.toDouble(),
  );
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final view = binding.platformDispatcher.views.single;
  final channel = Platform.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  final windows = Platform.isWindows;
  final registrations = <Map<dynamic, dynamic>>[];
  setUp(() {
    registrations.clear();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      if (!call.method.startsWith('createCustomCursor')) return null;
      final args = call.arguments as Map<dynamic, dynamic>;
      registrations.add(args);
      return args['name'];
    });
  });
  tearDown(() {
    view.resetDevicePixelRatio();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });
  for (final forbidden in [false, true]) {
    test('predefined cursor forbidden=$forbidden preserves native RGBA', () {
      view.devicePixelRatio = 1;
      return _checkPredefinedCursor(
          forbidden ? preForbiddenCursor : preDefaultCursor, registrations);
    });
  }
  for (final testCase in [
    (kRemoteViewStyleAdaptive, false, 0, 0.25, 1.0),
    (kRemoteViewStyleCustom, false, 0, 0.25, 1.0),
    (kRemoteViewStyleAdaptive, false, 1, 0.25, windows ? 2.0 : 1.0),
    (kRemoteViewStyleAdaptive, false, 2, 0.25, windows ? 1.0 : 0.5),
    (kRemoteViewStyleCustom, false, 2, 0.25, windows ? 1.0 : 0.5),
    (kRemoteViewStyleAdaptive, true, 2, 0.25, windows ? 2 / 3 : 1 / 3),
    (kRemoteViewStyleCustom, true, 2, 0.25, windows ? 2 / 3 : 1 / 3),
    (kRemoteViewStyleOriginal, false, 2, 0.5, windows ? 1.0 : 2 / 3),
  ]) {
    testWidgets(
        '${testCase.$1} zoom=${testCase.$2} peerDPR=${testCase.$3}',
        (tester) => tester
            .runAsync(() => _checkPolicy(tester, testCase, registrations)));
  }
  test('live DPR changes invalidate a cached native cursor',
      () => _checkDprChange(view, registrations));
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    testWidgets('$style forbidden cursor ignores remote DPR', (tester) =>
        tester.runAsync(() => _checkPolicy(tester,
            (style, false, 2, 0.25, 0.5), registrations, dpr: 1)));
    testWidgets('Linux $style zoom follows video pixels', (tester) => tester.runAsync(
        () => _checkPolicy(tester, (style, true, 2, 1.0, windows ? 1.0 : 0.5),
            registrations, linux: true)));
  }
}

Future<void> _checkPolicy(
    WidgetTester tester,
    (String, bool, int, double, double) testCase,
    List<Map<dynamic, dynamic>> registrations,
    {bool linux = false, double dpr = 2}) async {
  final (style, zoom, density, viewScale, expectedScale) = testCase;
  tester.view.devicePixelRatio = dpr;
  final data = await _data(density, '$style-$zoom-$density');
  final originalBytes = data.data;
  // A stale cached DPR must not affect the cursor when the window moves.
  final canvas = _Canvas(1, style: style, scale: viewScale);
  final ffi = _FFI(canvas);
  ffi.ffiModel.isPeerLinux = linux;
  if (linux) ffi.ffiModel.pi.displays.add(_LinuxDisplay());
  final cursor = _Cursor(data, ffi);
  final keyboardEnabled = true.obs;
  await tester.pumpWidget(MediaQuery(
    data: MediaQueryData(devicePixelRatio: dpr),
    child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => _Image()),
          ChangeNotifierProvider<CanvasModel>.value(value: canvas),
          ChangeNotifierProvider<CursorModel>.value(value: cursor),
        ],
        child: ImagePaint(
          ffi: ffi,
          id: 'dpi-policy',
          zoomCursor: zoom.obs,
          cursorOverImage: true.obs,
          keyboardEnabled: keyboardEnabled,
          remoteCursorMoved: false.obs,
        )),
  ));
  final revoke = !zoom && style != kRemoteViewStyleOriginal;
  if (revoke) {
    await Future.wait(cursor.cachedKeys
        .map(CursorManager.instance.ensureCursorRegistered));
    keyboardEnabled.value = false;
    await tester.pump();
  }
  await tester.pumpWidget(const SizedBox.shrink());
  for (final key in cursor.cachedKeys) {
    await deleteCustomCursor(key);
  }
  data.nativeImage.dispose();
  cursor.dispose();
  canvas.dispose();
  expect(data.scale, closeTo(expectedScale, 1e-9));
  expect(data.hotx, closeTo(data.hotxOrigin * expectedScale, 1e-9));
  expect(data.hoty, closeTo(data.hotyOrigin * expectedScale, 1e-9));
  expect(data.data, same(originalBytes));
  if (revoke) {
    final args = registrations.last;
    expect(args['name'], contains('_${kPreForbiddenCursorId}_'));
    final rasterSize = 32 * (Platform.isWindows ? 1 : dpr);
    expect((args['width'], args['height']), (rasterSize, rasterSize));
    expect((args['hotX'], args['hotY']), (0.0, 0.0));
  }
  if (style == kRemoteViewStyleAdaptive && !zoom && density > 0) {
    final args = registrations.first;
    expect((args['width'], args['height']), ((Platform.isLinux ? 18 : 9) * dpr, 18 * dpr));
    expect((args['hotX'], args['hotY']), (4 * dpr, 9 * dpr));
    expect(args['imagePixelRatio'], dpr);
  }
}

Future<void> _checkDprChange(
    TestFlutterView view, List<Map<dynamic, dynamic>> registrations) async {
  // Keep the scale above the minimum so only DPR invalidates the cache.
  final data = await _data(2, 'dpr-cache');
  final canvas = _Canvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
  final cursor = _Cursor(data, _FFI(canvas));
  for (final dpr in [2.0, 1.0]) {
    view.devicePixelRatio = dpr;
    buildCursorOfCache(cursor, 1, data);
    await deleteCustomCursor(cursor.cachedKeys.last);
  }
  expect(registrations, hasLength(2));
  expect(registrations[0]['name'], isNot(registrations[1]['name']));
  data.nativeImage.dispose();
  cursor.dispose();
  canvas.dispose();
}

Future<void> _checkPredefinedCursor(PredefinedCursor predefined,
    List<Map<dynamic, dynamic>> registrations) async {
  await Future.doWhile(() async {
    await Future<void>.delayed(const Duration(milliseconds: 10));
    return predefined.cache == null;
  }).timeout(const Duration(seconds: 5));
  final cache = predefined.cache!;
  final original = img
      .decodePng(base64Decode(predefined.png))!
      .convert(format: img.Format.uint8, numChannels: 4);
  final canvas = _Canvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
  final cursor = _Cursor(cache, _FFI(canvas));
  buildCursorOfCache(cursor, 1, cache);
  await deleteCustomCursor(cursor.cachedKeys.single);
  cursor.dispose();
  canvas.dispose();

  final args = registrations.single;
  final bytes = args['buffer'] as Uint8List;
  final decoded = Platform.isWindows
      ? img.Image.fromBytes(
          width: args['width'] as int,
          height: args['height'] as int,
          bytes: bytes.buffer,
          bytesOffset: bytes.offsetInBytes,
          order: img.ChannelOrder.bgra)
      : img.decodePng(bytes)!;
  expect((decoded.width, decoded.height), (original.width, original.height));
  expect(args['imagePixelRatio'], 1.0);
  expect((args['hotX'], args['hotY']), (cache.hotxOrigin, cache.hotyOrigin));
  expect(cache.image.getBytes(), original.getBytes());
  // Actual bundled pixels cover transparent, opaque and translucent colors.
  for (final (x, y) in [(0, 0), (1, 8), (13, 22), (16, 16)]) {
    final expected = original.getPixel(x, y);
    final actual = decoded.getPixel(x, y);
    expect(actual.a, expected.a);
    for (final (got, want) in [
      (actual.r, expected.r),
      (actual.g, expected.g),
      (actual.b, expected.b)
    ]) {
      expect(got, closeTo(want, 1), reason: '${predefined.id} at ($x, $y)');
    }
  }
}

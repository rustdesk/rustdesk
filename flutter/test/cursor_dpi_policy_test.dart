import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_custom_cursor/cursor_manager.dart' show CursorManager;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

import 'cursor_test_utils.dart';

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

class _LinuxDisplay extends Display {
  @override
  double get scale => 2;
}

Future<CursorData> _data(int density, String id,
    {(int, int) source = (9, 18)}) async {
  final bitmapDensity = density == 0 ? 1 : density;
  final image = await createTestImage(
      width: source.$1 * bitmapDensity, height: source.$2 * bitmapDensity);
  return CursorData(
    peerId: 'dpi-policy',
    id: id,
    image: img.Image(width: image.width, height: image.height, numChannels: 4),
    nativeImage: image,
    scale: 1,
    data: Uint8List.fromList([1, 2]),
    hotxOrigin: (source.$1 ~/ 2) * bitmapDensity.toDouble(),
    hotyOrigin: (source.$2 ~/ 2) * bitmapDensity.toDouble(),
    width: image.width,
    height: image.height,
    pixelRatio: density.toDouble(),
  );
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final view = binding.platformDispatcher.views.single;
  final windows = Platform.isWindows;
  final registrations = <Map<dynamic, dynamic>>[];
  captureNativeCursors(registrations);
  tearDown(view.resetDevicePixelRatio);
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
    (kRemoteViewStyleOriginal, false, 1, 0.5, windows ? 1.0 : 2 / 3),
    (kRemoteViewStyleOriginal, false, 2, 0.5, windows ? 1.0 : 0.5),
  ]) {
    testWidgets(
        '${testCase.$1} zoom=${testCase.$2} peerDPR=${testCase.$3}',
        (tester) => tester
            .runAsync(() => _checkPolicy(tester, testCase, registrations)));
  }
  test('live DPR changes invalidate a cached native cursor',
      () => _checkDprChange(view, registrations));
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    final dpr = style == kRemoteViewStyleAdaptive ? 1.0 : 2.0;
    final source = style == kRemoteViewStyleAdaptive ? (64, 4) : (4, 64);
    testWidgets('$style unknown-density thin cursor preserves zoom', (tester) =>
        tester.runAsync(() => _checkPolicy(tester,
            (style, true, 0, 0.5, windows ? 0.5 * dpr : 0.5), registrations,
            dpr: dpr, source: source)));
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
    {bool linux = false, double dpr = 2, (int, int) source = (9, 18)}) async {
  final (style, zoom, density, viewScale, expectedScale) = testCase;
  tester.view.devicePixelRatio = dpr;
  final data = await _data(density, '$style-$zoom-$density', source: source);
  final originalBytes = data.data;
  // A stale cached DPR must not affect the cursor when the window moves.
  final canvas = CursorTestCanvas(1, style: style, scale: viewScale);
  final ffi = CursorTestFFI(canvas);
  ffi.ffiModel.isPeerLinux = linux;
  if (linux) ffi.ffiModel.pi.displays.add(_LinuxDisplay());
  final cursor = _Cursor(data, ffi);
  final keyboardEnabled = true.obs;
  await tester.pumpWidget(MediaQuery(
    data: MediaQueryData(devicePixelRatio: dpr),
    child: MultiProvider(
        providers: [
          ChangeNotifierProvider<ImageModel>(create: (_) => CursorTestImage()),
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
  if (zoom && density == 0) {
    final args = registrations.single;
    final rasterScale = expectedScale * (Platform.isWindows ? 1 : dpr);
    final width = source.$1 * rasterScale;
    final height = source.$2 * rasterScale;
    expect((args['width'], args['height']),
        (Platform.isLinux && height > width ? height : width, height));
    expect((args['hotX'], args['hotY']),
        ((source.$1 ~/ 2) * rasterScale, (source.$2 ~/ 2) * rasterScale));
  }
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
  final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
  final cursor = _Cursor(data, CursorTestFFI(canvas));
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
  final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
  final cursor = _Cursor(cache, CursorTestFFI(canvas));
  buildCursorOfCache(cursor, 1, cache);
  await deleteCustomCursor(cursor.cachedKeys.single);
  cursor.dispose();
  canvas.dispose();

  final args = registrations.single;
  final decoded = decodeNativeCursorRaster(args);
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

// The test drives the cursor lifecycle normally owned by MouseTracker.
// ignore_for_file: invalid_use_of_protected_member

@TestOn('browser')
library;

import 'dart:convert';
import 'dart:js' as js;
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart' as model;
import 'package:flutter_hbb/web/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';
import 'package:image/image.dart' as img;
import 'package:provider/provider.dart';

import 'cursor_test_utils.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  _alphaTests();
  _thinCursorTests();
  _sourceValidationTests();
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final dpr in [1.0, 2.0]) {
      for (final (platform, density) in [
        (kPeerPlatformMacOS, null), (kPeerPlatformMacOS, 0.0),
        (kPeerPlatformMacOS, 1.0), (kPeerPlatformMacOS, 2.0),
        (kPeerPlatformLinux, 2.0), (kPeerPlatformWindows, 2.0),
      ]) {
        testWidgets('Web $style zoom off DPR $dpr $platform density=$density',
            (tester) => tester.runAsync(() => _checkPolicy(tester, style, dpr,
                pixelRatio: density, platform: platform)));
      }
    }
  }
  test('Web cursor aligns CSS hotspots with rounded PNG dimensions', () async {
    final registered = _captureCursor();
    final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 0.5);
    addTearDown(canvas.dispose);
    final ffi = CursorTestFFI(canvas);
    for (final (hotspot, scale, side, expected) in [
      ((7.0, 7.0), 633 / 1600, 19, (3, 3)),
      ((21.0, 23.0), 633 / 1600, 19, (8, 9)),
      ((22.0, 22.0), 633 / 1600, 19, (9, 9)),
      ((21.0, 23.0), 0.05, 12, (5, 6)),
      ((21.0, 23.0), 0.5, 24, (11, 12)),
      ((7.0, 7.0), 1.0, 48, (7, 7)),
    ]) {
      final cursor = await _loadCursor(ffi, '$hotspot-$scale',
          hotspot: hotspot, pixelRatio: null);
      final session =
          buildCursorOfCache(cursor, scale, cursor.cache).createSession(1);
      await session.activate();
      final uri = Uri.parse(registered['url'] as String);
      final bitmap = img.decodePng(uri.data!.contentAsBytes())!;
      expect((bitmap.width, bitmap.height), (side, side));
      expect((registered['hotx'], registered['hoty']), expected);
      session.dispose();
    }
  });
}

void _sourceValidationTests() {
  const side = 48;
  const sourceLimit = 4096;
  const bytes = side * side * 4;
  for (final density in [null, 4.0]) {
    test('Web rejects invalid cursor sources and recovers, density=$density', () async {
      final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
      addTearDown(canvas.dispose);
      final cursor = await _loadCursor(CursorTestFFI(canvas), 'source-$density',
          source: (side, side), pixelRatio: density);
      final cache = cursor.cache;
      final image = cursor.image;
      final event = {
        'id': 'source-$density', 'width': '$side', 'height': '$side',
        'hotx': '7', 'hoty': '9',
        if (density != null) 'scale': '$density',
        'colors': jsonEncode(List<int>.filled(bytes, 255)),
      };
      // Invalid JSON proves geometry is checked before decoding or allocation.
      const invalidPixels = 'must not decode an invalid source size';
      for (final invalid in [
        {'colors': '[1,2,3,4]'},
        {'colors': jsonEncode(List<int>.filled(bytes + 1, 255))},
        {'width': '0', 'colors': invalidPixels},
        {'height': '-1', 'colors': invalidPixels},
        {'width': '${sourceLimit * 4}', 'height': '${sourceLimit * 4}',
          'colors': invalidPixels},
        {'scale': '1e-300'},
        {'scale': '0.001'},
      ]) {
        await cursor.updateCursorData({...event, ...invalid});
        expect(cursor.cache, same(cache));
        expect(cursor.image, same(image));
        expect((cursor.hotx, cursor.hoty), (7, 9));
      }
      await cursor.updateCursorData(event);
      expect(cursor.cache, isNot(same(cache)));
    });
  }
}

void _thinCursorTests() {
  for (final style in [kRemoteViewStyleAdaptive, kRemoteViewStyleCustom]) {
    for (final zoom in [false, true]) {
      testWidgets('ImagePaint Web $style thin cursor zoom=$zoom', (tester) =>
          tester.runAsync(() => _checkPolicy(tester, style, 1,
              zoom: zoom, source: (64, 4), hotspot: (32, 2),
              expectedSize: zoom ? (32, 2) : (64, 4),
              expectedHotspot: zoom ? (16, 1) : (32, 2))));
    }
  }
  test('Web thin cursors keep a raster pixel and an in-bounds hotspot', () async {
    final registered = _captureCursor();
    final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 0.5);
    addTearDown(canvas.dispose);
    final ffi = CursorTestFFI(canvas);
    for (final (source, hotspot, scale, size, expected) in [
      ((4, 64), (2.0, 32.0), 0.5, (2, 32), (1, 16)),
      ((2, 128), (1.0, 64.0), 0.01, (1, 12), (0, 6)),
      ((128, 2), (64.0, 1.0), 0.01, (12, 1), (6, 0)),
    ]) {
      final cursor = await _loadCursor(ffi, '$source',
          source: source, hotspot: hotspot);
      for (final factor in [1.0, scale, 1.0]) {
        final session =
            buildCursorOfCache(cursor, factor, cursor.cache).createSession(1);
        await session.activate();
        session.dispose();
        final png = img.decodePng(Uri.parse(registered['url']).data!.contentAsBytes())!;
        expect((png.width, png.height), factor == 1 ? source : size);
        expect((registered['hotx'], registered['hoty']),
            factor == 1 ? (hotspot.$1, hotspot.$2) : expected);
      }
    }
  });
}

Future<model.CursorModel> _loadCursor(model.FFI ffi, String id,
    {List<int> pixel = const [255, 255, 255, 255],
    (int, int) source = (48, 48),
    (double, double) hotspot = (7, 9),
    double? pixelRatio = 1}) async {
  final cursor = model.CursorModel(WeakReference(ffi))..id = id;
  await cursor.updateCursorData({
    'id': id,
    'width': '${source.$1}',
    'height': '${source.$2}',
    'hotx': '${hotspot.$1}',
    'hoty': '${hotspot.$2}',
    if (pixelRatio != null) 'scale': '$pixelRatio',
    'colors': jsonEncode([for (var i = 0; i < source.$1 * source.$2; i++) ...pixel]),
  });
  addTearDown(() async {
    // Keep the session owner alive across asynchronous image decoding.
    expect(cursor.parent.target, same(ffi));
    for (final key in cursor.cachedKeys) {
      await deleteCustomCursor(key);
    }
    cursor.disposeImages();
    cursor.dispose();
  });
  return cursor;
}

Map<String, dynamic> _captureCursor() {
  final originals = {
    for (final key in ['isMobile', 'getByName', 'setByName'])
      key: js.context[key]
  };
  js.context['isMobile'] = js.allowInterop(() => false);
  js.context['getByName'] = js.allowInterop((String name, String value) => '');
  final registered = <String, dynamic>{};
  js.context['setByName'] = js.allowInterop((String name, String value) {
    if (name == 'cursor') {
      registered
        ..clear()
        ..addAll(jsonDecode(value));
    }
  });
  addTearDown(() {
    for (final entry in originals.entries) {
      js.context[entry.key] = entry.value;
    }
  });
  return registered;
}

Future<void> _checkPolicy(WidgetTester tester, String style, double dpr,
    {bool zoom = false, (int, int) source = (48, 48),
    (double, double) hotspot = (7, 9), (int, int) expectedSize = (48, 48),
    (int, int) expectedHotspot = (7, 9), double? pixelRatio = 1,
    String platform = kPeerPlatformMacOS}) async {
  final registered = _captureCursor();
  final canvas = CursorTestCanvas(1, style: style, scale: 0.5);
  addTearDown(canvas.dispose);
  final ffi = CursorTestFFI(canvas);
  ffi.ffiModel.pi.platform = platform;
  // Retina export changes the bitmap too; varying metadata alone misses this boundary.
  final density = platform == kPeerPlatformMacOS && pixelRatio == 2 ? 2 : 1;
  final cursor = await _loadCursor(ffi, '$style-$dpr-$zoom-$source-$platform-$pixelRatio',
      source: (source.$1 * density, source.$2 * density), pixelRatio: pixelRatio,
      hotspot: (hotspot.$1 * density, hotspot.$2 * density));
  await tester.pumpWidget(MediaQuery(
    data: MediaQueryData(devicePixelRatio: dpr),
    child: MultiProvider(
        providers: [
          ChangeNotifierProvider<model.ImageModel>(create: (_) => CursorTestImage()),
          ChangeNotifierProvider<model.CanvasModel>.value(value: canvas),
          ChangeNotifierProvider<model.CursorModel>.value(value: cursor),
        ],
        child: ImagePaint(
            ffi: ffi,
            id: 'web-cursor-test',
            zoomCursor: zoom.obs,
            cursorOverImage: true.obs,
            keyboardEnabled: true.obs,
            remoteCursorMoved: false.obs)),
  ));
  final session = tester
      .widget<MouseRegion>(find.byType(MouseRegion))
      .cursor
      .createSession(1);
  await session.activate();
  session.dispose();
  await tester.pumpWidget(const SizedBox.shrink());
  final png =
      img.decodePng(Uri.parse(registered['url']).data!.contentAsBytes())!;
  expect((png.width, png.height), expectedSize);
  expect((registered['hotx'], registered['hoty']), expectedHotspot);
}

void _alphaTests() {
  for (final (density, pixel) in [
    (1.0, [255, 255, 255, 128]),
    (2.0, [255, 128, 64, 128]),
    (1.0, [255, 128, 64, 112]),
    (1.0, [0, 0, 0, 0]),
    (1.0, [255, 255, 255, 255]),
    // Old macOS hosts send straight alpha without density metadata.
    (0.0, [255, 128, 64, 128]),
    (null, [80, 40, 20, 128]),
  ]) {
    test('Web macOS cursor density $density preserves RGBA $pixel',
        () => _checkAlpha(density, pixel));
  }
}

Future<void> _checkAlpha(double? density, List<int> pixel) async {
  final registered = _captureCursor();
  final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 0.5);
  addTearDown(canvas.dispose);
  final ffi = CursorTestFFI(canvas)..ffiModel.pi.platform = kPeerPlatformMacOS;
  final cursor = await _loadCursor(ffi, 'alpha-$density-$pixel',
      pixel: pixel, pixelRatio: density);
  // Cover both the painted remote cursor and the initial CSS cursor PNG.
  final straight = await cursor.image!
      .toByteData(format: ui.ImageByteFormat.rawStraightRgba);
  expect(straight!.buffer.asUint8List(0, 4), pixel);
  for (final scale in [1.0, 0.5, 1.0]) {
    final session =
        buildCursorOfCache(cursor, scale, cursor.cache).createSession(1);
    await session.activate();
    session.dispose();
    final png =
        img.decodePng(Uri.parse(registered['url']).data!.contentAsBytes())!;
    final color = png.getPixel(png.width ~/ 2, png.height ~/ 2);
    expect([color.r, color.g, color.b, color.a], pixel);
  }
}

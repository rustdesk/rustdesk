import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;

// Source size, source hotspot, requested/effective scale, artwork size, integer hotspot.
final _cases = [
  ((17, 23), (4.0, 4.0), 1 / 3, 12 / 17, (12, 16), (3.0, 3.0)),
  ((32, 16), (16.0, 8.0), 0.5, 0.75, (24, 12), (12.0, 6.0)),
  ((34, 46), (8.0, 8.0), 0.5, 0.5, (17, 23), (4.0, 4.0)),
  ((24, 24), (11.0, 11.0), 1 / 3, 0.5, (12, 12), (6.0, 6.0)),
  ((1, 48), (0.0, 24.0), 1.0, 1.0, (1, 48), (0.0, 24.0)),
  ((9, 18), (4.0, 9.0), 1.0, 1.0, (9, 18), (4.0, 9.0)),
  ((24, 24), (4.0, 4.0), 0.1, 0.5, (12, 12), (2.0, 2.0)),
  (
    (19, 27),
    (6.0, 13.0),
    1.37,
    1.37,
    Platform.isWindows ? (26, 36) : (26, 37),
    Platform.isWindows ? (8.0, 17.0) : (8.0, 18.0)
  ),
  (
    (18, 36),
    (8.0, 18.0),
    0.75,
    0.75,
    Platform.isWindows ? (13, 27) : (14, 27),
    (6.0, 14.0)
  ),
];
const _hotspotTolerance = 1e-9;

class _CursorModel implements CursorModel {
  @override
  final Set<String> cachedKeys = {};

  @override
  void addKey(String key) => cachedKeys.add(key);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _CursorFFI implements FFI {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

CursorData _cursorData((int, int) size, (double, double) hotspot) {
  final image = img.Image(width: size.$1, height: size.$2, numChannels: 4);
  for (var y = 0; y < image.height; y++) {
    image.setPixelRgba(image.width ~/ 2, y, 255, 255, 255, 255);
  }
  for (var x = 0; x < image.width; x++) {
    image.setPixelRgba(x, 0, 255, 0, 0, 255);
    image.setPixelRgba(x, image.height - 1, 0, 0, 255, 255);
  }
  return CursorData(
    peerId: 'cursor-test',
    id: 'native',
    image: image,
    scale: 1,
    data: Platform.isWindows
        ? image.getBytes(order: img.ChannelOrder.bgra)
        : Uint8List.fromList(img.encodePng(image)),
    hotxOrigin: hotspot.$1,
    hotyOrigin: hotspot.$2,
    width: size.$1,
    height: size.$2,
  );
}

Future<Map<dynamic, dynamic>> _register(
    WidgetTester tester, CursorData data, double scale) async {
  final channel = Platform.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  Map<dynamic, dynamic>? registered;
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
      (call) async {
    expect(
        call.method,
        Platform.isWindows
            ? 'createCustomCursor/windows'
            : 'createCustomCursor');
    registered = call.arguments as Map<dynamic, dynamic>;
    return registered!['name'];
  });
  addTearDown(() => tester.binding.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));
  buildCursorOfCache(_CursorModel(), scale, data);
  await tester.pump();
  expect(registered, isNotNull);
  return registered!;
}

void _expectArtwork(img.Image native, img.Image artwork) {
  final content = img.copyCrop(native,
      x: 0, y: 0, width: artwork.width, height: artwork.height);
  expect(content.getBytes(), artwork.getBytes());
  for (final pixel in native) {
    if (pixel.x >= artwork.width) {
      expect(pixel.a, 0, reason: 'Cursor padding must be transparent');
    }
  }
}

void main() {
  testWidgets('received cursor keeps edge colors across scale changes',
      (tester) async {
    const side = 4;
    for (final (rgba, expected) in [
      ([128, 64, 32, 128], [255, 128, 64, 128]),
      ([0, 0, 0, 0], [0, 0, 0, 0]),
      ([32, 64, 128, 255], [32, 64, 128, 255]),
    ]) {
      final cursor = CursorModel(WeakReference<FFI>(_CursorFFI()))
        ..id = 'edge-colors';
      addTearDown(cursor.disposeImages);
      addTearDown(cursor.dispose);
      await tester.runAsync(() => cursor.updateCursorData({
            'id': 'edge-colors',
            'hotx': '1',
            'hoty': '1',
            'width': '$side',
            'height': '$side',
            'colors': jsonEncode(List.generate(side * side, (_) => rgba)
                .expand((pixel) => pixel)
                .toList()),
          }));
      final data = cursor.cache!;
      for (final scale in [1.0, 0.5, 1.0]) {
        data.updateGetKey(scale);
        final image = Platform.isWindows
            ? img.Image.fromBytes(
                width: data.scaledWidth,
                height: data.scaledHeight,
                bytes: data.data!.buffer,
                order: img.ChannelOrder.bgra)
            : img.decodePng(data.data!)!;
        for (final pixel in image) {
          expect([pixel.r, pixel.g, pixel.b, pixel.a], expected,
              reason: 'Edge color must survive scale $scale');
        }
      }
    }
  });

  for (final (source, hotspot, scale, effectiveScale, size, integerHotspot)
      in _cases) {
    testWidgets('${source.$1}x${source.$2} cursor at scale $scale',
        (tester) async {
      final data = _cursorData(source, hotspot);
      final cursor = await _register(tester, data, scale);
      final artwork = Platform.isWindows
          ? img.Image.fromBytes(
              width: data.scaledWidth,
              height: data.scaledHeight,
              bytes: data.data!.buffer,
              order: img.ChannelOrder.bgra)
          : img.decodePng(data.data!)!;
      final native = Platform.isWindows
          ? img.Image.fromBytes(
              width: cursor['width'],
              height: cursor['height'],
              bytes: (cursor['buffer'] as Uint8List).buffer,
              bytesOffset: (cursor['buffer'] as Uint8List).offsetInBytes,
              order: img.ChannelOrder.bgra)
          : img.decodePng(cursor['buffer'] as Uint8List)!;
      final width = Platform.isLinux && size.$2 > size.$1 ? size.$2 : size.$1;

      expect(data.scale, effectiveScale);
      expect((artwork.width, artwork.height), size);
      expect(data.hotx / size.$1,
          closeTo(hotspot.$1 / source.$1, _hotspotTolerance));
      expect(data.hoty / size.$2,
          closeTo(hotspot.$2 / source.$2, _hotspotTolerance));
      expect((native.width, native.height), (width, size.$2));
      expect((cursor['width'], cursor['height']), (width, size.$2));
      expect(
          (cursor['hotX'], cursor['hotY']),
          Platform.isLinux || Platform.isWindows
              ? integerHotspot
              : (data.hotx, data.hoty));
      _expectArtwork(native, artwork);
      if (width == artwork.width) {
        expect(cursor['buffer'], data.data);
      }
    });
  }
}

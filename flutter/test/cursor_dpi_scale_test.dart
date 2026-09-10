import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;

// Source size, source hotspot, scale, artwork size, Linux hotspot.
const _cases = [
  ((9, 18), (4.0, 9.0), 1 / 3, (3, 6), (1.0, 3.0)),
  ((17, 23), (4.0, 4.0), 1 / 3, (6, 8), (1.0, 1.0)),
  ((32, 16), (16.0, 8.0), 0.5, (16, 8), (8.0, 4.0)),
  ((24, 24), (11.0, 11.0), 1 / 3, (8, 8), (4.0, 4.0)),
  ((3, 3), (2.0, 2.0), 1 / 3, (1, 1), (0.0, 0.0)),
  ((1, 48), (0.0, 24.0), 1 / 3, (1, 16), (0.0, 8.0)),
  ((9, 18), (4.0, 9.0), 1.0, (9, 18), (4.0, 9.0)),
  ((9, 18), (4.0, 9.0), 7 / 18, (4, 7), (2.0, 4.0)),
  ((24, 24), (4.0, 4.0), 0.1, (2, 2), (0.0, 0.0)),
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
    data: Uint8List.fromList(img.encodePng(image)),
    hotxOrigin: hotspot.$1,
    hotyOrigin: hotspot.$2,
    width: size.$1,
    height: size.$2,
  );
}

Future<Map<dynamic, dynamic>> _register(
    WidgetTester tester, CursorData data, double scale) async {
  const channel = MethodChannel('flutter_custom_cursor');
  Map<dynamic, dynamic>? registered;
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
      (call) async {
    expect(call.method, 'createCustomCursor');
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
  for (final (source, hotspot, scale, size, linuxHotspot) in _cases) {
    testWidgets('${source.$1}x${source.$2} cursor at scale $scale',
        (tester) async {
      final data = _cursorData(source, hotspot);
      final cursor = await _register(tester, data, scale);
      final artwork = img.decodePng(data.data!)!;
      final native = img.decodePng(cursor['buffer'] as Uint8List)!;
      final width = Platform.isLinux && size.$2 > size.$1 ? size.$2 : size.$1;

      expect(data.scale, scale);
      expect((artwork.width, artwork.height), size);
      expect(data.hotx / size.$1,
          closeTo(hotspot.$1 / source.$1, _hotspotTolerance));
      expect(data.hoty / size.$2,
          closeTo(hotspot.$2 / source.$2, _hotspotTolerance));
      expect((native.width, native.height), (width, size.$2));
      expect((cursor['width'], cursor['height']), (width, size.$2));
      expect((cursor['hotX'], cursor['hotY']),
          Platform.isLinux ? linuxHotspot : (data.hotx, data.hoty));
      _expectArtwork(native, artwork);
      if (width == artwork.width) {
        expect(cursor['buffer'], data.data);
      }
    }, skip: Platform.isWindows);
  }
}

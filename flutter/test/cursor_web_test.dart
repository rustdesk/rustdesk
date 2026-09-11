@TestOn('browser')
library;

import 'dart:convert';
import 'dart:js' as js;
import 'dart:typed_data';

import 'package:flutter_hbb/models/model.dart' as model;
import 'package:flutter_hbb/web/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;

class _CursorModel extends Fake implements model.CursorModel {
  @override
  final Set<String> cachedKeys = {};

  @override
  void addKey(String key) => cachedKeys.add(key);
}

void main() {
  test('Web cursor rounds its bitmap and hotspot together', () async {
    Map<String, dynamic>? registered;
    final original = js.context['setByName'];
    js.context['setByName'] = js.allowInterop((String name, String value) {
      expect(name, 'cursor');
      registered = jsonDecode(value) as Map<String, dynamic>;
    });
    addTearDown(() => js.context['setByName'] = original);

    const sourceSide = 48;
    final image = img.Image(width: sourceSide, height: sourceSide, numChannels: 4);
    img.fill(image, color: img.ColorRgba8(255, 255, 255, 255));
    for (final (sourceHotspot, scale, outputSide, expectedHotspot) in [
      ((7.0, 7.0), 633 / 1600, 19, (3, 3)),
      ((21.0, 23.0), 633 / 1600, 19, (8, 9)),
      ((22.0, 22.0), 633 / 1600, 19, (9, 9)),
      ((21.0, 23.0), 0.05, 12, (5, 6)),
      ((21.0, 23.0), 0.5, 24, (11, 12)),
      ((7.0, 7.0), 1.0, 48, (7, 7)),
    ]) {
      final cache = model.CursorData(
        peerId: 'web-cursor-test',
        id: '$sourceHotspot',
        image: image,
        scale: 1,
        data: Uint8List.fromList(img.encodePng(image)),
        hotxOrigin: sourceHotspot.$1,
        hotyOrigin: sourceHotspot.$2,
        width: sourceSide,
        height: sourceSide,
      );
      final cursor = buildCursorOfCache(_CursorModel(), scale, cache);
      final session = cursor.createSession(1);
      await session.activate();
      final uri = Uri.parse(registered!['url'] as String);
      final bitmap = img.decodePng(uri.data!.contentAsBytes())!;
      expect((bitmap.width, bitmap.height), (outputSide, outputSide));
      expect((registered!['hotx'], registered!['hoty']), expectedHotspot);
      session.dispose();
      await deleteCustomCursor(cache.updateGetKey(scale));
    }
  });
}

import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:image/image.dart' as img;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('sparse remote artwork cannot amplify the native bitmap without limit',
      () {
    const sourceSize = 64;
    const bitmapLimit = 512;
    final image = img.Image(width: sourceSize, height: sourceSize, numChannels: 4)
      ..setPixelRgba(0, 0, 255, 255, 255, 255);
    final cache = CursorData(
      peerId: 'sparse-cursor',
      id: '1',
      image: image,
      scale: 1,
      data: Uint8List.fromList(img.encodePng(image)),
      hotxOrigin: 32,
      hotyOrigin: 48,
      width: sourceSize,
      height: sourceSize,
    )..localSize = sourceSize.toDouble();
    final messages = <String>[];
    final previous = debugPrint;
    debugPrint = (String? message, {int? wrapWidth}) {
      messages.add(message ?? '');
    };
    addTearDown(() => debugPrint = previous);

    cache.updateGetKey(1);

    expect(max(cache.scaledWidth, cache.scaledHeight), bitmapLimit);
    expect(cache.hotx, 32 * cache.scaledWidth / sourceSize);
    expect(cache.hoty, 48 * cache.scaledHeight / sourceSize);
    expect(messages.single, contains('bitmap limit'));
  });
}

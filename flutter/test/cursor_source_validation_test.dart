import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';

import 'cursor_test_utils.dart';

const _side = 32;
const _rgbaChannels = 4;
const _sourceLimit = 4096;
const _invalidSizes = [
  (0, _side),
  (-1, _side),
  (_side, 0),
  (_side, -1),
  (_sourceLimit + 1, 1),
  (1, _sourceLimit + 1),
  (_sourceLimit * 4, _sourceLimit * 4),
];

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  for (final platform in [
    kPeerPlatformMacOS,
    kPeerPlatformWindows,
    kPeerPlatformLinux,
  ]) {
    for (final density in [null, '0', '4']) {
      test('cursor source validation $platform density=$density',
          () => _checkPackets(platform, density));
    }
  }
}

Future<void> _checkPackets(String platform, String? density) async {
  final canvas = CursorTestCanvas(1, style: kRemoteViewStyleAdaptive, scale: 1);
  final ffi = CursorTestFFI(canvas)..ffiModel.pi.platform = platform;
  final cursor = CursorModel(WeakReference(ffi))..id = 'valid';
  final messages = <String?>[];
  final originalPrint = debugPrint;
  debugPrint = (message, {wrapWidth}) => messages.add(message);
  addTearDown(() {
    debugPrint = originalPrint;
    expect(cursor.parent.target, same(ffi));
    cursor.disposeImages();
    cursor.dispose();
    canvas.dispose();
  });
  await cursor.updateCursorData(_event(density));
  final previous = cursor.cache;
  for (final length in [
    0,
    4,
    _side * _side * _rgbaChannels - 1,
    _side * _side * _rgbaChannels + 1
  ]) {
    await _expectRejected(cursor, _event(density, length: length));
    expect(messages, contains(contains('RGBA length')));
    messages.clear();
  }
  for (final (width, height) in _invalidSizes) {
    // Invalid JSON detects decoding before geometry validation, without risking
    // the oversized allocation if the production size check regresses.
    final packet = _event(density, width: width, height: height, length: 4)
      ..['colors'] = 'must not decode an invalid source size';
    await _expectRejected(cursor, packet);
    expect(messages, contains(contains('source size')));
    messages.clear();
  }
  for (final (width, height) in [
    (1, 1),
    (_sourceLimit, 1),
    (1, _sourceLimit)
  ]) {
    cursor.id = 'valid';
    await cursor
        .updateCursorData(_event(density, width: width, height: height));
    expect((cursor.image!.width, cursor.image!.height), (width, height));
    expect(cursor.cache, isNot(same(previous)));
  }
}

Future<void> _expectRejected(
    CursorModel cursor, Map<String, String> packet) async {
  final previous = cursor.cache;
  final image = cursor.image;
  final hotspot = (cursor.hotx, cursor.hoty);
  cursor.id = 'invalid';
  await cursor.updateCursorData({...packet, 'id': 'invalid'});
  expect(cursor.cache, same(previous));
  expect(cursor.image, same(image));
  expect((cursor.hotx, cursor.hoty), hotspot);
}

Map<String, String> _event(String? density,
        {int width = _side, int height = _side, int? length}) =>
    {
      'id': 'valid',
      'width': '$width',
      'height': '$height',
      'hotx': '0',
      'hoty': '0',
      if (density != null) 'scale': density,
      'colors': jsonEncode(
          List<int>.filled(length ?? width * height * _rgbaChannels, 255)),
    };

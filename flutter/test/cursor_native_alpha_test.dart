import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;

import 'cursor_test_utils.dart';

const _side = 32;
const _hotspot = Offset(7, 9);
const _colors = [
  [64, 32, 16, 128],
  [240, 100, 20, 255],
  [0, 0, 0, 0],
  [0, 0, 0, 128],
];
final _stripeWidth = _side ~/ _colors.length;

class _Canvas extends Fake implements CanvasModel {
  @override
  final viewStyle = ViewStyle(
      style: kRemoteViewStyleAdaptive,
      width: 200,
      height: 160,
      displayWidth: 200,
      displayHeight: 160);
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final view = binding.platformDispatcher.views.single;
  final registrations = <Map<dynamic, dynamic>>[];
  captureNativeCursors(registrations);
  tearDown(view.resetDevicePixelRatio);
  for (final testCase in <(String, String?, List<int>)>[
    (kPeerPlatformMacOS, null, [64, 32, 16, 128]),
    (kPeerPlatformMacOS, '0', [64, 32, 16, 128]),
    (kPeerPlatformMacOS, '1', [64, 32, 16, 128]),
    (kPeerPlatformMacOS, '2', [64, 32, 16, 128]),
    (kPeerPlatformWindows, null, [64, 32, 16, 128]),
    (kPeerPlatformWindows, '0', [64, 32, 16, 128]),
    (kPeerPlatformLinux, '0', [32, 16, 8, 128]),
  ]) {
    for (final dpr in [1.0, 2.0]) {
      test(
          'native ${testCase.$1} scale=${testCase.$2} DPR=$dpr preserves alpha',
          () {
        view.devicePixelRatio = dpr;
        return _checkCursor(testCase, dpr, registrations);
      });
    }
  }
}

Future<void> _checkCursor((String, String?, List<int>) testCase, double dpr,
    List<Map<dynamic, dynamic>> registrations) async {
  final (platform, density, color) = testCase;
  final ffi = CursorTestFFI(_Canvas())..ffiModel.pi.platform = platform;
  final cursor = CursorModel(WeakReference(ffi))..id = '$testCase-$dpr';
  addTearDown(() {
    cursor.disposeImages();
    cursor.dispose();
  });
  final palette = [color, ..._colors.skip(1)];
  final rgba = [
    for (var y = 0; y < _side; y++)
      for (var x = 0; x < _side; x++) ...palette[x ~/ _stripeWidth]
  ];
  await cursor.updateCursorData({
    'id': '$testCase-$dpr',
    'width': '$_side',
    'height': '$_side',
    'hotx': '${_hotspot.dx}',
    'hoty': '${_hotspot.dy}',
    if (density != null) 'scale': density,
    'colors': jsonEncode(rgba),
  });
  final cache = cursor.cache!;
  buildCursorOfCache(cursor, Platform.isWindows ? dpr : 1, cache);
  await deleteCustomCursor(cursor.cachedKeys.single);
  expect(cache.image.getBytes(), rgba); // Keep the original byte-cache source.
  _checkRegistration(registrations.single, dpr);
  // The same ui.Image also supplies the painted remote cursor.
  final straight = await cache.nativeImage
      .toByteData(format: ui.ImageByteFormat.rawStraightRgba);
  _checkColors(img.Image.fromBytes(
      width: _side,
      height: _side,
      bytes: straight!.buffer,
      bytesOffset: straight.offsetInBytes,
      order: img.ChannelOrder.rgba));
  // Real sessions own FFI strongly throughout asynchronous cursor decoding.
  expect(cursor.parent.target, same(ffi));
}

void _checkRegistration(Map<dynamic, dynamic> args, double dpr) {
  final decoded = decodeNativeCursorRaster(args);
  expect((decoded.width, decoded.height), (_side * dpr, _side * dpr));
  expect((args['hotX'], args['hotY']), (_hotspot.dx * dpr, _hotspot.dy * dpr));
  _checkColors(decoded);
}

void _checkColors(img.Image bitmap) {
  for (var stripe = 0; stripe < _colors.length; stripe++) {
    final pixel = bitmap.getPixel(
        ((stripe + 0.5) * bitmap.width / _colors.length).floor(),
        bitmap.height ~/ 2);
    final expected = _colors[stripe];
    expect(pixel.a, expected.last);
    for (final (actual, wanted) in [
      (pixel.r, expected[0]),
      (pixel.g, expected[1]),
      (pixel.b, expected[2])
    ]) {
      expect(actual, closeTo(wanted, 1), reason: 'RGBA stripe $stripe');
    }
  }
}

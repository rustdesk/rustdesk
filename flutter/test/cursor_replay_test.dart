import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _FFI extends Fake implements FFI {
  _FFI() {
    ffiModel = FfiModel(WeakReference(this));
    cursorModel = CursorModel(WeakReference(this));
  }
  @override
  final SessionID sessionId = const Uuid().v4obj();
  @override
  late final FfiModel ffiModel;
  @override
  late final CursorModel cursorModel;
}

Uint8List _pixels(int size, int seed) =>
    Uint8List.fromList(List.generate(size * size * 4, (i) => (i + seed) % 256));

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final channel = Platform.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  late _FFI ffi;
  setUp(() {
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      final args = call.arguments as Map<dynamic, dynamic>;
      return call.method.startsWith('createCustomCursor') ? args['name'] : null;
    });
    ffi = _FFI();
  });
  tearDown(() {
    ffi.cursorModel.disposeImages();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });

  test('a shape sent again after a reconnect is kept once', () async {
    for (var round = 0; round < 3; round++) {
      await ffi.ffiModel.handleCursorData('1', 0, 0, 8, 8, _pixels(8, 1));
      await ffi.ffiModel.handleCursorData('2', 1, 1, 8, 8, _pixels(8, 2));
    }
    expect(ffi.ffiModel.cachedPeerData.cursors.keys, ['1', '2']);
  });

  test('a moved tab carries the pixels as they are', () async {
    final pixels = _pixels(16, 7);
    await ffi.ffiModel.handleCursorData('9', 3, 4, 16, 16, pixels);
    final carried = ffi.ffiModel.cachedPeerData.toString();
    expect(carried.length, lessThan(pixels.length * 2),
        reason: 'the pixels travel as base64, not as a list of numbers');

    final cursor = CachedPeerData.fromString(carried)!.cursors['9']!;
    expect(cursor.colors, pixels);
    expect([cursor.hotx, cursor.hoty, cursor.width, cursor.height],
        [3, 4, 16, 16]);
  });

  test('a moved tab shows the shape that arrived last', () async {
    await ffi.ffiModel.handleCursorData('1', 0, 0, 8, 8, _pixels(8, 1));
    await ffi.ffiModel.handleCursorData('2', 0, 0, 8, 8, _pixels(8, 2));
    await ffi.ffiModel.handleCursorData('1', 0, 0, 8, 8, _pixels(8, 1));
    final carried =
        CachedPeerData.fromString(ffi.ffiModel.cachedPeerData.toString())!;
    expect(carried.lastCursorId['id'], '1',
        reason: 'the replay goes in first-seen order and ends on this');
  });

  test('a shape is decoded from its pixels as they arrive', () async {
    await ffi.ffiModel.handleCursorData('5', 0, 0, 8, 8, _pixels(8, 5));
    expect(ffi.cursorModel.cache?.id, '5');
    expect(ffi.cursorModel.image, isNotNull);
  });
}

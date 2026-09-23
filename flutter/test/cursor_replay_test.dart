import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_hbb/utils/image.dart' as img;
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

Future<void> _feed(_FFI ffi, String id, {int size = 8, int seed = 0}) =>
    ffi.ffiModel.handleCursorData(id, 0, 0, size, size, _pixels(size, seed));

void _select(_FFI ffi, String id) {
  final evt = {'id': id};
  ffi.ffiModel.updateLastCursorId(evt);
  ffi.ffiModel.handleCursorId(evt);
}

Future<void> _settle() async {
  for (var i = 0; i < 20; i++) {
    await Future<void>.delayed(const Duration(milliseconds: 5));
  }
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final channel = Platform.isWindows
      ? SystemChannels.mouseCursor
      : const MethodChannel('flutter_custom_cursor');
  final registered = <String>[];
  late _FFI ffi;
  setUp(() {
    registered.clear();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      final args = call.arguments as Map<dynamic, dynamic>;
      if (call.method.startsWith('createCustomCursor')) {
        registered.add(args['name'] as String);
        return args['name'];
      }
      return null;
    });
    ffi = _FFI();
  });
  tearDown(() {
    ffi.cursorModel.disposeImages();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });

  test('a shape sent again after a reconnect is kept once', () async {
    for (var round = 0; round < 3; round++) {
      await _feed(ffi, '1', seed: 1);
      await _feed(ffi, '2', seed: 2);
    }
    expect(ffi.cursorModel.shapeIds, ['1', '2']);
  });

  test('only the shape in use keeps its pixels besides its image', () async {
    await _feed(ffi, '1');
    await _feed(ffi, '2');
    await _feed(ffi, '3');
    for (final id in ['1', '2']) {
      expect(ffi.cursorModel.cachedShape(id)!.hasPixels, isFalse);
      expect(ffi.cursorModel.cachedShape(id)!.data, isNull);
    }
    expect(ffi.cursorModel.cachedShape('3')!.hasPixels, isTrue);
  });

  test('a shape shown before is shown again without its pixels', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    expect(cursor.cache!.hasPixels, isFalse,
        reason: 'once registered, even the shape in use keeps only its image');
    expect(cursor.cache!.data, isNull, reason: 'nor the bytes it was given');
    await _feed(ffi, '2');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    expect(registered.length, 2);

    _select(ffi, '1');
    expect(cursor.cache!.hasPixels, isFalse);
    expect(buildCursorOfCache(cursor, 1.0, cursor.cache),
        isNot(MouseCursor.defer));
    await _settle();
    expect(registered.length, 2, reason: 'its native cursor is still there');
  });

  test('a shape shown at a new scale gets its pixels back from its image',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    await _feed(ffi, '2', size: 32);
    _select(ffi, '1');
    expect(buildCursorOfCache(cursor, 0.5, cursor.cache), MouseCursor.defer);
    await _settle();

    expect(cursor.cache!.hasPixels, isTrue);
    buildCursorOfCache(cursor, 0.5, cursor.cache);
    await _settle();
    expect(registered.length, 1);
    expect((cursor.cache!.rasterWidth, cursor.cache!.rasterHeight), (16, 16));
  });

  test('a shape read back from its image is the pixels it came as', () async {
    // Includes channels above alpha, as a straight-alpha peer sends them.
    final pixels = _pixels(16, 3);
    final image = (await img.decodeImageFromPixels(
        pixels, 16, 16, ui.PixelFormat.rgba8888))!;
    final back = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
    image.dispose();
    expect(back!.buffer.asUint8List(), pixels);
  });

  test('a moved tab carries the pixels as they are', () async {
    final pixels = _pixels(16, 7);
    await ffi.ffiModel.handleCursorData('9', 3, 4, 16, 16, pixels);
    final carried = await ffi.ffiModel.cachedPeerDataString();
    expect(ffi.ffiModel.cachedPeerData.cursors, isEmpty,
        reason: 'the pixels read back for the move are not kept');
    expect(carried.length, lessThan(pixels.length * 2),
        reason: 'the pixels travel as base64, not as a list of numbers');

    final cursor = CachedPeerData.fromString(carried)!.cursors['9']!;
    expect(cursor.colors, pixels);
    expect([cursor.hotx, cursor.hoty, cursor.width, cursor.height],
        [3, 4, 16, 16]);
  });

  test('a moved tab shows the shape that arrived last', () async {
    await _feed(ffi, '1', seed: 1);
    await _feed(ffi, '2', seed: 2);
    await _feed(ffi, '1', seed: 1);
    final carried =
        CachedPeerData.fromString(await ffi.ffiModel.cachedPeerDataString())!;
    expect(carried.lastCursorId['id'], '1',
        reason: 'the replay goes in first-seen order and ends on this');
  });

  test('a shape is decoded from its pixels as they arrive', () async {
    await _feed(ffi, '5');
    expect(ffi.cursorModel.cache?.id, '5');
    expect(ffi.cursorModel.image, isNotNull);
  });
}

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/generated_bridge.dart' show CursorShape;
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/native/custom_cursor.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

/// Stands in for the core, which keeps every shape the peer sent.
class _Cursor extends CursorModel {
  _Cursor(FFI ffi) : super(WeakReference(ffi));
  final core = <String, CursorShape>{};
  final fetched = <String>[];
  @override
  Future<CursorShape?> fetchCursorShape(String id) async {
    fetched.add(id);
    return core[id];
  }
}

class _FFI extends Fake implements FFI {
  _FFI() {
    ffiModel = FfiModel(WeakReference(this));
    cursorModel = _Cursor(this);
  }
  @override
  final SessionID sessionId = const Uuid().v4obj();
  @override
  late final FfiModel ffiModel;
  @override
  late final CursorModel cursorModel;
  _Cursor get cursor => cursorModel as _Cursor;
}

Uint8List _pixels(int size, int seed) =>
    Uint8List.fromList(List.generate(size * size * 4, (i) => (i + seed) % 256));

/// A shape as the core delivers it, and keeps it.
Future<void> _feed(_FFI ffi, String id, {int size = 8, int seed = 0}) {
  final pixels = _pixels(size, seed);
  ffi.cursor.core[id] =
      CursorShape(hotx: 0, hoty: 0, width: size, height: size, colors: pixels);
  return ffi.ffiModel.handleCursorData(id, 0, 0, size, size, pixels);
}

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
  final deleted = <String>[];
  late _FFI ffi;
  setUp(() {
    registered.clear();
    deleted.clear();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel,
        (call) async {
      final args = call.arguments as Map<dynamic, dynamic>;
      if (call.method.startsWith('createCustomCursor')) {
        registered.add(args['name'] as String);
        return args['name'];
      }
      if (call.method.startsWith('deleteCustomCursor')) {
        deleted.add(args['name'] as String);
      }
      return null;
    });
    ffi = _FFI();
  });
  tearDown(() {
    ffi.cursorModel.disposeImages();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });

  test('only the shape in use keeps an image, and nothing else keeps pixels',
      () async {
    for (var round = 0; round < 3; round++) {
      await _feed(ffi, '1', seed: 1);
      await _feed(ffi, '2', seed: 2);
      await _feed(ffi, '3', seed: 3);
    }
    expect(ffi.cursorModel.shapeIds, ['3']);
    for (final id in ['1', '2']) {
      expect(ffi.cursorModel.cachedShape(id)!.hasPixels, isFalse);
      expect(ffi.cursorModel.cachedShape(id)!.data, isNull);
    }
    expect(ffi.cursor.fetched, isEmpty);
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
    expect(ffi.cursor.fetched, isEmpty, reason: 'nothing had to be decoded');
  });

  test('the shapes used last keep their native cursors, the others go',
      () async {
    const max = CursorModel.kMaxNativeCursors;
    final cursor = ffi.cursorModel;
    Future<void> show() async {
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }

    for (var i = 0; i < max; i++) {
      await _feed(ffi, '$i');
      await show();
    }
    _select(ffi, '0');
    await show();
    expect(deleted, isEmpty, reason: '$max fit');
    expect(ffi.cursor.fetched, isEmpty);

    await _feed(ffi, '$max');
    await show();
    expect(deleted, [registered[1]], reason: 'shape 1 was used longest ago');
    expect(cursor.cachedKeys.length, max);

    _select(ffi, '2');
    await show();
    expect(ffi.cursor.fetched, isEmpty, reason: 'shape 2 kept its cursor');
    _select(ffi, '1');
    expect(buildCursorOfCache(cursor, 1.0, cursor.cache), MouseCursor.defer);
    await _settle();
    expect(ffi.cursor.fetched, ['1'], reason: 'rebuilt from the core');
  });

  test('shapes each shown once leave no more native cursors than the limit',
      () async {
    const max = CursorModel.kMaxNativeCursors;
    final cursor = ffi.cursorModel;
    for (var i = 0; i < max + 4; i++) {
      await _feed(ffi, '$i');
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    expect(registered.length, max + 4);
    expect(registered.length - deleted.length, lessThanOrEqualTo(max + 1),
        reason: 'the one replaced last waits for the next build');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    expect(registered.length - deleted.length, max);
  });

  test('a shape painted again is decoded again from the core', () async {
    await _feed(ffi, '1', size: 16);
    await _feed(ffi, '2');
    _select(ffi, '1');
    expect(ffi.cursorModel.image, isNull);
    await _settle();
    expect(ffi.cursor.fetched, ['1']);
    expect(ffi.cursorModel.image?.width, 16);
    expect(ffi.cursorModel.shapeIds, ['1']);
  });

  test('a shape the core gives but that does not decode is asked for once',
      () async {
    await _feed(ffi, '1');
    await _feed(ffi, '2');
    ffi.cursor.core['1'] = CursorShape(
        hotx: 0, hoty: 0, width: 8, height: 8, colors: Uint8List(3));
    _select(ffi, '1');
    for (var i = 0; i < 5; i++) {
      ffi.cursorModel.image;
      await _settle();
    }
    expect(ffi.cursor.fetched, ['1']);
  });

  test('a shape switched away from while it decoded is asked for again',
      () async {
    await _feed(ffi, '1', size: 16);
    await _feed(ffi, '2');
    _select(ffi, '1');
    ffi.cursorModel.image; // asks the core
    // The core has answered and the shape is decoding when the peer moves on.
    await Future<void>.delayed(Duration.zero);
    _select(ffi, '2');
    await _settle();
    _select(ffi, '1');
    ffi.cursorModel.image;
    await _settle();
    expect(ffi.cursor.fetched, ['1', '1']);
    expect(ffi.cursorModel.image?.width, 16);
  });

  test('a shape decoded after the session was cleared is not kept', () async {
    await _feed(ffi, '1', size: 16);
    await _feed(ffi, '2');
    _select(ffi, '1');
    ffi.cursorModel.image; // asks the core
    ffi.cursorModel.clear();
    await _settle();
    expect(ffi.cursorModel.shapeIds, isEmpty);
    expect(ffi.cursorModel.cache, isNull);
  });

  test('a shape the core does not have is asked for once', () async {
    await _feed(ffi, '1');
    await _feed(ffi, '2');
    ffi.cursor.core.remove('1');
    _select(ffi, '1');
    for (var i = 0; i < 5; i++) {
      ffi.cursorModel.image;
      await _settle();
    }
    expect(ffi.cursor.fetched, ['1']);
  });

  test('a shape the core lacks is remembered only while it is in use',
      () async {
    await _feed(ffi, '1');
    for (var i = 0; i < 1000; i++) {
      _select(ffi, 'missing$i');
      ffi.cursorModel.image;
    }
    await _settle();
    expect(ffi.cursor.fetched.length, 1000, reason: 'each is asked for once');
    _select(ffi, 'missing0');
    await _settle();
    expect(ffi.cursor.fetched.length, 1001,
        reason: 'nothing is kept for a shape no longer in use');
  });

  test('a shape the core lacks is asked for again after another was in use',
      () async {
    await _feed(ffi, '1');
    _select(ffi, 'missing0');
    await _settle();
    _select(ffi, '1');
    _select(ffi, 'missing0');
    await _settle();
    expect(ffi.cursor.fetched, ['missing0', 'missing0']);
  });

  test('a shape shown at a new scale is decoded again from the core', () async {
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

  test('a moved tab carries the id in use, and its window asks the core',
      () async {
    await _feed(ffi, '1', size: 16);
    await _feed(ffi, '2');
    await _feed(ffi, '1', size: 16);
    final carried = ffi.ffiModel.cachedPeerData.toString();
    expect(carried, isNot(contains('colors')));
    expect(CachedPeerData.fromString(carried)!.lastCursorId['id'], '1',
        reason: 'a shape arriving is the shape in use');

    final moved = _FFI();
    addTearDown(moved.cursorModel.disposeImages);
    moved.cursor.core.addAll(ffi.cursor.core);
    _select(moved, '1');
    await _settle();
    expect(moved.cursor.fetched, ['1']);
    expect(moved.cursorModel.cache?.id, '1');
    expect(moved.cursorModel.image?.width, 16);
  });

  test('a shape keeps one native cursor, the one at its raster', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    final first = registered.single;

    // Asks for its pixels back, registers the new raster, then is shown again.
    for (var i = 0; i < 3; i++) {
      buildCursorOfCache(cursor, 0.5, cursor.cache);
      await _settle();
    }
    expect(registered.length, 2);
    expect(deleted, [first],
        reason: 'the replaced one goes once its successor has been shown');
    expect(cursor.cachedKeys, {registered.last});
  });

  test('a raster returned to before its cursor was dropped is kept', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    for (final scale in [1.0, 0.5, 0.5, 1.0, 1.0]) {
      buildCursorOfCache(cursor, scale, cursor.cache);
      await _settle();
    }
    final shown = cursor.nativeKey(cursor.cache!, 1.0);
    expect(buildCursorOfCache(cursor, 1.0, cursor.cache),
        isNot(MouseCursor.defer));
    await _settle();
    expect(deleted, isNot(contains(shown)), reason: 'it is the one on screen');
    expect(cursor.cachedKeys, {shown});
  });

  test('a cleared session forgets the native cursors it deleted', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    cursor.clear();
    await _settle();
    expect(deleted, registered);
    expect(cursor.cachedKeys, isEmpty);
  });

  test('a tab closing does not delete the cursors of another tab', () async {
    final other = _FFI();
    addTearDown(other.cursorModel.disposeImages);
    while (preDefaultCursor.cache == null) {
      await _settle();
    }
    buildCursorOfCache(ffi.cursorModel, 1.0, preDefaultCursor.cache);
    buildCursorOfCache(other.cursorModel, 1.0, preDefaultCursor.cache);
    await _settle();
    expect(registered.toSet().length, 2, reason: 'each tab has its own');

    ffi.cursorModel.clear();
    await _settle();
    expect(other.cursorModel.cachedKeys.single, isNot(isIn(deleted)));
  });

  test('a shape given as a view into a larger buffer is read from the view',
      () async {
    final pixels = _pixels(8, 3);
    final padded = Uint8List(16 + pixels.length)
      ..setRange(16, 16 + pixels.length, pixels);
    await ffi.ffiModel
        .handleCursorData('1', 0, 0, 8, 8, Uint8List.sublistView(padded, 16));
    final p = ffi.cursorModel.cache!.image.getPixel(0, 0);
    expect([p.r, p.g, p.b, p.a], [3, 4, 5, 6]);
  });
}

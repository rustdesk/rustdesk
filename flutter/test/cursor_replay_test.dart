import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';
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
  Future<CursorShape?> fetchCursorShape(String id) {
    fetched.add(id);
    return Future.value(core[id]);
  }

  @override
  bool get showsRemoteCursor => false;
}

String? _key(MouseCursor cursor) =>
    cursor is FlutterCustomMemoryImageCursor ? cursor.key : null;

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
  for (var i = 0; i < 2; i++) {
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

  test(
      'only the shape in use keeps an image, and a shape its native cursor holds '
      'keeps no pixels', () async {
    final cursor = ffi.cursorModel;
    for (var round = 0; round < 3; round++) {
      for (final (id, seed) in [('1', 1), ('2', 2), ('3', 3)]) {
        await _feed(ffi, id, seed: seed);
        buildCursorOfCache(cursor, 1.0, cursor.cache);
        await _settle();
      }
    }
    expect(ffi.cursorModel.shapeIds, ['3']);
    for (final id in ['1', '2']) {
      expect(ffi.cursorModel.cachedShape(id)!.hasPixels, isFalse);
      expect(ffi.cursorModel.cachedShape(id)!.data, isNull);
    }
    expect(ffi.cursor.fetched, isEmpty);
  });

  test('the shape in use keeps its pixels, and a new raster is made at once',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    expect(cursor.cache!.hasPixels, isTrue);
    expect(buildCursorOfCache(cursor, 0.5, cursor.cache),
        isNot(MouseCursor.defer));
    await _settle();
    expect(ffi.cursor.fetched, isEmpty, reason: 'nothing had to be decoded');
    expect(registered.length, 2);
    expect((cursor.cache!.rasterWidth, cursor.cache!.rasterHeight), (16, 16));

    await _feed(ffi, '2');
    expect(cursor.cachedShape('1')!.hasPixels, isFalse,
        reason: 'switched away from, a shape keeps no pixels');
    _select(ffi, '1');
    expect(buildCursorOfCache(cursor, 0.5, cursor.cache),
        isNot(MouseCursor.defer));
    await _settle();
    expect(registered.length, 2, reason: 'its native cursor is still there');
    expect(ffi.cursor.fetched, isEmpty);
  });

  test('native cursors stay for the session, every raster of every shape',
      () async {
    // A delete frees nothing on Windows, so none is deleted before the session ends.
    final cursor = ffi.cursorModel;
    for (var i = 0; i < 100; i++) {
      await _feed(ffi, '$i', size: 32);
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    expect(deleted, isEmpty);
    expect(cursor.cachedKeys.length, 100);

    _select(ffi, '0');
    expect(_key(buildCursorOfCache(cursor, 1.0, cursor.cache)),
        cursor.nativeKey(cursor.cache!, 1.0));
    await _settle();
    expect(ffi.cursor.fetched, isEmpty, reason: 'its cursor is still there');
    expect(registered.length, 100);
  });

  test('a new raster of a shape without pixels keeps the cursor shown before',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    await _feed(ffi, '2', size: 32);
    final shown = _key(buildCursorOfCache(cursor, 1.0, cursor.cache));
    await _settle();
    _select(ffi, '1'); // its native cursor held its pixels, so they went
    expect(_key(buildCursorOfCache(cursor, 0.5, cursor.cache)), shown);
    await _settle();
    expect(ffi.cursor.fetched, ['1']);
    expect(_key(buildCursorOfCache(cursor, 0.5, cursor.cache)),
        cursor.nativeKey(cursor.cache!, 0.5));
  });

  test('a shape painted again is decoded again, the last one shown meanwhile',
      () async {
    await _feed(ffi, '1', size: 16);
    await _feed(ffi, '2');
    _select(ffi, '1');
    expect(ffi.cursorModel.image?.width, 8, reason: 'shape 2 until 1 is back');
    await _settle();
    expect(ffi.cursor.fetched, ['1']);
    expect(ffi.cursorModel.image?.width, 16);
    expect(ffi.cursorModel.shapeIds, ['1']);
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
}

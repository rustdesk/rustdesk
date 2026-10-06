import 'dart:async';
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
  // While set, a fetch answers only when [answerFetches] is called.
  bool holdFetches = false;
  // While set, a fetch throws before it returns a future.
  bool throwAtOnce = false;
  final _held = <Completer<CursorShape?>, String>{};
  @override
  Future<CursorShape?> fetchCursorShape(String id) {
    fetched.add(id);
    if (throwAtOnce) throw StateError('no core');
    if (!holdFetches) return Future.value(core[id]);
    final answer = Completer<CursorShape?>();
    _held[answer] = id;
    return answer.future;
  }

  void answerFetches() {
    final held = Map.of(_held);
    _held.clear();
    held.forEach((answer, id) => answer.complete(core[id]));
  }

  void failFetches() {
    final held = Map.of(_held);
    _held.clear();
    held.forEach((answer, _) => answer.completeError(StateError('failed')));
  }

  bool showRemoteCursor = false;
  @override
  bool get showsRemoteCursor => showRemoteCursor;
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

  test(
      'a raster made before is found again after the shape was switched away from',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, 'A', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    final first = registered.single;
    buildCursorOfCache(cursor, 0.5, cursor.cache);
    await _settle();
    await _feed(ffi, 'B');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    final count = registered.length;

    _select(ffi, 'A'); // its pixels went; its cursors at both rasters stay
    expect(_key(buildCursorOfCache(cursor, 1.0, cursor.cache)), first);
    await _settle();
    expect(ffi.cursor.fetched, isEmpty);
    expect(registered.length, count);
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

  test('shapes that keep decoding late still get their native cursors',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, 'arrow');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    // A and B arrive, each decoding only after the peer moved on.
    for (final (id, seed) in [('A', 1), ('B', 2)]) {
      final pixels = _pixels(8, seed);
      ffi.cursor.core[id] =
          CursorShape(hotx: 0, hoty: 0, width: 8, height: 8, colors: pixels);
      final decoding = ffi.ffiModel.handleCursorData(id, 0, 0, 8, 8, pixels);
      _select(ffi, 'arrow');
      await decoding;
    }
    // Each fetch answers only after the next switch.
    ffi.cursor.holdFetches = true;
    for (var i = 0; i < 6; i++) {
      _select(ffi, i.isEven ? 'A' : 'B');
      ffi.cursor.answerFetches();
      await _settle();
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    expect(registered.where((key) => key.contains('_A_')), isNotEmpty);
    expect(registered.where((key) => key.contains('_B_')), isNotEmpty);
  });

  test('a zoom still gets native cursors for shapes that keep decoding late',
      () async {
    final cursor = ffi.cursorModel;
    // An animation whose frames have native cursors at 1.0 only.
    for (final (id, seed) in [('A', 1), ('B', 2)]) {
      await _feed(ffi, id, size: 32, seed: seed);
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    // At 0.5 each frame is fetched again, and each fetch answers after the next switch.
    ffi.cursor.holdFetches = true;
    for (var i = 0; i < 6; i++) {
      _select(ffi, i.isEven ? 'A' : 'B');
      ffi.cursor.answerFetches();
      await _settle();
      buildCursorOfCache(cursor, 0.5, cursor.cache);
      await _settle();
    }
    expect(
        registered
            .where((key) => key.contains('_A_') && key.endsWith('_16_16')),
        isNotEmpty);
    expect(
        registered
            .where((key) => key.contains('_B_') && key.endsWith('_16_16')),
        isNotEmpty);
  });

  test(
      'a raster still missing keeps a late restore, whatever was shown meanwhile',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, 'A', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    await _feed(ffi, 'B');
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();

    _select(ffi, 'A');
    ffi.cursor.holdFetches = true;
    buildCursorOfCache(cursor, 0.5, cursor.cache); // no cursor at 0.5: fetched
    buildCursorOfCache(cursor, 1.0, cursor.cache); // 1.0 has one meanwhile
    _select(ffi, 'B');
    ffi.cursor.answerFetches();
    await _settle();
    ffi.cursor.fetched.clear();

    _select(ffi, 'A');
    buildCursorOfCache(cursor, 0.5, cursor.cache);
    await _settle();
    expect(ffi.cursor.fetched, isEmpty,
        reason: 'its pixels waited for the cursor at 0.5');
    expect(
        registered
            .where((key) => key.contains('_A_') && key.endsWith('_16_16')),
        isNotEmpty);
  });

  test('shapes waiting for a native cursor keep their pixels, within the limit',
      () async {
    const max = CursorModel.kRecentShapes;
    for (var i = 0; i <= max + 1; i++) {
      await _feed(ffi, '$i');
    }
    expect(ffi.cursorModel.cachedShape('0')!.hasPixels, isFalse,
        reason: 'the one waiting longest is let go');
    expect(ffi.cursorModel.cachedShape('1')!.hasPixels, isTrue);
    expect(ffi.cursorModel.cachedShape('$max')!.hasPixels, isTrue);
  });

  test('two animated cursors and the everyday set keep their native cursors',
      () async {
    // Each frame is a shape of its own: 18 for the Windows busy cursor, 23 for
    // KDE Breeze's wait and progress cursors on X11.
    const frames = 23;
    const statics = 12;
    final cursor = ffi.cursorModel;
    final shapes = [
      for (var i = 0; i < statics; i++) 's$i',
      for (var i = 0; i < frames; i++) 'wait$i',
      for (var i = 0; i < frames; i++) 'progress$i',
    ];
    for (var i = 0; i < shapes.length; i++) {
      await _feed(ffi, shapes[i], seed: i);
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    ffi.cursor.fetched.clear();
    for (final id in shapes) {
      _select(ffi, id);
      buildCursorOfCache(cursor, 1.0, cursor.cache);
      await _settle();
    }
    expect(ffi.cursor.fetched, isEmpty,
        reason: 'every shape kept its native cursor');
    expect(registered.length, shapes.length);
    expect(deleted, isEmpty);
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
    expect(ffi.cursorModel.cachedShape('1')!.hasPixels, isTrue,
        reason:
            'decoded after the peer moved on, it waits for its native cursor');
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

  test('a shape keeps a native cursor for each raster it was shown at',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1', size: 32);
    for (final scale in [1.0, 0.5, 1.0, 0.5]) {
      buildCursorOfCache(cursor, scale, cursor.cache);
      await _settle();
    }
    expect(registered.length, 2, reason: 'a raster returned to is reused');
    expect(deleted, isEmpty);
    expect(cursor.cachedKeys, registered.toSet());
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

  test('a shape still decoding leaves the one shown before in place', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1');
    final shown = _key(buildCursorOfCache(cursor, 1.0, cursor.cache));
    await _settle();
    _select(ffi, '2'); // its shape has not decoded yet
    expect(cursor.cache?.id, '1');
    expect(_key(buildCursorOfCache(cursor, 1.0, cursor.cache)), shown);
    expect(cursor.image, isNotNull);
    await _feed(ffi, '2');
    expect(cursor.cache?.id, '2');
  });

  for (final how in ['does not have', 'cannot decode']) {
    test('a shape the core $how leaves the default cursor, not the one before',
        () async {
      final cursor = ffi.cursorModel;
      await _feed(ffi, '1');
      if (how == 'cannot decode') {
        ffi.cursor.core['2'] = CursorShape(
            hotx: 0, hoty: 0, width: 8, height: 8, colors: Uint8List(3));
      }
      _select(ffi, '2');
      expect(cursor.cache?.id, '1', reason: 'kept while the shape may come');
      var notified = 0;
      cursor.addListener(() => notified++);
      await _settle();
      expect(cursor.cache, isNull,
          reason: 'the desktop shows preDefaultCursor');
      expect(cursor.image, isNull, reason: 'and so does a painted cursor');
      expect(notified, greaterThan(0));
    });
  }

  test('a shape sent that does not decode leaves the default cursor', () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1');
    ffi.cursor.core['2'] = CursorShape(
        hotx: 0, hoty: 0, width: 8, height: 8, colors: Uint8List(3));
    await ffi.ffiModel.handleCursorData('2', 0, 0, 8, 8, Uint8List(3));
    expect(cursor.cache, isNull);
    expect(cursor.image, isNull);
    _select(ffi, '2');
    await _settle();
    expect(ffi.cursor.fetched, isEmpty, reason: 'it is not asked for either');
  });

  test('a fetch failing after the session was cleared marks nothing in it',
      () async {
    final cursor = ffi.cursorModel;
    ffi.cursor.holdFetches = true;
    _select(ffi, '1'); // not decoded here yet, so it is asked for
    cursor.clear();
    ffi.cursor.failFetches();
    await _settle();
    ffi.cursor.holdFetches = false;
    _select(ffi, '1'); // the same id, now in the new session
    await _settle();
    expect(ffi.cursor.fetched, ['1', '1']);
  });

  test('a fetch that throws at once tells the listeners after the build',
      () async {
    final cursor = ffi.cursorModel;
    await _feed(ffi, '1');
    ffi.cursor.throwAtOnce = true;
    cursor.id = '2';
    var notified = 0;
    cursor.addListener(() => notified++);
    cursor.restorePixels('2');
    expect(notified, 0, reason: 'a build may be what asked');
    await _settle();
    expect(notified, greaterThan(0));
    expect(cursor.cache, isNull);
  });

  test('a raster still being made falls back to a shape, not the forbidden one',
      () async {
    final cursor = ffi.cursorModel;
    for (var i = 0; preForbiddenCursor.cache == null && i < 100; i++) {
      await _settle();
    }
    expect(preForbiddenCursor.cache, isNotNull);
    await _feed(ffi, '1', size: 32);
    buildCursorOfCache(cursor, 1.0, cursor.cache);
    await _settle();
    await _feed(ffi, '2', size: 32);
    final shown = _key(buildCursorOfCache(cursor, 1.0, cursor.cache));
    await _settle();
    buildCursorOfCache(cursor, 1.0, preForbiddenCursor.cache); // input disabled
    _select(ffi, '1'); // its native cursor held its pixels, so they went
    expect(_key(buildCursorOfCache(cursor, 0.5, cursor.cache)), shown);
  });

  group('a painted cursor', () {
    void painted(String how) {
      if (how == 'mobile') {
        final was = isMobile;
        isMobile = true;
        addTearDown(() => isMobile = was);
      } else {
        ffi.cursor.showRemoteCursor = true;
      }
    }

    for (final how in ['mobile', 'remote cursor shown']) {
      test('keeps the images of two animations and the everyday set ($how)',
          () async {
        painted(how);
        final cursor = ffi.cursorModel;
        final shapes = [
          for (var i = 0; i < 12; i++) 's$i',
          for (var i = 0; i < 23; i++) 'wait$i',
          for (var i = 0; i < 23; i++) 'progress$i',
        ];
        for (var i = 0; i < shapes.length; i++) {
          await _feed(ffi, shapes[i], seed: i);
        }
        for (final id in shapes) {
          _select(ffi, id);
          expect(cursor.shapeIds.last, id, reason: 'its image is at hand');
        }
        await _settle();
        expect(ffi.cursor.fetched, isEmpty);
      });
    }

    test('keeps the images used last, and decodes the others again', () async {
      painted('mobile');
      const max = CursorModel.kRecentShapes;
      final cursor = ffi.cursorModel;
      for (var i = 0; i <= max; i++) {
        await _feed(ffi, '$i');
      }
      expect(cursor.shapeIds.length, max);
      _select(ffi, '$max');
      _select(ffi, '1');
      await _settle();
      expect(ffi.cursor.fetched, isEmpty, reason: 'shape 1 was used lately');
      _select(ffi, '0');
      expect(cursor.image, isNotNull, reason: 'shape 1 until 0 is back');
      await _settle();
      expect(ffi.cursor.fetched, ['0']);
      expect(cursor.shapeIds.length, max);
    });

    test('keeps a shape that finished decoding after the peer moved on',
        () async {
      painted('mobile');
      const max = CursorModel.kRecentShapes;
      final cursor = ffi.cursorModel;
      for (var i = 0; i <= max; i++) {
        await _feed(ffi, '$i');
      }
      _select(ffi, '0');
      cursor.image; // let go, so it is asked for
      await Future<void>.delayed(Duration.zero);
      _select(ffi, '1');
      await _settle();
      _select(ffi, '0');
      cursor.image;
      await _settle();
      expect(ffi.cursor.fetched, ['0'], reason: 'the late one was kept');
      expect(cursor.shapeIds.last, '0');
    });
  });
}

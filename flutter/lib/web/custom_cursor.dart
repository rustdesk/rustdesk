import 'dart:convert';
import 'dart:js' as js;
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'package:flutter_hbb/models/model.dart' as model;

class CursorData {
  final String key;
  final String url;
  final double hotX;
  final double hotY;
  final int width;
  final int height;

  CursorData({
    required this.key,
    required this.url,
    required this.hotX,
    required this.hotY,
    required this.width,
    required this.height,
  });
}

/// The cursor manager
class CursorManager {
  final Map<String, CursorData> _cursors = <String, CursorData>{};
  String latestKey = '';

  CursorManager._();
  static CursorManager instance = CursorManager._();

  Future<void> registerCursor(CursorData data) async {
    _cursors[data.key] = data;
  }

  Future<void> deleteCursor(String key) async {
    _cursors.remove(key);
  }

  Future<void> setSystemCursor(String key) async {
    if (latestKey == key) {
      return;
    }
    latestKey = key;

    final CursorData? cursorData = _cursors[key];
    if (cursorData != null) {
      js.context.callMethod('setByName', [
        'cursor',
        jsonEncode({
          'url': cursorData.url,
          // Rounding must keep the hotspot inside even a one-pixel raster.
          'hotx': cursorData.hotX.round().clamp(0, cursorData.width - 1),
          'hoty': cursorData.hotY.round().clamp(0, cursorData.height - 1),
        })
      ]);
    }
  }

  Future<void> resetSystemCursor() async {
    latestKey = '';
    js.context.callMethod('setByName', ['cursor', 'auto']);
  }
}

class FlutterCustomMemoryImageCursor extends MouseCursor {
  final String key;
  const FlutterCustomMemoryImageCursor({required this.key});

  @override
  MouseCursorSession createSession(int device) =>
      _FlutterCustomMemoryImageCursorSession(this, device);

  @override
  String get debugDescription =>
      objectRuntimeType(this, 'FlutterCustomMemoryImageCursor');
}

class _FlutterCustomMemoryImageCursorSession extends MouseCursorSession {
  _FlutterCustomMemoryImageCursorSession(
      FlutterCustomMemoryImageCursor cursor, int device)
      : super(cursor, device);

  @override
  FlutterCustomMemoryImageCursor get cursor =>
      super.cursor as FlutterCustomMemoryImageCursor;

  @override
  Future<void> activate() async {
    await CursorManager.instance.setSystemCursor(cursor.key);
  }

  @override
  void dispose() {}
}

deleteCustomCursor(String key) => CursorManager.instance.deleteCursor(key);
resetSystemCursor() => CursorManager.instance.resetSystemCursor();

MouseCursor buildCursorOfCache(
    model.CursorModel cursor, double scale, model.CursorData? cache) {
  if (cache == null) {
    return MouseCursor.defer;
  } else {
    // A short-edge minimum can enlarge thin artwork beyond CSS cursor limits.
    // Keep unzoomed images unchanged and use the long edge when resizing.
    final key = cache.updateGetKey(scale, useLegacyMinimum: scale == 1.0);
    if (key == null) return MouseCursor.defer;
    if (!cursor.cachedKeys.contains(key)) {
      // data should be checked here, because it may be changed after `updateGetKey()`
      final data = cache.data;
      if (data == null) {
        return MouseCursor.defer;
      }
      debugPrint(
          "Register custom cursor with key $key (${cache.hotx},${cache.hoty})");
      CursorManager.instance.registerCursor(CursorData(
          key: key,
          url: 'data:image/rgba;base64,${base64Encode(data)}',
          width: max(1, (cache.width * cache.scale).round()),
          height: max(1, (cache.height * cache.scale).round()),
          hotX: cache.hotx,
          hotY: cache.hoty));
      cursor.addKey(key);
    }
    return FlutterCustomMemoryImageCursor(key: key);
  }
}

import 'dart:math';

import 'package:flutter_custom_cursor/cursor_manager.dart'
    as custom_cursor_manager;
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:image/image.dart' as img;

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';

deleteCustomCursor(String key) =>
    custom_cursor_manager.CursorManager.instance.deleteCursor(key);
resetSystemCursor() {}

double _nativeHotspot(double hotspot, int bitmapSize) {
  if (!isLinux) return hotspot;
  // GDK takes integer hotspots inside the bitmap; avoid truncating toward zero.
  return min(hotspot.round(), bitmapSize - 1).toDouble();
}

Uint8List _padCursorWidth(Uint8List data, int width) {
  final bitmap = img.decodePng(data);
  if (bitmap == null) {
    throw const FormatException('Invalid native cursor PNG');
  }
  final padded = img.copyExpandCanvas(bitmap,
      newWidth: width,
      newHeight: bitmap.height,
      position: img.ExpandCanvasPosition.topLeft,
      toImage: img.Image(width: width, height: bitmap.height, numChannels: 4));
  return Uint8List.fromList(img.encodePng(padded));
}

MouseCursor buildCursorOfCache(
    CursorModel cursor, double scale, CursorData? cache) {
  if (cache == null) {
    return MouseCursor.defer;
  } else {
    final key = cache.updateGetKey(scale);
    if (!cursor.cachedKeys.contains(key)) {
      // data should be checked here, because it may be changed after `updateGetKey()`
      final data = cache.data;
      if (data == null) {
        return MouseCursor.defer;
      }
      // Pad tall Linux buffers to prevent clipping in the hardware cursor plane.
      final width = isLinux
          ? max(cache.scaledWidth, cache.scaledHeight)
          : cache.scaledWidth;
      final buffer =
          width == cache.scaledWidth ? data : _padCursorWidth(data, width);
      debugPrint(
          "Register custom cursor with key $key (${cache.hotx},${cache.hoty})");
      // [Safety]
      // It's ok to call async registerCursor in current synchronous context,
      // because activating the cursor is also an async call and will always
      // be executed after this.
      custom_cursor_manager.CursorManager.instance
          .registerCursor(custom_cursor_manager.CursorData()
            ..name = key
            ..buffer = buffer
            ..width = width
            ..height = cache.scaledHeight
            ..hotX = _nativeHotspot(cache.hotx, cache.scaledWidth)
            ..hotY = _nativeHotspot(cache.hoty, cache.scaledHeight));
      cursor.addKey(key);
    }
    return FlutterCustomMemoryImageCursor(key: key);
  }
}

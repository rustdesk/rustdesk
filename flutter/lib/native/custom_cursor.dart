import 'package:flutter_custom_cursor/cursor_manager.dart'
    as custom_cursor_manager;
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'package:flutter_hbb/common.dart' show isLinux;
import 'package:flutter_hbb/models/model.dart';
import 'package:image/image.dart' as img;

deleteCustomCursor(String key) =>
    custom_cursor_manager.CursorManager.instance.deleteCursor(key);
resetSystemCursor() {}

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
      // Square canvases avoid clipping or stray edge pixels on Linux.
      final width = isLinux && cache.rasterWidth < cache.rasterHeight
          ? cache.rasterHeight
          : cache.rasterWidth;
      final height = isLinux ? width : cache.rasterHeight;
      debugPrint(
          "Register custom cursor with key $key (${cache.hotx},${cache.hoty})");
      // [Safety]
      // It's ok to call async registerCursor in current synchronous context,
      // because activating the cursor is also an async call and will always
      // be executed after this.
      custom_cursor_manager.CursorManager.instance
          .registerCursor(custom_cursor_manager.CursorData()
            ..name = key
            ..buffer =
                width == cache.rasterWidth && height == cache.rasterHeight
                    ? data
                    : _padCursor(data, width)
            ..width = width
            ..height = height
            ..hotX = cache.hotx
            ..hotY = cache.hoty);
      cursor.addKey(key);
    }
    return FlutterCustomMemoryImageCursor(key: key);
  }
}

Uint8List _padCursor(Uint8List data, int size) {
  final bitmap = img.decodePng(data);
  if (bitmap == null) {
    throw const FormatException('Invalid native cursor PNG');
  }
  final padded = img.copyExpandCanvas(bitmap,
      newWidth: size,
      newHeight: size,
      position: img.ExpandCanvasPosition.topLeft,
      toImage: img.Image(width: size, height: size, numChannels: 4));
  return Uint8List.fromList(img.encodePng(padded));
}

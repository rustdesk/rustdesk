import 'package:flutter_custom_cursor/cursor_manager.dart'
    as custom_cursor_manager;
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'package:flutter_hbb/models/model.dart';

deleteCustomCursor(String key) =>
    custom_cursor_manager.CursorManager.instance.deleteCursor(key);

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
      debugPrint(
          "Register custom cursor with key $key (${cache.hotx},${cache.hoty})");
      // [Safety]
      // It's ok to call async registerCursor in current synchronous context,
      // because activating the cursor is also an async call and will always
      // be executed after this.
      custom_cursor_manager.CursorManager.instance
          .registerCursor(custom_cursor_manager.CursorData()
            ..name = key
            ..buffer = data
            ..width = (cache.width * cache.scale).toInt()
            ..height = (cache.height * cache.scale).toInt()
            ..hotX = cache.hotx
            ..hotY = cache.hoty);
      cursor.addKey(key);
    }
    return FlutterCustomMemoryImageCursor(key: key);
  }
}

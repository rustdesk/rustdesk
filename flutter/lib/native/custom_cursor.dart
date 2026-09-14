import 'dart:async';
import 'dart:math' as math;

import 'package:flutter_custom_cursor/cursor_manager.dart'
    as custom_cursor_manager;
import 'package:flutter_custom_cursor/flutter_custom_cursor.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show WidgetsBinding;

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';

deleteCustomCursor(String key) =>
    custom_cursor_manager.CursorManager.instance.deleteCursor(key);
resetSystemCursor() {}

MouseCursor buildCursorOfCache(
    CursorModel cursor, double scale, CursorData? cache) {
  if (cache == null) {
    return MouseCursor.defer;
  } else {
    // Include the live DPR so moving between monitors rebuilds the native
    // bitmap even when the remote view scale has not changed.
    final dpr = WidgetsBinding
        .instance.platformDispatcher.views.single.devicePixelRatio;
    // Keep Original and older peers' pixel units and scale-1 behavior.
    // Resizing uses a long-edge minimum regardless of source density.
    final legacyMinimum = cache.pixelRatio == 0 ||
        cursor.parent.target?.canvasModel.viewStyle.style == kRemoteViewStyleOriginal;
    // The minimum is logical, while Windows callers pass a physical scale.
    final effectiveScale = !legacyMinimum && isWindows
        ? math.max(scale, kMinCursorSize * dpr / math.max(cache.width, cache.height))
        : scale;
    final cacheKey = cache.updateGetKey(effectiveScale, resizeImage: false,
        useLegacyMinimum: legacyMinimum,
        rasterScale: isWindows ? 1 : (isLinux ? dpr.ceilToDouble() : dpr));
    if (cacheKey == null) return MouseCursor.defer;
    final key = '${cacheKey}_$dpr';
    if (!cursor.cachedKeys.contains(key)) {
      debugPrint(
          "Register custom cursor with key $key (${cache.hotx},${cache.hoty})");
      unawaited(custom_cursor_manager.CursorManager.instance
          .registerCursorImage(
        name: key,
        image: cache.nativeImage,
        hotSpot: Offset(cache.hotxOrigin, cache.hotyOrigin),
        // Windows callers already express scale in physical pixels.
        // The plugin takes logical scale and applies DPR during rasterization.
        scale: isWindows ? cache.scale / dpr : cache.scale,
        devicePixelRatio: dpr,
      )
          .then<void>((_) {}, onError: (Object error, StackTrace stack) {
        cursor.cachedKeys.remove(key);
        FlutterError.reportError(FlutterErrorDetails(
            exception: error,
            stack: stack,
            library: 'native cursor',
            context: ErrorDescription('registering cursor $key')));
      }));
      cursor.addKey(key);
    }
    return FlutterCustomMemoryImageCursor(
        key: key,
        registrationToken: custom_cursor_manager.CursorManager.instance
            .registrationTokenFor(key));
  }
}

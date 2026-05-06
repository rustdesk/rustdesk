import 'dart:math';
import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';

const _maxRemoteCanvasMargin = 400.0;
const _nearEdgeThreshold = 3.0;

// Minimal test fixtures for the pure canvas-margin math. These intentionally
// cover only the fields used by pointer mapping.
enum ScrollStyle { scrollbar, scrollauto }

class CanvasCoords {
  double x = 0;
  double y = 0;
  double scale = 1.0;
  double scrollX = 0;
  double scrollY = 0;
  double displayWidth = 0;
  double displayHeight = 0;
  double paddingX = 0;
  double paddingY = 0;
  ScrollStyle scrollStyle = ScrollStyle.scrollauto;
  Size size = Size.zero;
}

double clampMargin(double value) {
  return min(_maxRemoteCanvasMargin, max(0, value));
}

int normalizeMarginForStorage(double value) {
  return value.clamp(0, _maxRemoteCanvasMargin).round();
}

Rect? computePaddedRect(Rect? realRect, double margin) {
  if (realRect == null) return null;
  if (margin <= 0) return realRect;
  return Rect.fromLTRB(
    realRect.left - margin,
    realRect.top - margin,
    realRect.right + margin,
    realRect.bottom + margin,
  );
}

double computeDisplayPaddingX(Rect? paddedRect, Rect? realRect) {
  if (paddedRect == null || realRect == null) return 0;
  return realRect.left - paddedRect.left;
}

double computeDisplayPaddingY(Rect? paddedRect, Rect? realRect) {
  if (paddedRect == null || realRect == null) return 0;
  return realRect.top - paddedRect.top;
}

double computeAdaptiveScale({
  required double viewWidth,
  required double viewHeight,
  required int displayWidth,
  required int displayHeight,
}) {
  if (viewWidth == 0 ||
      viewHeight == 0 ||
      displayWidth == 0 ||
      displayHeight == 0) {
    return 1.0;
  }
  return min(viewWidth / displayWidth, viewHeight / displayHeight);
}

(double, double) computeCanvasOffset(
    Size viewSize, int displayWidth, int displayHeight, double scale) {
  final x = (viewSize.width - displayWidth * scale) / 2;
  final y = (viewSize.height - displayHeight * scale) / 2;
  return (x, y);
}

(double, double)? computePointerPosition({
  required double pointerX,
  required double pointerY,
  required CanvasCoords canvas,
  required Rect remoteRect,
}) {
  double x = pointerX;
  double y = pointerY;

  final nearRight = (canvas.size.width - x) < _nearEdgeThreshold;
  final nearBottom = (canvas.size.height - y) < _nearEdgeThreshold;
  final displayWidth =
      canvas.displayWidth > 0 ? canvas.displayWidth : remoteRect.width;
  final displayHeight =
      canvas.displayHeight > 0 ? canvas.displayHeight : remoteRect.height;
  final imageWidth = displayWidth * canvas.scale;
  final imageHeight = displayHeight * canvas.scale;

  if (canvas.scrollStyle != ScrollStyle.scrollauto) {
    x += imageWidth * canvas.scrollX;
    y += imageHeight * canvas.scrollY;

    if (canvas.size.width > imageWidth) {
      x -= ((canvas.size.width - imageWidth) / 2);
    }
    if (canvas.size.height > imageHeight) {
      y -= ((canvas.size.height - imageHeight) / 2);
    }
  } else {
    x -= canvas.x;
    y -= canvas.y;
  }

  x /= canvas.scale;
  y /= canvas.scale;
  if (canvas.scale > 0 && canvas.scale < 1) {
    final step = 1.0 / canvas.scale - 1;
    if (nearRight) {
      x += step;
    }
    if (nearBottom) {
      y += step;
    }
  }

  final paddedX = x;
  final paddedY = y;
  x = paddedX - canvas.paddingX + remoteRect.left;
  y = paddedY - canvas.paddingY + remoteRect.top;

  final insidePaddedRect = paddedX >= 0 &&
      paddedY >= 0 &&
      paddedX <= displayWidth &&
      paddedY <= displayHeight;
  if (insidePaddedRect) {
    x = x.clamp(remoteRect.left, remoteRect.right).toDouble();
    y = y.clamp(remoteRect.top, remoteRect.bottom).toDouble();
  }

  return (x, y);
}

void main() {
  group('Remote canvas margin math', () {
    test('clamps and normalizes margin values', () {
      expect(clampMargin(-10), 0);
      expect(clampMargin(50), 50);
      expect(clampMargin(999), _maxRemoteCanvasMargin);

      expect(normalizeMarginForStorage(50.7), 51);
      expect(normalizeMarginForStorage(-5), 0);
      expect(normalizeMarginForStorage(999), _maxRemoteCanvasMargin.toInt());
    });

    test('expands remote rect and derives display padding', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final paddedRect = computePaddedRect(realRect, 100)!;

      expect(paddedRect, Rect.fromLTRB(-100, -100, 2020, 1180));
      expect(computeDisplayPaddingX(paddedRect, realRect), 100);
      expect(computeDisplayPaddingY(paddedRect, realRect), 100);
    });

    test('margin-expanded display affects adaptive scale and centering', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final paddedRect = computePaddedRect(realRect, 100)!;
      final displayWidth = paddedRect.width.toInt();
      final displayHeight = paddedRect.height.toInt();
      final scale = computeAdaptiveScale(
        viewWidth: 1920,
        viewHeight: 1080,
        displayWidth: displayWidth,
        displayHeight: displayHeight,
      );
      final (x, y) = computeCanvasOffset(
          Size(1920, 1080), displayWidth, displayHeight, scale);

      expect(scale, closeTo(1080 / 1280, 0.0001));
      expect(x, closeTo((1920 - 2120 * scale) / 2, 0.01));
      expect(y, closeTo(0, 0.01));
    });
  });

  group('Pointer coordinate transforms', () {
    test('no margin maps pointer directly in scrollauto mode', () {
      final canvas = CanvasCoords()
        ..scale = 1.0
        ..displayWidth = 1920
        ..displayHeight = 1080
        ..size = Size(1920, 1080);
      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      final result = computePointerPosition(
          pointerX: 960, pointerY: 540, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(960, 0.01));
      expect(result.$2, closeTo(540, 0.01));
    });

    test('margin padding offsets pointer coordinates', () {
      final canvas = CanvasCoords()
        ..displayWidth = 2120
        ..displayHeight = 1280
        ..paddingX = 100
        ..paddingY = 100
        ..scale = 1.0
        ..size = Size(2120, 1280);
      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      final result = computePointerPosition(
          pointerX: 100, pointerY: 100, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(0, 0.01));
      expect(result.$2, closeTo(0, 0.01));
    });

    test('pointer in margin area clamps to remote rect boundary', () {
      final canvas = CanvasCoords()
        ..displayWidth = 2120
        ..displayHeight = 1280
        ..paddingX = 100
        ..paddingY = 100
        ..scale = 1.0
        ..size = Size(2120, 1280);
      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      final result = computePointerPosition(
          pointerX: 50, pointerY: 50, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(0, 0.01));
      expect(result.$2, closeTo(0, 0.01));
    });

    test('margin and adaptive scale map view center to remote center', () {
      final scale = 1080.0 / 2360;
      final canvas = CanvasCoords()
        ..displayWidth = 4040
        ..displayHeight = 2360
        ..paddingX = 100
        ..paddingY = 100
        ..scale = scale
        ..x = (1920 - 4040 * scale) / 2
        ..y = (1080 - 2360 * scale) / 2
        ..size = Size(1920, 1080);
      final remoteRect = Rect.fromLTWH(0, 0, 3840, 2160);

      final result = computePointerPosition(
          pointerX: 960, pointerY: 540, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(1920, 1));
      expect(result.$2, closeTo(1080, 1));
    });

    test('zoomed-out near edge applies edge correction', () {
      final canvas = CanvasCoords()
        ..scale = 0.5
        ..displayWidth = 200
        ..displayHeight = 200
        ..size = Size(100, 100);
      final remoteRect = Rect.fromLTWH(0, 0, 200, 200);

      final result = computePointerPosition(
          pointerX: 99, pointerY: 99, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(199, 0.01));
      expect(result.$2, closeTo(199, 0.01));
    });
  });
}

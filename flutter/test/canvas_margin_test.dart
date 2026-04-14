import 'dart:math';
import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';

// =============================================================================
// Standalone replicas of production types and logic for testability.
//
// These mirror the pure math from CanvasModel, ViewStyle, CanvasCoords,
// and InputModel._handlePointerDevicePos without pulling in the full app
// dependency tree (which includes FFI bindings that can't run in test).
// =============================================================================

// --- Constants (from consts.dart) ---
const int kDesktopDefaultDisplayWidth = 1080;
const int kDesktopDefaultDisplayHeight = 720;
const int kMobileDefaultDisplayWidth = 720;
const int kMobileDefaultDisplayHeight = 1280;
const kRemoteViewStyleOriginal = 'original';
const kRemoteViewStyleAdaptive = 'adaptive';
const kRemoteViewStyleCustom = 'custom';

// --- ScrollStyle (from model.dart) ---
enum ScrollStyle { scrollbar, scrollauto, scrolledge }

// --- ViewStyle (from model.dart) ---
class ViewStyle {
  final String style;
  final double width;
  final double height;
  final int displayWidth;
  final int displayHeight;

  ViewStyle({
    required this.style,
    required this.width,
    required this.height,
    required this.displayWidth,
    required this.displayHeight,
  });

  static int _double2Int(double v) => (v * 100).round().toInt();

  @override
  bool operator ==(Object other) =>
      other is ViewStyle &&
      other.runtimeType == runtimeType &&
      _innerEqual(other);

  bool _innerEqual(ViewStyle other) {
    return style == other.style &&
        ViewStyle._double2Int(other.width) == ViewStyle._double2Int(width) &&
        ViewStyle._double2Int(other.height) == ViewStyle._double2Int(height) &&
        other.displayWidth == displayWidth &&
        other.displayHeight == displayHeight;
  }

  @override
  int get hashCode => Object.hash(
        style,
        ViewStyle._double2Int(width),
        ViewStyle._double2Int(height),
        displayWidth,
        displayHeight,
      ).hashCode;

  double get scale {
    double s = 1.0;
    if (style == kRemoteViewStyleAdaptive) {
      if (width != 0 &&
          height != 0 &&
          displayWidth != 0 &&
          displayHeight != 0) {
        final s1 = width / displayWidth;
        final s2 = height / displayHeight;
        s = s1 < s2 ? s1 : s2;
      }
    }
    return s;
  }
}

// --- CanvasCoords (from input_model.dart) ---
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

  CanvasCoords();

  Map<String, dynamic> toJson() {
    return {
      'x': x,
      'y': y,
      'scale': scale,
      'scrollX': scrollX,
      'scrollY': scrollY,
      'displayWidth': displayWidth,
      'displayHeight': displayHeight,
      'paddingX': paddingX,
      'paddingY': paddingY,
      'scrollStyle': scrollStyle.name,
      'size': {
        'w': size.width,
        'h': size.height,
      }
    };
  }

  static CanvasCoords fromJson(Map<String, dynamic> json) {
    final model = CanvasCoords();
    model.x = json['x'];
    model.y = json['y'];
    model.scale = json['scale'];
    model.scrollX = json['scrollX'];
    model.scrollY = json['scrollY'];
    model.displayWidth = (json['displayWidth'] ?? 0).toDouble();
    model.displayHeight = (json['displayHeight'] ?? 0).toDouble();
    model.paddingX = (json['paddingX'] ?? 0).toDouble();
    model.paddingY = (json['paddingY'] ?? 0).toDouble();
    model.scrollStyle = ScrollStyle.values.firstWhere(
      (e) => e.name == json['scrollStyle'],
      orElse: () => ScrollStyle.scrollauto,
    );
    model.size = Size(json['size']['w'], json['size']['h']);
    return model;
  }
}

// =============================================================================
// Helper functions replicating CanvasModel pure math
// =============================================================================

/// Replicates CanvasModel.remoteCanvasMargin clamping logic.
double clampMargin(double value) {
  return min(400, max(0, value));
}

/// Replicates CanvasModel.setRemoteCanvasMargin normalization logic.
int normalizeMarginForStorage(double value) {
  return value.clamp(0, 400).round();
}

/// Replicates CanvasModel.paddedRect computation.
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

/// Replicates CanvasModel.displayPaddingX computation.
double computeDisplayPaddingX(Rect? paddedRect, Rect? realRect) {
  if (paddedRect == null || realRect == null) return 0;
  return realRect.left - paddedRect.left;
}

/// Replicates CanvasModel.displayPaddingY computation.
double computeDisplayPaddingY(Rect? paddedRect, Rect? realRect) {
  if (paddedRect == null || realRect == null) return 0;
  return realRect.top - paddedRect.top;
}

/// Replicates CanvasModel.getDisplayWidth using paddedRect.
int computeDisplayWidth(Rect? paddedRect, {bool isDesktop = true}) {
  final defaultWidth =
      isDesktop ? kDesktopDefaultDisplayWidth : kMobileDefaultDisplayWidth;
  return paddedRect?.width.toInt() ?? defaultWidth;
}

/// Replicates CanvasModel.getDisplayHeight using paddedRect.
int computeDisplayHeight(Rect? paddedRect, {bool isDesktop = true}) {
  final defaultHeight =
      isDesktop ? kDesktopDefaultDisplayHeight : kMobileDefaultDisplayHeight;
  return paddedRect?.height.toInt() ?? defaultHeight;
}

/// Replicates CanvasModel._resetCanvasOffset centering computation.
(double, double) computeCanvasOffset(
    Size viewSize, int displayWidth, int displayHeight, double scale) {
  final x = (viewSize.width - displayWidth * scale) / 2;
  final y = (viewSize.height - displayHeight * scale) / 2;
  return (x, y);
}

/// Replicates the core coordinate transform math from
/// InputModel._handlePointerDevicePos.
(double, double)? computePointerPosition({
  required double pointerX,
  required double pointerY,
  required CanvasCoords canvas,
  required Rect remoteRect,
}) {
  double x = pointerX;
  double y = pointerY;

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

// =============================================================================
// Tests
// =============================================================================

void main() {
  // ===========================================================================
  // Margin clamping
  // ===========================================================================
  group('Margin clamping', () {
    test('clamps negative values to 0', () {
      expect(clampMargin(-10), 0);
      expect(clampMargin(-0.5), 0);
    });

    test('clamps values above 400 to 400', () {
      expect(clampMargin(500), 400);
      expect(clampMargin(999), 400);
    });

    test('passes through valid values', () {
      expect(clampMargin(0), 0);
      expect(clampMargin(50), 50);
      expect(clampMargin(200), 200);
      expect(clampMargin(400), 400);
    });

    test('normalizeMarginForStorage rounds to int', () {
      expect(normalizeMarginForStorage(50.7), 51);
      expect(normalizeMarginForStorage(50.3), 50);
      expect(normalizeMarginForStorage(-5), 0);
      expect(normalizeMarginForStorage(999), 400);
    });
  });

  // ===========================================================================
  // paddedRect computation
  // ===========================================================================
  group('paddedRect computation', () {
    test('returns null when realRect is null', () {
      expect(computePaddedRect(null, 50), isNull);
    });

    test('returns realRect unchanged when margin is 0', () {
      final rect = Rect.fromLTWH(0, 0, 1920, 1080);
      expect(computePaddedRect(rect, 0), rect);
    });

    test('returns realRect unchanged when margin is negative', () {
      final rect = Rect.fromLTWH(0, 0, 1920, 1080);
      expect(computePaddedRect(rect, -10), rect);
    });

    test('expands rect by margin on all sides', () {
      final rect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(rect, 50)!;
      expect(padded.left, -50);
      expect(padded.top, -50);
      expect(padded.right, 1970);
      expect(padded.bottom, 1130);
      expect(padded.width, 2020); // 1920 + 2*50
      expect(padded.height, 1180); // 1080 + 2*50
    });

    test('works with non-zero origin rect', () {
      final rect = Rect.fromLTRB(100, 200, 1920, 1080);
      final padded = computePaddedRect(rect, 100)!;
      expect(padded.left, 0);
      expect(padded.top, 100);
      expect(padded.right, 2020);
      expect(padded.bottom, 1180);
    });

    test('handles max margin (400)', () {
      final rect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(rect, 400)!;
      expect(padded.width, 2720); // 1920 + 2*400
      expect(padded.height, 1880); // 1080 + 2*400
    });
  });

  // ===========================================================================
  // displayPadding computation
  // ===========================================================================
  group('displayPadding computation', () {
    test('returns 0 when either rect is null', () {
      final rect = Rect.fromLTWH(0, 0, 100, 100);
      expect(computeDisplayPaddingX(null, rect), 0);
      expect(computeDisplayPaddingX(rect, null), 0);
      expect(computeDisplayPaddingY(null, rect), 0);
      expect(computeDisplayPaddingY(rect, null), 0);
    });

    test('padding equals the margin value', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final paddedRect = computePaddedRect(realRect, 75)!;
      expect(computeDisplayPaddingX(paddedRect, realRect), 75);
      expect(computeDisplayPaddingY(paddedRect, realRect), 75);
    });

    test('padding is 0 when margin is 0', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final paddedRect = computePaddedRect(realRect, 0)!;
      expect(computeDisplayPaddingX(paddedRect, realRect), 0);
      expect(computeDisplayPaddingY(paddedRect, realRect), 0);
    });

    test('padding equals margin for non-zero origin rect', () {
      final realRect = Rect.fromLTRB(100, 200, 1920, 1080);
      final paddedRect = computePaddedRect(realRect, 150)!;
      expect(computeDisplayPaddingX(paddedRect, realRect), 150);
      expect(computeDisplayPaddingY(paddedRect, realRect), 150);
    });
  });

  // ===========================================================================
  // Display dimensions with margin
  // ===========================================================================
  group('Display dimensions with margin', () {
    test('includes margin in display width and height', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(realRect, 50)!;
      expect(computeDisplayWidth(padded), 2020);
      expect(computeDisplayHeight(padded), 1180);
    });

    test('returns default dimensions when paddedRect is null', () {
      expect(computeDisplayWidth(null, isDesktop: true),
          kDesktopDefaultDisplayWidth);
      expect(computeDisplayHeight(null, isDesktop: true),
          kDesktopDefaultDisplayHeight);
    });

    test('no margin means display dimensions equal real rect', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(realRect, 0)!;
      expect(computeDisplayWidth(padded), 1920);
      expect(computeDisplayHeight(padded), 1080);
    });

    test('max margin (400) display dimensions', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(realRect, 400)!;
      expect(computeDisplayWidth(padded), 2720);
      expect(computeDisplayHeight(padded), 1880);
    });
  });

  // ===========================================================================
  // ViewStyle.scale with different display ratios and margins
  // ===========================================================================
  group('ViewStyle.scale computation', () {
    test('original style always returns scale 1.0', () {
      final vs = ViewStyle(
        style: kRemoteViewStyleOriginal,
        width: 1920,
        height: 1080,
        displayWidth: 1920,
        displayHeight: 1080,
      );
      expect(vs.scale, 1.0);
    });

    test('adaptive — same local and remote dimensions', () {
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 1920,
        displayHeight: 1080,
      );
      expect(vs.scale, 1.0);
    });

    test('adaptive — remote 2x larger (3840x2160 into 1920x1080)', () {
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 3840,
        displayHeight: 2160,
      );
      expect(vs.scale, 0.5);
    });

    test('adaptive — picks smaller ratio when aspect ratios differ', () {
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 3840,
        displayHeight: 1080,
      );
      // s1 = 1920/3840 = 0.5, s2 = 1080/1080 = 1.0 → 0.5
      expect(vs.scale, 0.5);
    });

    test('adaptive — margin-expanded display (margin=100 on 1920x1080)', () {
      // Padded: 2120x1280
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 2120,
        displayHeight: 1280,
      );
      // s1 = 1920/2120 ≈ 0.9057, s2 = 1080/1280 = 0.84375 → 0.84375
      expect(vs.scale, closeTo(0.84375, 0.0001));
    });

    test('adaptive — 2x remote scale, 1x local, margin=100', () {
      // Remote: 3840x2160, margin=100 → padded: 4040x2360
      // Local: 1920x1080
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 4040,
        displayHeight: 2360,
      );
      expect(vs.scale, closeTo(1080.0 / 2360, 0.0001));
    });

    test('adaptive — 1x remote, 2x local (HiDPI), margin=100', () {
      // Remote: 1920x1080, margin=100 → padded: 2120x1280
      // Local: 3840x2160
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 3840,
        height: 2160,
        displayWidth: 2120,
        displayHeight: 1280,
      );
      expect(vs.scale, closeTo(2160.0 / 1280, 0.0001));
    });

    test('custom style returns 1.0 (actual scale applied externally)', () {
      final vs = ViewStyle(
        style: kRemoteViewStyleCustom,
        width: 1920,
        height: 1080,
        displayWidth: 1920,
        displayHeight: 1080,
      );
      expect(vs.scale, 1.0);
    });
  });

  // ===========================================================================
  // ViewStyle equality — margin changes display dimensions
  // ===========================================================================
  group('ViewStyle equality', () {
    test('equal when all fields match', () {
      final a = ViewStyle(
          style: kRemoteViewStyleAdaptive,
          width: 1920,
          height: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      final b = ViewStyle(
          style: kRemoteViewStyleAdaptive,
          width: 1920,
          height: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      expect(a, equals(b));
    });

    test('not equal when margin changes display dimensions', () {
      final noMargin = ViewStyle(
          style: kRemoteViewStyleAdaptive,
          width: 1920,
          height: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      final withMargin = ViewStyle(
          style: kRemoteViewStyleAdaptive,
          width: 1920,
          height: 1080,
          displayWidth: 2120,
          displayHeight: 1280);
      expect(noMargin, isNot(equals(withMargin)));
    });
  });

  // ===========================================================================
  // Canvas offset centering (_resetCanvasOffset)
  // ===========================================================================
  group('Canvas offset centering', () {
    test('same size, no margin — offset is 0', () {
      final (x, y) = computeCanvasOffset(Size(1920, 1080), 1920, 1080, 1.0);
      expect(x, 0);
      expect(y, 0);
    });

    test('display smaller than view — positive offset centers it', () {
      final (x, y) = computeCanvasOffset(Size(1920, 1080), 1280, 720, 1.0);
      expect(x, 320); // (1920 - 1280) / 2
      expect(y, 180); // (1080 - 720) / 2
    });

    test('with margin — padded display affects centering', () {
      // View: 1920x1080, padded display: 2120x1280, adaptive scale
      final scale = 1080.0 / 1280; // 0.84375
      final (x, y) = computeCanvasOffset(Size(1920, 1080), 2120, 1280, scale);
      expect(x, closeTo((1920 - 2120 * scale) / 2, 0.01));
      expect(y, closeTo((1080 - 1280 * scale) / 2, 0.01));
    });

    test('2x remote display with margin — scale < 1', () {
      // View: 1920x1080, padded: 4040x2360
      final scale = 1080.0 / 2360;
      final (x, y) = computeCanvasOffset(Size(1920, 1080), 4040, 2360, scale);
      expect(x, closeTo((1920 - 4040 * scale) / 2, 0.01));
      expect(y, closeTo((1080 - 2360 * scale) / 2, 0.01));
    });

    test('small remote, large local (HiDPI) with margin — scale > 1', () {
      // View: 3840x2160, padded: 2120x1280
      final scale = 2160.0 / 1280;
      final (x, y) = computeCanvasOffset(Size(3840, 2160), 2120, 1280, scale);
      expect(x, closeTo((3840 - 2120 * scale) / 2, 0.01));
      expect(y, closeTo((2160 - 1280 * scale) / 2, 0.01));
    });
  });

  // ===========================================================================
  // CanvasCoords serialization
  // ===========================================================================
  group('CanvasCoords serialization', () {
    test('toJson includes padding and display fields', () {
      final coords = CanvasCoords();
      coords.paddingX = 100;
      coords.paddingY = 100;
      coords.displayWidth = 2120;
      coords.displayHeight = 1280;

      final json = coords.toJson();
      expect(json['paddingX'], 100);
      expect(json['paddingY'], 100);
      expect(json['displayWidth'], 2120);
      expect(json['displayHeight'], 1280);
    });

    test('fromJson roundtrip preserves all fields', () {
      final original = CanvasCoords();
      original.x = 10;
      original.y = 20;
      original.scale = 0.75;
      original.scrollX = 0.1;
      original.scrollY = 0.2;
      original.displayWidth = 2120;
      original.displayHeight = 1280;
      original.paddingX = 100;
      original.paddingY = 100;
      original.scrollStyle = ScrollStyle.scrollbar;
      original.size = Size(1920, 1080);

      final json = original.toJson();
      final restored = CanvasCoords.fromJson(json);

      expect(restored.x, original.x);
      expect(restored.y, original.y);
      expect(restored.scale, original.scale);
      expect(restored.scrollX, original.scrollX);
      expect(restored.scrollY, original.scrollY);
      expect(restored.displayWidth, original.displayWidth);
      expect(restored.displayHeight, original.displayHeight);
      expect(restored.paddingX, original.paddingX);
      expect(restored.paddingY, original.paddingY);
      expect(restored.scrollStyle, original.scrollStyle);
      expect(restored.size, original.size);
    });

    test('fromJson defaults padding/display to 0 when fields missing', () {
      final json = {
        'x': 0.0,
        'y': 0.0,
        'scale': 1.0,
        'scrollX': 0.0,
        'scrollY': 0.0,
        'scrollStyle': 'scrollauto',
        'size': {'w': 1920.0, 'h': 1080.0},
      };
      final coords = CanvasCoords.fromJson(json);
      expect(coords.displayWidth, 0);
      expect(coords.displayHeight, 0);
      expect(coords.paddingX, 0);
      expect(coords.paddingY, 0);
    });
  });

  // ===========================================================================
  // Pointer coordinate transforms with margin
  // ===========================================================================
  group('Pointer coordinate transforms', () {
    test('no margin — pointer maps directly (scrollauto)', () {
      final canvas = CanvasCoords();
      canvas.x = 0;
      canvas.y = 0;
      canvas.scale = 1.0;
      canvas.displayWidth = 1920;
      canvas.displayHeight = 1080;
      canvas.paddingX = 0;
      canvas.paddingY = 0;
      canvas.scrollStyle = ScrollStyle.scrollauto;
      canvas.size = Size(1920, 1080);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final result = computePointerPosition(
          pointerX: 960, pointerY: 540, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(960, 0.01));
      expect(result.$2, closeTo(540, 0.01));
    });

    test('with margin — pointer offset by padding (scrollauto)', () {
      final canvas = CanvasCoords();
      canvas.displayWidth = 2120; // 1920 + 2*100
      canvas.displayHeight = 1280; // 1080 + 2*100
      canvas.paddingX = 100;
      canvas.paddingY = 100;
      canvas.scale = 1.0;
      canvas.x = 0;
      canvas.y = 0;
      canvas.scrollStyle = ScrollStyle.scrollauto;
      canvas.size = Size(2120, 1280);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      // Pointer at (100, 100) → subtract padding → remote (0, 0)
      final result = computePointerPosition(
          pointerX: 100, pointerY: 100, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(0, 0.01));
      expect(result.$2, closeTo(0, 0.01));
    });

    test('with margin — center of padded view maps to center of remote', () {
      final canvas = CanvasCoords();
      canvas.displayWidth = 2120;
      canvas.displayHeight = 1280;
      canvas.paddingX = 100;
      canvas.paddingY = 100;
      canvas.scale = 1.0;
      canvas.x = 0;
      canvas.y = 0;
      canvas.scrollStyle = ScrollStyle.scrollauto;
      canvas.size = Size(2120, 1280);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      // Center of padded display: (1060, 640) → minus padding → (960, 540)
      final result = computePointerPosition(
          pointerX: 1060,
          pointerY: 640,
          canvas: canvas,
          remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(960, 0.01));
      expect(result.$2, closeTo(540, 0.01));
    });

    test('with margin and adaptive scale — 2x remote display', () {
      // Remote: 3840x2160, margin: 100, padded: 4040x2360
      // Local view: 1920x1080, adaptive scale: 1080/2360
      final scale = 1080.0 / 2360;
      final displayWidth = 4040.0;
      final displayHeight = 2360.0;

      final canvas = CanvasCoords();
      canvas.displayWidth = displayWidth;
      canvas.displayHeight = displayHeight;
      canvas.paddingX = 100;
      canvas.paddingY = 100;
      canvas.scale = scale;
      canvas.x = (1920 - displayWidth * scale) / 2;
      canvas.y = (1080 - displayHeight * scale) / 2;
      canvas.scrollStyle = ScrollStyle.scrollauto;
      canvas.size = Size(1920, 1080);

      final remoteRect = Rect.fromLTWH(0, 0, 3840, 2160);

      // Pointer at center of view → should map to center of remote
      final result = computePointerPosition(
          pointerX: 960, pointerY: 540, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(1920, 1));
      expect(result.$2, closeTo(1080, 1));
    });

    test('with margin — scrollbar style, no scroll offset', () {
      final canvas = CanvasCoords();
      canvas.displayWidth = 2020; // 1920 + 2*50
      canvas.displayHeight = 1180; // 1080 + 2*50
      canvas.paddingX = 50;
      canvas.paddingY = 50;
      canvas.scale = 1.0;
      canvas.scrollX = 0;
      canvas.scrollY = 0;
      canvas.scrollStyle = ScrollStyle.scrollbar;
      canvas.size = Size(1920, 1080);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      // Image (2020x1180) > view (1920x1080), no centering.
      // Pointer at (50, 50), scrollX=0 → x=50/1.0=50, paddedX=50
      // x = 50 - 50(padding) + 0(rect.left) = 0
      final result = computePointerPosition(
          pointerX: 50, pointerY: 50, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(0, 0.01));
      expect(result.$2, closeTo(0, 0.01));
    });

    test('with margin — scrollbar style, 50% scroll offset', () {
      final canvas = CanvasCoords();
      canvas.displayWidth = 2020;
      canvas.displayHeight = 1180;
      canvas.paddingX = 50;
      canvas.paddingY = 50;
      canvas.scale = 1.0;
      canvas.scrollX = 0.5;
      canvas.scrollY = 0.5;
      canvas.scrollStyle = ScrollStyle.scrollbar;
      canvas.size = Size(1920, 1080);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      // With 50% scroll: x = 0 + 2020*0.5 = 1010
      // Image > view → no centering subtraction
      // /scale(1.0) → 1010, minus padding(50) = 960
      final result = computePointerPosition(
          pointerX: 0, pointerY: 0, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(960, 0.01));
      expect(result.$2, closeTo(540, 0.01));
    });

    test('pointer in margin area clamps to remote rect boundary', () {
      final canvas = CanvasCoords();
      canvas.displayWidth = 2120;
      canvas.displayHeight = 1280;
      canvas.paddingX = 100;
      canvas.paddingY = 100;
      canvas.scale = 1.0;
      canvas.x = 0;
      canvas.y = 0;
      canvas.scrollStyle = ScrollStyle.scrollauto;
      canvas.size = Size(2120, 1280);

      final remoteRect = Rect.fromLTWH(0, 0, 1920, 1080);

      // Pointer at (50, 50): inside padded rect, but maps to (-50, -50)
      // in remote coords → clamped to (0, 0)
      final result = computePointerPosition(
          pointerX: 50, pointerY: 50, canvas: canvas, remoteRect: remoteRect);

      expect(result, isNotNull);
      expect(result!.$1, closeTo(0, 0.01));
      expect(result.$2, closeTo(0, 0.01));
    });
  });

  // ===========================================================================
  // End-to-end: different scale ratios with margin
  // ===========================================================================
  group('Scale ratio scenarios with margin', () {
    test('1:1 ratio, no margin', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(realRect, 0)!;
      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: padded.width.toInt(),
        displayHeight: padded.height.toInt(),
      );
      expect(vs.scale, 1.0);
      expect(computeDisplayWidth(padded), 1920);
      expect(computeDisplayHeight(padded), 1080);
    });

    test('2x remote, 1x local, margin=100', () {
      final realRect = Rect.fromLTWH(0, 0, 3840, 2160);
      final padded = computePaddedRect(realRect, 100)!;
      expect(padded.width, 4040);
      expect(padded.height, 2360);

      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 4040,
        displayHeight: 2360,
      );
      expect(vs.scale, closeTo(1080.0 / 2360, 0.0001));
    });

    test('1x remote, 2x local (HiDPI), margin=200', () {
      final realRect = Rect.fromLTWH(0, 0, 1920, 1080);
      final padded = computePaddedRect(realRect, 200)!;
      expect(padded.width, 2320);
      expect(padded.height, 1480);

      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 3840,
        height: 2160,
        displayWidth: 2320,
        displayHeight: 1480,
      );
      expect(vs.scale, closeTo(2160.0 / 1480, 0.0001));
    });

    test('1.5x remote, 1x local, margin=50, original view style', () {
      final realRect = Rect.fromLTWH(0, 0, 2880, 1620);
      final padded = computePaddedRect(realRect, 50)!;

      final vs = ViewStyle(
        style: kRemoteViewStyleOriginal,
        width: 1920,
        height: 1080,
        displayWidth: padded.width.toInt(),
        displayHeight: padded.height.toInt(),
      );
      // Original always 1.0
      expect(vs.scale, 1.0);
    });

    test('portrait remote, landscape local, margin=100', () {
      final realRect = Rect.fromLTWH(0, 0, 1080, 1920);
      final padded = computePaddedRect(realRect, 100)!;
      expect(padded.width, 1280);
      expect(padded.height, 2120);

      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 1280,
        displayHeight: 2120,
      );
      // s1 = 1920/1280 = 1.5, s2 = 1080/2120 ≈ 0.5094 → 0.5094
      expect(vs.scale, closeTo(1080.0 / 2120, 0.0001));
    });

    test('multi-monitor: wide remote, standard local, margin=100', () {
      // Dual-monitor remote: 3840x1080, margin=100 → padded: 4040x1280
      // Local: 1920x1080
      final realRect = Rect.fromLTWH(0, 0, 3840, 1080);
      final padded = computePaddedRect(realRect, 100)!;
      expect(padded.width, 4040);
      expect(padded.height, 1280);

      final vs = ViewStyle(
        style: kRemoteViewStyleAdaptive,
        width: 1920,
        height: 1080,
        displayWidth: 4040,
        displayHeight: 1280,
      );
      // s1 = 1920/4040 ≈ 0.4752, s2 = 1080/1280 = 0.84375 → 0.4752
      expect(vs.scale, closeTo(1920.0 / 4040, 0.0001));
    });
  });
}

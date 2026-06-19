import 'package:flutter_test/flutter_test.dart';

// Pure-logic test for the canvas-overflow predicate that drives whether
// a two-finger gesture should pan the local canvas (when remote content
// overflows the viewport) or be treated as a no-op / pass-through (when
// it doesn't). Refs: rustdesk/rustdesk#15209.
//
// The shim here mirrors the production predicate in
// `CanvasModel.isPanScrollUseful()` (model.dart). Since CanvasModel pulls
// in the FFI runtime, the shim lets us iterate on the math in isolation.
class _OverflowPredicate {
  double scale;
  double sizeWidth;
  double sizeHeight;
  int displayWidth;
  int displayHeight;
  _OverflowPredicate({
    required this.scale,
    required this.sizeWidth,
    required this.sizeHeight,
    required this.displayWidth,
    required this.displayHeight,
  });

  bool isPanScrollUseful() {
    if (sizeWidth == 0 || sizeHeight == 0) return false;
    final dw = displayWidth * scale;
    final dh = displayHeight * scale;
    return dw > sizeWidth || dh > sizeHeight;
  }
}

void main() {
  group('CanvasModel.isPanScrollUseful (predicate semantics)', () {
    test('false when remote display fits the viewport exactly', () {
      final c = _OverflowPredicate(
          scale: 1.0,
          sizeWidth: 1920,
          sizeHeight: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      expect(c.isPanScrollUseful(), isFalse);
    });

    test('false when remote display fits well inside the viewport', () {
      final c = _OverflowPredicate(
          scale: 0.5,
          sizeWidth: 1920,
          sizeHeight: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      expect(c.isPanScrollUseful(), isFalse);
    });

    test('true when zoomed-in display overflows on x only', () {
      final c = _OverflowPredicate(
          scale: 1.5,
          sizeWidth: 1920,
          sizeHeight: 1080,
          displayWidth: 1920,
          displayHeight: 600);
      expect(c.isPanScrollUseful(), isTrue);
    });

    test('true when zoomed-in display overflows on y only', () {
      final c = _OverflowPredicate(
          scale: 1.5,
          sizeWidth: 1920,
          sizeHeight: 1080,
          displayWidth: 800,
          displayHeight: 1080);
      expect(c.isPanScrollUseful(), isTrue);
    });

    test('true when zoomed-in display overflows on both axes', () {
      final c = _OverflowPredicate(
          scale: 2.0,
          sizeWidth: 1920,
          sizeHeight: 1080,
          displayWidth: 1920,
          displayHeight: 1080);
      expect(c.isPanScrollUseful(), isTrue);
    });

    test('false when viewport size is zero (model not yet laid out)', () {
      final c = _OverflowPredicate(
          scale: 1.0,
          sizeWidth: 0,
          sizeHeight: 0,
          displayWidth: 1920,
          displayHeight: 1080);
      expect(c.isPanScrollUseful(), isFalse);
    });
  });
}

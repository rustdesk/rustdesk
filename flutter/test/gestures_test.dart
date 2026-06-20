import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';

// Tests that CustomTouchGestureRecognizer refuses to claim trackpad
// pan-zoom events on mobile platforms (iOS/Android), but keeps the
// default behaviour on desktop. Refs: rustdesk/rustdesk#15209.
//
// Background: Flutter's base ScaleGestureRecognizer.isPointerPanZoomAllowed
// returns true unconditionally, regardless of supportedDevices, so on
// iPadOS the Magic Keyboard trackpad's PointerPanZoom* events get claimed
// as scale/pan and routed into local canvas pan via onTwoFingerScaleUpdate.
// We override to return false on mobile so the raw
// Listener.onPointerPanZoomUpdate path (input_model.dart) — which already
// sends the scroll to the remote — is the sole consumer.

PointerPanZoomStartEvent _panZoomStart() => const PointerPanZoomStartEvent(
      pointer: 1,
      position: Offset(100, 100),
    );

void main() {
  group('CustomTouchGestureRecognizer.isPointerPanZoomAllowed', () {
    test('returns false on iOS', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);

      final recognizer = CustomTouchGestureRecognizer();
      addTearDown(recognizer.dispose);

      expect(recognizer.isPointerPanZoomAllowed(_panZoomStart()), isFalse);
    });

    test('returns false on Android', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);

      final recognizer = CustomTouchGestureRecognizer();
      addTearDown(recognizer.dispose);

      expect(recognizer.isPointerPanZoomAllowed(_panZoomStart()), isFalse);
    });

    test('returns true on macOS (desktop) — preserves existing trackpad behaviour', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);

      final recognizer = CustomTouchGestureRecognizer();
      addTearDown(recognizer.dispose);

      expect(recognizer.isPointerPanZoomAllowed(_panZoomStart()), isTrue);
    });

    test('returns true on Linux (desktop)', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);

      final recognizer = CustomTouchGestureRecognizer();
      addTearDown(recognizer.dispose);

      expect(recognizer.isPointerPanZoomAllowed(_panZoomStart()), isTrue);
    });

    test('returns true on Windows (desktop)', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);

      final recognizer = CustomTouchGestureRecognizer();
      addTearDown(recognizer.dispose);

      expect(recognizer.isPointerPanZoomAllowed(_panZoomStart()), isTrue);
    });
  });
}

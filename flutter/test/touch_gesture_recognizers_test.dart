import 'package:flutter/gestures.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Touch gesture recognizers isPointerAllowed', () {
    test('TouchTapGestureRecognizer rejects mouse and allows direct touch', () {
      final recognizer = TouchTapGestureRecognizer(
        isPhysicalPointer: (event) => false,
      )..onTap = () {};

      // Mouse is always rejected
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.mouse,
            buttons: kPrimaryButton,
            position: Offset(50, 50),
          ),
        ),
        isFalse,
      );

      // Direct touchscreen touch is allowed, even near cursor/hover position
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.touch,
            buttons: kPrimaryButton,
            position: Offset(10, 10),
          ),
        ),
        isTrue,
      );
    });

    test('CustomTouchGestureRecognizer rejects mouse and allows direct touch', () {
      final recognizer = CustomTouchGestureRecognizer(
        isPhysicalPointer: (event) => false,
      );

      // Mouse is rejected
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.mouse,
            buttons: kPrimaryButton,
            position: Offset(50, 50),
          ),
        ),
        isFalse,
      );

      // Direct touchscreen touch for virtual trackpad / pan is allowed
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.touch,
            buttons: kPrimaryButton,
            position: Offset(20, 20),
          ),
        ),
        isTrue,
      );
    });

    test('Touchscreen touches are always allowed and never impersonated as trackpad', () {
      final tapRecognizer = TouchTapGestureRecognizer(
        isPhysicalPointer: (event) => false,
      )..onTap = () {};

      final gestureRecognizer = CustomTouchGestureRecognizer(
        isPhysicalPointer: (event) => false,
      );

      for (final recognizer in [tapRecognizer, gestureRecognizer]) {
        expect(
          recognizer.isPointerAllowed(
            const PointerDownEvent(
              kind: PointerDeviceKind.touch,
              buttons: kPrimaryButton,
              position: Offset(150, 250),
            ),
          ),
          isTrue,
        );
      }
    });
  });
}

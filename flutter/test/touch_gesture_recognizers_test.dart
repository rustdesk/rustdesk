import 'package:flutter/gestures.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Touch gesture recognizers isPointerAllowed', () {
    test('TouchTapGestureRecognizer rejects mouse and physical touchpad clicks',
        () {
      final recognizer = TouchTapGestureRecognizer(
        isPhysicalPointer: (event) => event.position == const Offset(10, 10),
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

      // Touchpad click at hover position is rejected
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.touch,
            buttons: kPrimaryButton,
            position: Offset(10, 10),
          ),
        ),
        isFalse,
      );

      // Direct touchscreen touch is allowed
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.touch,
            buttons: kPrimaryButton,
            position: Offset(100, 200),
          ),
        ),
        isTrue,
      );
    });

    test('CustomTouchGestureRecognizer rejects mouse and allows direct touch',
        () {
      final recognizer = CustomTouchGestureRecognizer(
        isPhysicalPointer: (event) => event.position == const Offset(20, 20),
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

      // Physical touchpad click at hover position is rejected
      expect(
        recognizer.isPointerAllowed(
          const PointerDownEvent(
            kind: PointerDeviceKind.touch,
            buttons: kPrimaryButton,
            position: Offset(20, 20),
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
            position: Offset(300, 400),
          ),
        ),
        isTrue,
      );
    });
  });
}

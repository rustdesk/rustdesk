import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';

void main() {
  testWidgets(
      'two-finger end reset is not cancelled by moves and fires exactly once '
      'after 200ms', (tester) async {
    var oneFingerStart = 0;
    var twoFingerStart = 0;
    var twoFingerEnd = 0;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  CustomTouchGestureRecognizer>(
              CustomTouchGestureRecognizer.new,
              (r) => r
                ..onOneFingerPanStart = (d) {
                  oneFingerStart++;
                }
                ..onTwoFingerScaleStart = (d) {
                  twoFingerStart++;
                }
                ..onTwoFingerScaleEnd = (d) {
                  twoFingerEnd++;
                }),
        },
        child: const SizedBox.expand(),
      ),
    );

    // Back-to-back pointer-up reproduces the single onEnd the reset relies on.
    final finger1 =
        await tester.startGesture(const Offset(100, 100), pointer: 1);
    final finger2 =
        await tester.startGesture(const Offset(200, 100), pointer: 2);
    await tester.pump();
    await finger1.moveTo(const Offset(120, 120));
    await finger2.moveTo(const Offset(220, 120));
    await tester.pump();
    expect(twoFingerStart, 1);

    await finger1.up();
    await finger2.up();
    await tester.pump();
    expect(twoFingerEnd, 1);

    await tester.pump(const Duration(milliseconds: 50));
    final finger =
        await tester.startGesture(const Offset(150, 150), pointer: 3);
    await finger.moveTo(const Offset(210, 150));
    await tester.pump();
    expect(oneFingerStart, 0);

    for (var i = 1; i <= 5; i++) {
      await tester.pump(const Duration(milliseconds: 40));
      await finger.moveTo(Offset((210 + i).toDouble(), 150));
      await tester.pump();
    }
    expect(oneFingerStart, 1);

    await finger.up();
    await tester.pump(const Duration(milliseconds: 200));
  });
}

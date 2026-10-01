import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('lifting one finger of a pinch pans with the other once',
      (tester) async {
    var oneFingerStarts = 0;
    await tester.pumpWidget(RawGestureDetector(
      behavior: HitTestBehavior.opaque,
      gestures: <Type, GestureRecognizerFactory>{
        CustomTouchGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<CustomTouchGestureRecognizer>(
                CustomTouchGestureRecognizer.new,
                (recognizer) =>
                    recognizer..onOneFingerPanStart = (_) => oneFingerStarts++),
      },
      child: const SizedBox.expand(),
    ));

    final first = await tester.startGesture(const Offset(100, 100), pointer: 1);
    final second =
        await tester.startGesture(const Offset(200, 100), pointer: 2);
    await second.moveBy(const Offset(35, 0));
    await tester.pump();
    await second.up();
    for (var i = 0; i < 8; i++) {
      await first.moveBy(const Offset(10, 0));
      await tester.pump(const Duration(milliseconds: 50));
    }
    await first.up();
    await tester.pump(const Duration(milliseconds: 300));

    expect(oneFingerStarts, 1);
  });
}

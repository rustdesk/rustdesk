import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets(
      '2-finger touch then lift 1 finger and move remaining finger triggers scroll',
      (tester) async {
    final scrollDeltas = <Offset>[];
    var oneFingerPans = 0;
    var scrollEnded = false;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
              CustomTouchGestureRecognizer>(
            () => CustomTouchGestureRecognizer(),
            (instance) {
              instance
                ..onOneFingerPanUpdate = (_) {
                  oneFingerPans++;
                }
                ..onTwoFingerActivatedScrollUpdate = (d) {
                  scrollDeltas.add(d.delta);
                }
                ..onTwoFingerActivatedScrollEnd = (_) {
                  scrollEnded = true;
                };
            },
          ),
        },
        child: const SizedBox.expand(),
      ),
    );

    final g1 = await tester.startGesture(const Offset(100, 100));
    final g2 = await tester.startGesture(const Offset(140, 100));
    await tester.pump(const Duration(milliseconds: 20));

    await g2.up();
    await tester.pump(const Duration(milliseconds: 20));

    await g1.moveBy(const Offset(0, 10));
    await g1.moveBy(const Offset(0, 20));
    await tester.pump();

    expect(scrollDeltas, isNotEmpty);
    expect(oneFingerPans, 0);

    await g1.up();
    await tester.pump(const Duration(milliseconds: 250));
    expect(scrollEnded, isTrue);
  });

  testWidgets('3-finger tap triggers onThreeFingerTap', (tester) async {
    var threeFingerTapped = false;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
              CustomTouchGestureRecognizer>(
            () => CustomTouchGestureRecognizer(),
            (instance) {
              instance.onThreeFingerTap = () {
                threeFingerTapped = true;
              };
            },
          ),
        },
        child: const SizedBox.expand(),
      ),
    );

    final g1 = await tester.startGesture(const Offset(100, 100));
    final g2 = await tester.startGesture(const Offset(130, 100));
    final g3 = await tester.startGesture(const Offset(160, 100));
    await tester.pump(const Duration(milliseconds: 30));

    await g3.up();
    await g2.up();
    await g1.up();
    await tester.pump(const Duration(milliseconds: 250));

    expect(threeFingerTapped, isTrue);
  });

  testWidgets(
      '3-finger touch then lift 2 fingers and slide remaining finger triggers app switch',
      (tester) async {
    final appSwitchDeltas = <Offset>[];
    var appSwitchEnded = false;
    var threeFingerTapped = false;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
              CustomTouchGestureRecognizer>(
            () => CustomTouchGestureRecognizer(),
            (instance) {
              instance
                ..onThreeFingerTap = () {
                  threeFingerTapped = true;
                }
                ..onThreeFingerAppSwitchUpdate = (d) {
                  appSwitchDeltas.add(d.delta);
                }
                ..onThreeFingerAppSwitchEnd = (_) {
                  appSwitchEnded = true;
                };
            },
          ),
        },
        child: const SizedBox.expand(),
      ),
    );

    final g1 = await tester.startGesture(const Offset(100, 100));
    final g2 = await tester.startGesture(const Offset(130, 100));
    final g3 = await tester.startGesture(const Offset(160, 100));
    await tester.pump(const Duration(milliseconds: 20));

    await g3.up();
    await g2.up();
    await tester.pump(const Duration(milliseconds: 20));

    await g1.moveBy(const Offset(15, 0));
    await g1.moveBy(const Offset(40, 0));
    await tester.pump();

    expect(appSwitchDeltas, isNotEmpty);
    expect(threeFingerTapped, isFalse);

    await g1.up();
    await tester.pump(const Duration(milliseconds: 250));
    expect(appSwitchEnded, isTrue);
  });
}

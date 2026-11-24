import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final action in ['click', 'zoom', 'drag']) {
    testWidgets('second tap routes to $action', (tester) async {
      final events = <String>[];
      await tester.pumpWidget(RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: {
          DoubleTapGestureRecognizer:
              GestureRecognizerFactoryWithHandlers<DoubleTapGestureRecognizer>(
                  () => DoubleTapGestureRecognizer(),
                  (r) => r.onDoubleTap = () => events.add('click')),
          HoldTapMoveGestureRecognizer:
              GestureRecognizerFactoryWithHandlers<HoldTapMoveGestureRecognizer>(
                  () => HoldTapMoveGestureRecognizer(), (r) {
            r.onStationarySecondTap = (_) {
              events.add('zoom');
              return true;
            };
            r.onHoldDragStart = (_) => events.add('drag');
            r.onHoldDragEnd = (_) {};
          }),
        },
        child: const SizedBox.expand(),
      ));
      const point = Offset(80, 80);
      await tester.tapAt(point);
      await tester.pump(const Duration(milliseconds: 90));
      final second = await tester.startGesture(point);
      if (action == 'zoom') await tester.pump(const Duration(milliseconds: 330));
      if (action == 'drag') await second.moveBy(const Offset(40, 0));
      await second.up();
      await tester.pump(const Duration(milliseconds: 400));
      expect(events, [action]);
    });
  }
}

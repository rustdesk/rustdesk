import 'package:flutter_hbb/mobile/models/double_tap_zoom.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('zoom amount defaults to 125 percent and supports up to 250 percent', () {
    expect(zoomStrengthFromOption(''), 1.25);
    expect(zoomStrengthFromOption('150'), 1.5);
    expect(zoomStrengthFromOption('600'), 2.5);
  });
  testWidgets('zoom advances smoothly, finishes and can be interrupted',
      (tester) async {
    final frames = <double>[];
    final animation = animateDoubleTapZoom(tester, frames.add);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 115));
    expect(frames.last, inExclusiveRange(0, 1));
    await tester.pump(const Duration(milliseconds: 115));
    expect(frames.last, 1);
    animation.dispose();

    final interrupted = animateDoubleTapZoom(tester, frames.add);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 60));
    interrupted.stop();
    final last = frames.last;
    await tester.pump(const Duration(milliseconds: 250));
    expect(frames.last, last);
    interrupted.dispose();
  });

  test(
      '50 percent reaches native resolution in two gestures on different displays',
      () {
    for (final fit in [0.12, 0.2, 0.6]) {
      const native = 1 / 3;
      double next(double current) => nextDoubleTapZoomScale(current, fit,
          nativeScale: native, amount: 0.5);
      final first = next(fit);
      expect(first, closeTo((fit + native) / 2, 1e-8));
      expect(next(first), closeTo(native, 1e-8));
      expect(next(native), fit);
    }
  });

  test('100 percent is pixel for pixel in one gesture and 250 percent is 2.5:1',
      () {
    const fit = 0.15;
    for (final dpr in [1.0, 2.5, 3.0]) {
      for (final amount in [1.0, 1.25, 2.5]) {
        final target = nextDoubleTapZoomScale(fit, fit,
            nativeScale: 1 / dpr, amount: amount);
        expect(target * dpr, closeTo(amount, 1e-8));
        expect(
            nextDoubleTapZoomScale(target, fit,
                nativeScale: 1 / dpr, amount: amount),
            fit);
      }
    }
  });
}

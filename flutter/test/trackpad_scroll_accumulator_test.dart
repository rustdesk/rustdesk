import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/trackpad_scroll_accumulator.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const legacyUnitsPerPoint = 0.06;
  const highResolutionUnitsPerPoint =
      legacyUnitsPerPoint * kHighResolutionScrollUnitsPerStep;

  test('high-resolution units preserve a sub-detent delta', () {
    final legacy = TrackpadScrollAccumulator();
    final highResolution = TrackpadScrollAccumulator();

    expect(legacy.take(const Offset(1, 0), legacyUnitsPerPoint), Offset.zero);
    expect(highResolution.take(const Offset(1, 0), highResolutionUnitsPerPoint),
        const Offset(7, 0));
  });

  test('fractional units are conserved independently on both axes', () {
    final accumulator = TrackpadScrollAccumulator();
    var emitted = Offset.zero;

    for (var i = 0; i < 5; i++) {
      emitted +=
          accumulator.take(const Offset(1, -1), highResolutionUnitsPerPoint);
    }

    expect(emitted, const Offset(36, -36));
  });

  test('changing resolution mode converts fractional units', () {
    final accumulator = TrackpadScrollAccumulator();

    expect(accumulator.take(const Offset(10, 0), legacyUnitsPerPoint),
        Offset.zero);
    expect(accumulator.take(const Offset(0.1, 0), highResolutionUnitsPerPoint),
        const Offset(72, 0));
  });

  testSmoothFlingOwnership();
  testWheelFlingPolicy();
}

void testSmoothFlingOwnership() {
  test('smooth gestures leave inertia to the receiving application', () {
    expect(
        shouldStartTrackpadFling(
          scrollMode: LinuxTrackpadScrollMode.smooth,
          isViewOnly: false,
          delta: const Offset(3, -2),
          minimumDelta: 2,
        ),
        isFalse);
  });
}

void testWheelFlingPolicy() {
  for (final mode in [
    LinuxTrackpadScrollMode.legacy,
    LinuxTrackpadScrollMode.highResolutionWheel,
  ]) {
    test('${mode.name} gestures retain client inertia', () {
      expect(
          shouldStartTrackpadFling(
            scrollMode: mode,
            isViewOnly: false,
            delta: const Offset(3, -2),
            minimumDelta: 2,
          ),
          isTrue);
    });
    test('${mode.name} gestures at the fling threshold do not start inertia',
        () {
      expect(
          shouldStartTrackpadFling(
            scrollMode: mode,
            isViewOnly: false,
            delta: const Offset(2, -2),
            minimumDelta: 2,
          ),
          isFalse);
    });
    test('${mode.name} gestures in view-only mode do not start inertia', () {
      expect(
          shouldStartTrackpadFling(
            scrollMode: mode,
            isViewOnly: true,
            delta: const Offset(3, -2),
            minimumDelta: 2,
          ),
          isFalse);
    });
  }
}

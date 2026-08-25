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

  test('smooth gesture end uses client fling until delta is below one point',
      () {
    final accumulator = TrackpadScrollAccumulator();
    final smoothUnitsPerPoint = kSmoothScrollUnitsPerPoint.toDouble();

    expect(
        shouldStartTrackpadFling(
          scrollMode: LinuxTrackpadScrollMode.smooth,
          isViewOnly: false,
          delta: const Offset(3, -2),
          minimumDelta: 2,
        ),
        isTrue);
    expect(accumulator.takeFling(const Offset(3, -2), smoothUnitsPerPoint),
        const Offset(360, -240));
    expect(accumulator.takeFling(const Offset(0.9, -0.9), smoothUnitsPerPoint),
        Offset.zero);
  });
}

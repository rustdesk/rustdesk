import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/models/trackpad_motion.dart';

void main() {
  test('saved strengths give independent off, default and maximum values', () {
    expect(trackpadMotionStrength('0'), 0);
    expect(trackpadMotionStrength(''), 0.5);
    expect(trackpadMotionStrength('100'), 1);
    expect(trackpadMotionStrength('125'), 1);
  });

  test('slow swipes retain precision and fast swipes get bounded gain', () {
    expect(trackpadMotionGain(170, 0.5), 1);
    expect(trackpadMotionGain(200, 0.5), closeTo(1, 0.005));
    expect(trackpadMotionGain(900, 0.5), greaterThan(2));
    expect(trackpadMotionGain(2000, 0.5), closeTo(2.15, 0.001));
    expect(trackpadMotionGain(2000, 1), closeTo(3.3, 0.001));
    expect(trackpadMotionGain(2000, 0), 1);
  });

  test('gain builds gradually across sampling rates and resets after a pause',
      () {
    const shortSample = Duration(milliseconds: 8);
    final first = trackpadFilteredSpeed(0, 12, shortSample);
    final second = trackpadFilteredSpeed(first, 12, shortSample);
    final combined = trackpadFilteredSpeed(0, 24, shortSample * 2);
    expect(trackpadMotionGain(first, 0.5), lessThan(1.55));
    expect(trackpadMotionGain(second, 0.5),
        greaterThan(trackpadMotionGain(first, 0.5)));
    expect(second, closeTo(combined, 0.001));
    expect(trackpadFilteredSpeed(second, 24, const Duration(milliseconds: 90)),
        0);
  });

  test('higher glide settings keep a fast release moving for longer', () {
    const fresh = Duration(milliseconds: 10);
    final normal = trackpadGlideDuration(trackpadMotionStrength(''));
    final longest = trackpadGlideDuration(1);
    expect(trackpadGlideDuration(0), Duration.zero);
    expect(normal, const Duration(milliseconds: 500));
    expect(longest, const Duration(milliseconds: 1000));
    expect(trackpadGlideDistance(1500, fresh, longest),
        closeTo(2 * trackpadGlideDistance(1500, fresh, normal), 0.001));
    expect(trackpadGlideProgress(Duration.zero, longest), 0);
    expect(trackpadGlideProgress(normal, longest), closeTo(0.75, 0.001));
    expect(trackpadGlideProgress(longest, longest), 1);
  });

  test('glide stays off for slow or stale releases and limits fast travel', () {
    const fresh = Duration(milliseconds: 10);
    final duration = trackpadGlideDuration(1);
    expect(trackpadGlideDistance(1500, fresh, Duration.zero), 0);
    expect(trackpadGlideDistance(500, fresh, duration), 0);
    expect(trackpadGlideDistance(1500, const Duration(milliseconds: 90), duration),
        0);
    expect(trackpadGlideDistance(5000, fresh, duration), closeTo(600, 0.001));
  });
}

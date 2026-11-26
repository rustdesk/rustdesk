import 'package:flutter_hbb/utils/relative_mouse_position_tracker.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('queued mouse movement does not send a jump when recentering', () {
    for (final direction in [1.0, -1.0]) {
      final tracker = RelativeMousePositionTracker();
      const center = Offset(500, 300);
      final edge = center + Offset(direction * 400, 0);
      tracker.update(edge);
      tracker.recenterTo(center);

      final step = Offset(direction, 0);
      final firstAfterRecenter = center + step * 3;
      expect(
        [
          tracker.update(edge + step),
          tracker.update(firstAfterRecenter),
          tracker.update(firstAfterRecenter + step * 4),
        ],
        [step, null, step * 4],
      );
    }
  });
}

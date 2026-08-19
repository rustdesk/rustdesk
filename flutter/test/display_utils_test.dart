import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/display_utils.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('nextDisplayIndex', () {
    test('advances to the next display', () {
      expect(nextDisplayIndex(0, 3), 1);
      expect(nextDisplayIndex(1, 3), 2);
    });

    test('wraps around after the last display', () {
      expect(nextDisplayIndex(2, 3), 0);
    });

    test('skips the "All displays" pseudo-monitor and starts at the first', () {
      expect(nextDisplayIndex(kAllDisplayValue, 3), 0);
    });

    test('cycles through all individual displays and back to the start', () {
      const total = 4;
      var current = 0;
      final order = <int>[];
      for (var i = 0; i < total; i++) {
        current = nextDisplayIndex(current, total);
        order.add(current);
      }
      expect(order, [1, 2, 3, 0]);
    });
  });
}

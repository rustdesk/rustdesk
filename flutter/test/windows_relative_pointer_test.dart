import 'dart:ui';

import 'package:flutter_hbb/utils/windows_relative_pointer.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('queued notifications after recenter contribute only physical movement',
      () {
    Offset? os = const Offset(1800, 500);
    final pointer = WindowsRelativePointer(() => os);
    expect(pointer.movement(), isNull);
    os = const Offset(1810, 502);
    expect(pointer.movement(), const Offset(10, 2));

    os = const Offset(960, 540);
    pointer.didWarp();
    // A real movement arrived before Flutter delivered the warp notification.
    os = const Offset(968, 539);
    expect(pointer.movement(), const Offset(8, -1));
    expect(pointer.movement(), Offset.zero);
  });

  test('an unavailable cursor or loss of focus starts a new baseline', () {
    Offset? os = const Offset(1800, 500);
    final pointer = WindowsRelativePointer(() => os)..didWarp();
    os = null;
    expect(pointer.movement(), isNull);
    os = const Offset(-1800, 10);
    expect(pointer.movement(), isNull);
    pointer.reset();
    os = const Offset(0, 0);
    expect(pointer.movement(), isNull);
  });
}

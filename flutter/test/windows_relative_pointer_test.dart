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
  test('a queued edge notification recenters without another delta or flush',
      () {
    const inside = Offset(95, 50),
        edge = Offset(100, 50),
        center = Offset(50, 50);
    Offset? os;
    final pointer = WindowsRelativePointer(() => os);
    final sent = <Offset>[];
    var warps = 0;
    void checkEdge(Offset local) {
      if (local != edge) return;
      warps++;
      os = center;
      pointer.didWarp();
    }

    // Seed the baseline, then deliver an older interior notification after
    // the OS cursor already reached the edge.
    os = inside;
    pointer.handleMove(inside, sent.add, checkEdge);
    os = edge;
    pointer.handleMove(inside, sent.add, checkEdge);
    expect(sent, [const Offset(5, 0)]);
    expect(warps, 0);

    // The earlier delta can already have been flushed. This notification
    // has zero sampled movement and must trigger the warp on its own.
    pointer.handleMove(edge, sent.add, checkEdge);
    expect(warps, 1);
    pointer.handleMove(center, sent.add, checkEdge);
    expect(sent, [const Offset(5, 0)]);

    os = null;
    pointer.handleMove(edge, sent.add, checkEdge);
    expect(warps, 2);
    expect(sent, [const Offset(5, 0)]);
  });
}

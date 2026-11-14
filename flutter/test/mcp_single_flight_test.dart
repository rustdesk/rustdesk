import 'dart:async';

import 'package:flutter_hbb/mcp/mcp_utils.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('overlapping calls share one operation per key', () async {
    final flights = McpSingleFlight<int>();
    final barrier = Completer<void>();
    var opened = 0;

    Future<int> open() {
      opened++;
      return barrier.future.then((_) => opened);
    }

    // Both calls resume from their own awaits before either registers, which
    // is what two overlapping HTTP requests do.
    final first = flights.start('peer', open);
    final second = flights.start('peer', open);
    expect(opened, 1);
    expect(identical(first, second), isTrue);

    barrier.complete();
    expect(await Future.wait([first, second]), [1, 1]);
    expect(opened, 1);
  });

  test('a caller timeout leaves the shared operation pending', () async {
    final flights = McpSingleFlight<int>();
    final finish = Completer<int>();
    var opened = 0;
    final started = flights.start('peer', () {
      opened++;
      return finish.future;
    });

    await expectLater(started.timeout(const Duration(milliseconds: 20)),
        throwsA(isA<TimeoutException>()));
    // The approval is still up; a retry joins it instead of asking again.
    final retry = flights.start('peer', () {
      opened++;
      return finish.future;
    });
    expect(identical(started, retry), isTrue);
    expect(opened, 1);

    finish.complete(7);
    expect(await retry, 7);
  });

  test('a finished operation is not reused and its removal stays its own',
      () async {
    final flights = McpSingleFlight<int>();
    var opened = 0;
    Future<int> open(int value) {
      opened++;
      return Future.value(value);
    }

    expect(await flights.start('peer', () => open(1)), 1);
    await Future<void>.delayed(Duration.zero);
    expect(flights.pending('peer'), isNull);

    // A new operation starts after the first one is gone.
    final second = flights.start('peer', () {
      opened++;
      return Completer<int>().future;
    });
    expect(opened, 2);
    // Completing the old one again must not drop the new entry.
    await Future<void>.delayed(Duration.zero);
    expect(flights.pending('peer'), isNotNull);
    expect(identical(flights.pending('peer'), second), isTrue);
  });

  test('different keys run independently', () async {
    final flights = McpSingleFlight<int>();
    final a = Completer<int>();
    final b = Completer<int>();
    final first = flights.start('a', () => a.future);
    final second = flights.start('b', () => b.future);
    expect(identical(first, second), isFalse);
    a.complete(1);
    b.complete(2);
    expect(await Future.wait([first, second]), [1, 2]);
  });
}

import 'dart:async';

import 'package:flutter_hbb/mcp/mcp_dispatcher.dart';
import 'package:flutter_hbb/mcp/mcp_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('concurrent input operations finish without interleaving', () async {
    final input = McpInputState();
    final events = <String>[];
    Future<void> drag(String name) => input.run(() async {
          events.add('$name down');
          await Future<void>.delayed(Duration.zero);
          events.add('$name up');
        });
    await Future.wait([drag('a'), drag('b')]);
    expect(events, ['a down', 'a up', 'b down', 'b up']);
  });

  test('handoff cancels queued input and releases an in-flight press once',
      () async {
    final input = McpInputState();
    final started = Completer<void>();
    final sent = Completer<void>();
    final events = <String>[];
    final active = input.run(() async {
      started.complete();
      await sent.future;
      events.add('down');
      input.buttons.add('left');
      input.checkActive();
      events.add('unexpected move');
    });
    final cancelled = expectLater(active, throwsA(isA<McpToolException>()));
    await started.future;
    final queued = expectLater(
        input.run(() async => events.add('unexpected down')),
        throwsA(isA<McpToolException>()));
    Future<void> cleanup() async {
      for (final button in input.buttons) {
        events.add('$button up');
      }
      input.buttons.clear();
    }

    final released = input.release(cleanup);
    final repeated = input.release(cleanup);
    sent.complete();
    await Future.wait([cancelled, queued, released, repeated]);
    await expectLater(input.run(() async => events.add('late down')),
        throwsA(isA<McpToolException>()));
    expect(events, ['down', 'left up']);
  });

  test('a failed input call does not prevent release', () async {
    final input = McpInputState();
    final events = <String>[];
    final failed = expectLater(input.run(() async {
      input.keys.add('VK_SHIFT');
      throw StateError('FFI failed after key down');
    }), throwsStateError);
    await failed;
    await input.release(() async {
      for (final key in input.keys) {
        events.add('$key up');
      }
    });
    expect(events, ['VK_SHIFT up']);
  });
}

import 'dart:async';

import 'package:flutter_hbb/mobile/utils/mobile_input_session.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('CRLF text is submitted as one newline without an implicit Return',
      () async {
    final calls = <String>[];
    final session = MobileInputSession(
      canSend: () => true,
      sendText: (text) async => calls.add(text),
    );
    await session.submit('mañana\r\n日本\r\n\n끝');
    expect(calls, ['mañana\n日本\n\n끝']);
  });

  test('text, Return and subsequent remote keys preserve submission order',
      () async {
    final calls = <String>[];
    final textDone = Completer<void>();
    final session = MobileInputSession(
      canSend: () => true,
      sendText: (text) async {
        calls.add(text);
        await textDone.future;
      },
    );
    final text = session.submit('你好😀');
    final enter = session.dispatch(() async => calls.add('VK_RETURN'));
    final key = session.dispatch(() async => calls.add('VK_BACK'));
    await Future<void>.delayed(Duration.zero);
    expect(calls, ['你好😀']);
    textDone.complete();
    await Future.wait([text, enter, key]);
    expect(calls, ['你好😀', 'VK_RETURN', 'VK_BACK']);
  });

  test('failure cancels queued commands without retrying submitted text',
      () async {
    final calls = <String>[];
    final textDone = Completer<void>();
    final session = MobileInputSession(
      canSend: () => true,
      sendText: (_) => textDone.future,
    );
    final text = session.submit('draft');
    final key = session.dispatch(() async => calls.add('VK_RETURN'));
    final failures = Future.wait([
      expectLater(text, throwsStateError),
      expectLater(key, throwsStateError),
    ]);
    textDone.completeError(StateError('send failed'));
    await failures;
    expect(calls, isEmpty);
    await session.dispatch(() async => calls.add('VK_BACK'));
    expect(calls, ['VK_BACK'], reason: 'A new explicit action can recover');
  });

  test('a failed submission does not strand an already pressed remote key',
      () async {
    final calls = <String>[];
    final textDone = Completer<void>();
    var allowed = true;
    final session = MobileInputSession(
      canSend: () => allowed,
      sendText: (_) => textDone.future,
    );
    await session.dispatch(() async => calls.add('Ctrl down'));
    final text = session.submit('draft');
    final release =
        session.dispatch(() async => calls.add('Ctrl up'), isKeyRelease: true);
    final failed = expectLater(text, throwsStateError);
    await Future<void>.delayed(Duration.zero);
    allowed = false;
    textDone.completeError(StateError('send failed'));
    await Future.wait([failed, release]);
    expect(calls, ['Ctrl down', 'Ctrl up']);
  });

  test('revoked input and replaced sessions reject pending remote operations',
      () async {
    var allowed = true;
    final calls = <String>[];
    final textDone = Completer<void>();
    final session = MobileInputSession(
      canSend: () => allowed,
      sendText: (_) => textDone.future,
    );
    final text = session.submit('draft');
    final key = session.dispatch(() async => calls.add('VK_RETURN'));
    final failed = expectLater(key, throwsStateError);
    await Future<void>.delayed(Duration.zero);
    session.invalidate();
    allowed = false;
    textDone.complete();
    await Future.wait([text, failed]);
    expect(calls, isEmpty);
    await expectLater(
        session.dispatch(() async => calls.add('VK_BACK')), throwsStateError);
  });
}

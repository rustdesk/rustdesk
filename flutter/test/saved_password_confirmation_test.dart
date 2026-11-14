import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/saved_password_confirmation.dart';

void main() {
  test('uses the same stored peer for formatted and relay IDs', () {
    for (final id in ['123 456 789', '123456789/r', r'123456789\r']) {
      expect(savedPasswordPeerId(id), '123456789');
    }
    expect(savedPasswordPeerId('123 456 789/r@server.example?key=public-key'),
        '123456789@server.example');
    expect(savedPasswordPeerId('192.168.1.2:21118'), '192.168.1.2:21118');
  });

  test('only saved-password launches with the option on ask for confirmation',
      () async {
    for (final enabled in [false, true]) {
      for (final saved in [false, true]) {
        for (final method in ['', 'request-approval']) {
          var prompts = 0;
          final result = await SavedPasswordConfirmation().confirm(
            id: '123456789',
            enabled: enabled,
            loginMethod: method,
            hasSavedPassword: (_) async => saved,
            showConfirmation: (_) async {
              prompts++;
              return false;
            },
          );
          final shouldPrompt = enabled && saved && method.isEmpty;
          expect(prompts, shouldPrompt ? 1 : 0);
          expect(result, !shouldPrompt);
        }
      }
    }
  });

  test('only explicit acceptance proceeds; cancel and dismissal abort',
      () async {
    for (final answer in [true, false, null]) {
      final result = await SavedPasswordConfirmation().confirm(
        id: '123456789',
        enabled: true,
        loginMethod: '',
        hasSavedPassword: (_) async => true,
        showConfirmation: (_) async => answer,
      );
      expect(result, answer == true);
    }
  });

  test('repeated clicks cannot share approval to launch multiple sessions',
      () async {
    final gate = SavedPasswordConfirmation();
    final answer = Completer<bool>();
    var prompts = 0;
    Future<bool> connect(String id) => gate.confirm(
          id: id,
          enabled: true,
          loginMethod: '',
          hasSavedPassword: (_) async => true,
          showConfirmation: (_) {
            prompts++;
            return answer.future;
          },
        );
    final first = connect('123 456 789');
    expect(await connect('123456789/r'), isFalse);
    answer.complete(true);
    expect(await first, isTrue);
    expect(prompts, 1);
    // A later, deliberate launch asks again.
    expect(await connect('123456789'), isTrue);
    expect(prompts, 2);
  });

  test('failed credential lookup does not approve a connection or block retry',
      () async {
    final gate = SavedPasswordConfirmation();
    await expectLater(
        gate.confirm(
          id: '123456789',
          enabled: true,
          loginMethod: '',
          hasSavedPassword: (_) async => throw StateError('lookup failed'),
          showConfirmation: (_) async => true,
        ),
        throwsStateError);
    expect(
        await gate.confirm(
          id: '123456789',
          enabled: true,
          loginMethod: '',
          hasSavedPassword: (_) async => true,
          showConfirmation: (_) async => false,
        ),
        isFalse);
  });
}

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'mobile_text_input_test.dart' show InputFixture;

void main() {
  final platforms =
      TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS});
  testWidgets('Android Return stays a key without clearing the native editor',
      (tester) async {
    final input = InputFixture();
    await input.mount(tester);
    expect(tester.testTextInput.setClientArgs?['inputAction'],
        TextInputAction.done.toString());
    tester.testTextInput.log.clear();
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(tester.testTextInput.log.map((call) => call.method),
        isNot(contains('TextInput.clearClient')));
    await input.type(tester, 'he', composing: true);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(input.sent, [('', true), ('he', false), ('', true)]);
    expect(input.errors, isEmpty);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    await input.type(tester, 'x');
    expect(input.sent.last, ('x', false));
    await tester.pumpWidget(const SizedBox.shrink());
  }, variant: TargetPlatformVariant.only(TargetPlatform.android));

  testWidgets('Return retires callbacks from the preceding native editor',
      (tester) async {
    final input = InputFixture();
    await input.mount(tester);
    await input.type(tester, 'he',
        composing: defaultTargetPlatform == TargetPlatform.android);
    final client = currentClient(tester);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    await lateEdit(tester, client, insertion: false);
    expect(input.errors, isEmpty);
    expect(input.sent, [('he', false), ('', true)]);
    await input.type(tester, 'x');
    expect(input.sent.last, ('x', false));
    expect(tester.testTextInput.hasAnyClients, isTrue);
    await tester.pumpWidget(const SizedBox.shrink());
  }, variant: platforms);

  for (final sendReturn in [false, true]) {
    testWidgets(
        'old send failure preserves new composition, Return=$sendReturn',
        (tester) async {
      final input = InputFixture();
      final oldSend = Completer<void>();
      await input.mount(tester, submit: (text, enter) {
        if (text == 'old' || enter) return oldSend.future;
        return Future.value();
      });
      if (sendReturn) {
        await tester.testTextInput.receiveAction(TextInputAction.done);
      } else {
        await input.type(tester, 'old');
        input.controller.resetContext();
      }
      await input.type(tester, 'ni', composing: true);
      expect(tester.testTextInput.editingState?['text'], ' ni');
      oldSend.completeError(StateError('Previous submission failed'));
      await tester.pump();
      expect(input.errors, hasLength(1));
      final textAfterFailure = tester.testTextInput.editingState?['text'];
      await tester.pumpWidget(const SizedBox.shrink());
      expect(textAfterFailure, ' ni',
          reason: 'The new composition belongs to a different input context');
    }, variant: platforms);
  }

  for (final nativeClose in [false, true]) {
    testWidgets('closing input rejects stale composition, native=$nativeClose',
        (tester) async {
      final input = InputFixture();
      await input.mount(tester);
      await input.type(tester, 'he', composing: true);
      final client = currentClient(tester);
      final sent = [
        if (defaultTargetPlatform == TargetPlatform.android) ('he', false)
      ];
      addTearDown(() => tester.binding
          .handleAppLifecycleStateChanged(AppLifecycleState.resumed));
      Future<void>? closing;
      if (nativeClose) {
        closing = tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          SystemChannels.textInput.name,
          SystemChannels.textInput.codec.encodeMethodCall(
              MethodCall('TextInputClient.onConnectionClosed', [client])),
          (_) {},
        );
      } else {
        tester.binding
            .handleAppLifecycleStateChanged(AppLifecycleState.inactive);
        expect(tester.testTextInput.hasAnyClients, isFalse);
      }
      await lateEdit(tester, client, insertion: false);
      await closing;
      expect(input.sent, sent);
      expect(input.errors, isEmpty);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      input.focus.requestFocus();
      await tester.pump();
      await input.type(tester, '你好');
      expect(input.sent, [...sent, ('你好', false)]);
      await tester.pumpWidget(const SizedBox.shrink());
    },
        variant: nativeClose
            ? platforms
            : TargetPlatformVariant.only(TargetPlatform.iOS));
  }

  testWidgets('navigation retires text queued by the preceding native editor',
      (tester) async {
    final input = InputFixture();
    await input.mount(tester);
    await input.type(tester, 'he');
    final client = currentClient(tester);
    input.controller.resetContext();
    await tester.pump();
    await lateEdit(tester, client, insertion: true);
    expect(input.errors, isEmpty);
    expect(input.sent, [('he', false)]);
    await input.type(tester, 'x');
    expect(input.sent, [('he', false), ('x', false)]);
    expect(input.backspaces, 0);
    await tester.pumpWidget(const SizedBox.shrink());
  }, variant: platforms);
}

int currentClient(WidgetTester tester) => tester.testTextInput.log
    .lastWhere((call) => call.method == 'TextInput.setClient')
    .arguments[0] as int;

Future<void> lateEdit(WidgetTester tester, int client,
    {required bool insertion}) async {
  final delta = {
    'oldText': ' he',
    'deltaStart': insertion ? 3 : -1,
    'deltaEnd': insertion ? 3 : -1,
    'deltaText': insertion ? 'y' : '',
    'selectionBase': insertion ? 4 : 3,
    'selectionExtent': insertion ? 4 : 3,
    'composingBase': -1,
    'composingExtent': -1,
  };
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    SystemChannels.textInput.name,
    SystemChannels.textInput.codec.encodeMethodCall(MethodCall(
      'TextInputClient.updateEditingStateWithDeltas',
      [
        client,
        {
          'deltas': [delta, if (!insertion) delta]
        }
      ],
    )),
    (_) {},
  );
  await tester.pump();
}

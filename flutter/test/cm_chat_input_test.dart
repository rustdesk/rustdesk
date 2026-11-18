import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:uuid/uuid.dart';

class _Session extends Fake implements FFI {
  @override
  final sessionId = const Uuid().v4obj();
}

class _Binding extends Fake implements RustdeskImpl {
  final sent = <String>[];

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<void> cmSendChat(
      {required int connId, required String msg, dynamic hint}) async {
    sent.add(msg);
  }
}

void main() {
  testWidgets('focused CM chat accepts local typing, paste, Enter and IME',
      (tester) async {
    final binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    final session = _Session();
    final chat = ChatModel(WeakReference(session))..isConnManager = true;
    addTearDown(() {
      chat.dispose();
      chat.inputNode.dispose();
    });
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async => call.method == 'Clipboard.getData'
            ? {'text': 'clipboard text'}
            : null);
    addTearDown(() => tester.binding.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null));
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: TextField(
      controller: chat.textController,
      focusNode: chat.inputNode,
      autofocus: true,
    ))));
    await tester.pump();
    expect(await tester.sendKeyDownEvent(LogicalKeyboardKey.keyA), isFalse);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyV);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(chat.textController.text, 'clipboard text');

    chat.textController.text = 'local draft';
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(binding.sent, ['local draft']);
    expect(chat.textController.text, '');
    tester.testTextInput.enterText('Local IME text');
    await tester.pump();
    expect(chat.textController.text, 'Local IME text');
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show SessionID, isTest;
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:uuid/uuid.dart';

class _Binding extends Fake implements RustdeskImpl {
  _Binding(this.remote, this.allowRemote);

  final bool remote;
  final bool allowRemote;
  final sent = <(int, String)>[];

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      allowRemote ? 'Y' : 'N';

  @override
  Future<void> cmCheckClickTime({required int connId, dynamic hint}) async {}

  @override
  Future<double> cmGetClickTime({dynamic hint}) async =>
      remote ? DateTime.now().millisecondsSinceEpoch.toDouble() : 0;

  @override
  Future<void> cmSendChat(
      {required int connId, required String msg, dynamic hint}) async {
    sent.add((connId, msg));
  }
}

class _Session extends Fake implements FFI {
  @override
  final SessionID sessionId = const Uuid().v4obj();

  @override
  late final serverModel = ServerModel(WeakReference(this));
}

const _scenarios = [
  (
    name: 'remote Enter preserves the draft',
    remote: true,
    allow: false,
    disconnect: false
  ),
  (
    name: 'local Enter sends the draft',
    remote: false,
    allow: false,
    disconnect: false
  ),
  (
    name: 'allowed remote Enter sends the draft',
    remote: true,
    allow: true,
    disconnect: false
  ),
  (
    name: 'disconnect while sending preserves the draft',
    remote: false,
    allow: false,
    disconnect: true
  ),
];

void main() {
  setUp(() => isTest = true);
  tearDown(() => isTest = false);

  for (final scenario in _scenarios) {
    testWidgets(scenario.name, (tester) async {
      final binding = _Binding(scenario.remote, scenario.allow);
      platformFFI.setBindingForTesting(binding);
      final session = _Session();
      final client =
          Client(1, true, false, false, 'peer', 'peer', true, true, true);
      session.serverModel.clients.add(client);
      final chat = ChatModel(WeakReference(session))..isConnManager = true;
      final key = MessageKey('peer', 1);
      chat.updateConnIdOfKey(key);
      const draft = 'host clipboard';
      chat.textController.text = draft;
      await tester.pumpWidget(MaterialApp(
        home: Scaffold(
          body: TextField(
              controller: chat.textController, focusNode: chat.inputNode),
        ),
      ));
      chat.inputNode.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      client.disconnected = scenario.disconnect;
      await tester.pump(const Duration(milliseconds: 200));

      final allowed =
          (!scenario.remote || scenario.allow) && !scenario.disconnect;
      expect(binding.sent, allowed ? [(key.connId, draft)] : isEmpty);
      expect(chat.textController.text, allowed ? '' : draft);
      expect(chat.messages[key]?.chatMessages.length ?? 0, allowed ? 1 : 0);
      await tester.pumpWidget(const SizedBox.shrink());
      chat.dispose();
      chat.inputNode.dispose();
      session.serverModel.dispose();
    });
  }
}

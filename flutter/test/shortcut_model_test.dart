import 'dart:async';

import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show SessionID;
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_constants.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';
import 'package:uuid/uuid.dart';

class _ChatBinding extends Fake implements RustdeskImpl {
  final sent = <(SessionID, String)>[];
  final cmSent = <(int, String)>[];

  @override
  String translate({required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<void> sessionSendChat(
      {required SessionID sessionId, required String text, dynamic hint}) async {
    sent.add((sessionId, text));
  }

  @override
  Future<void> cmSendChat(
      {required int connId, required String msg, dynamic hint}) async {
    cmSent.add((connId, msg));
  }
}

class _Chat extends ChatModel {
  _Chat(FFI session) : super(WeakReference(session));

  final opened = <MessageKey>[];

  @override
  void toggleChatOverlay(
      {Offset? chatInitPos, bool requestSoftKeyboard = true}) {
    opened.add(currentKey);
  }
}

class _Session extends Fake implements FFI {
  @override
  bool closed = false;
  @override
  final String id = 'test-peer';
  @override
  final SessionID sessionId = const Uuid().v4obj();
  @override
  late final FfiModel ffiModel = FfiModel(WeakReference(this));
  @override
  late final _Chat chatModel = _Chat(this);
  @override
  late ShortcutModel shortcutModel;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _Session session;
  late ShortcutModel model;

  setUp(() {
    session = _Session();
    model = ShortcutModel(WeakReference(session));
    session.shortcutModel = model;
  });

  tearDown(() => model.clear());

  test('fresh Chat shortcut sends through its outgoing session', () async {
    final binding = _ChatBinding();
    platformFFI.setBindingForTesting(binding);
    final chat = session.chatModel;
    addTearDown(() {
      chat.dispose();
      chat.inputNode.dispose();
      session.ffiModel.dispose();
    });
    expect(chat.currentKey.isOut, isFalse);
    expect(chat.messages, isEmpty);
    registerSessionShortcutActions(session);

    model.onTriggered(kShortcutActionToggleChat);
    await pumpEventQueue();
    chat.send(ChatMessage(
        text: 'first message', user: chat.me, createdAt: DateTime.now()));
    await pumpEventQueue();

    expect(binding.sent, [(session.sessionId, 'first message')]);
    expect(binding.cmSent, isEmpty);
    expect(chat.opened, [MessageKey(session.id, ChatModel.clientModeID)]);
    expect(chat.messages.keys, [MessageKey(session.id, ChatModel.clientModeID)]);
  });

  test('a removed menu action cannot run its cached callback', () async {
    var calls = 0;
    model.register('mute', () => calls++);
    model.registerRefresher(['mute'], () => model.unregister('mute'));

    model.onTriggered('mute');
    await pumpEventQueue();

    expect(calls, 0);
  });

  test('an action becomes available without reopening its menu', () async {
    var calls = 0;
    model.registerRefresher(['screenshot'], () {
      model.register('screenshot', () => calls++);
    });

    model.onTriggered('screenshot');
    await pumpEventQueue();

    expect(calls, 1);
  });

  test('refresh finishes before invoking the current callback', () async {
    final ready = Completer<void>();
    final calls = <String>[];
    model.register('cursor', () => calls.add('stale'));
    model.registerRefresher(['cursor'], () async {
      await ready.future;
      model.register('cursor', () => calls.add('current'));
    });

    model.onTriggered('cursor');
    expect(calls, isEmpty);
    ready.complete();
    await pumpEventQueue();

    expect(calls, ['current']);
  });

  test('closing during a refresh prevents late registration and dispatch',
      () async {
    final ready = Completer<void>();
    var calls = 0;
    model.registerRefresher(['mute'], () async {
      await ready.future;
      model.register('mute', () => calls++);
    });

    model.onTriggered('mute');
    session.closed = true;
    model.clear();
    ready.complete();
    await pumpEventQueue();
    model.onTriggered('mute');
    await pumpEventQueue();

    expect(calls, 0);
  });
}

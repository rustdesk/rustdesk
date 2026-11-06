import 'dart:async';
import 'dart:convert';

import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show SessionID;
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_constants.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/local_flutter_shortcuts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';
import 'package:uuid/uuid.dart';

class _ChatBinding extends Fake implements RustdeskImpl {
  final sent = <(SessionID, String)>[];
  final cmSent = <(int, String)>[];

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<void> sessionSendChat(
      {required SessionID sessionId,
      required String text,
      dynamic hint}) async {
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

class _LegacyBinding extends _ChatBinding {
  final keys = <(String, bool, bool)>[];

  @override
  bool sessionGetToggleOptionSync(
          {required SessionID sessionId, required String arg, dynamic hint}) =>
      false;

  @override
  String mainGetInputSource({dynamic hint}) => 'Input source 2';
  @override
  bool mainCurrentIsWayland({dynamic hint}) => false;
  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      '{"enabled":true,"bindings":[{"action":"toggle_mute",'
      '"key":"s","mods":["alt"]}]}';

  @override
  Future<void> sessionInputKey(
      {required SessionID sessionId,
      required String name,
      required bool down,
      required bool press,
      required bool alt,
      required bool ctrl,
      required bool shift,
      required bool command,
      dynamic hint}) async {
    expect([press, ctrl, shift, command], [false, false, false, false]);
    keys.add((name, down, alt));
  }

  @override
  Future<void> sessionSendMouse(
      {required SessionID sessionId, required String msg, dynamic hint}) async {
    expect(jsonDecode(msg)['relative_mouse_mode'], '0');
  }
}

Future<void> _rawKey(InputModel input,
    {required int scan,
    required int key,
    bool down = true,
    bool alt = true}) async {
  final data = RawKeyEventDataLinux(
      keyHelper: GtkKeyHelper(),
      scanCode: scan,
      keyCode: key,
      unicodeScalarValues: key < 256 ? key : 0,
      modifiers: alt ? GtkKeyHelper.modifierMod1 : 0,
      isDown: down);
  final event = down ? RawKeyDownEvent(data: data) : RawKeyUpEvent(data: data);
  RawKeyboard.instance.handleRawKeyEvent(event);
  input.handleRawKeyEvent(event);
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
  @override
  ConnType get connType => ConnType.defaultConn;
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

  for (final peer in [
    kPeerPlatformWindows,
    kPeerPlatformLinux,
    kPeerPlatformMacOS
  ]) {
    for (final alt in [(64, 0xffe9, 'VK_MENU'), (108, 0xffea, 'RAlt')]) {
      test('Legacy $peer ${alt.$3} shortcut releases modifiers before dispatch',
          () async {
        final binding = _LegacyBinding();
        platformFFI.setBindingForTesting(binding);
        session.ffiModel.pi.platform = peer;
        final input = InputModel(WeakReference(session))
          ..keyboardMode = kKeyLegacyMode;
        addTearDown(() {
          session.closed = true;
          input.resetModifiers();
          input.disposeRelativeMouseMode();
          RawKeyboard.instance.clearKeysPressed();
          LocalFlutterShortcutDispatcher.resetFiredKeys();
          session.ffiModel.dispose();
        });
        final firedAt = <int>[];
        model.register(
            kShortcutActionToggleMute, () => firedAt.add(binding.keys.length));
        await _rawKey(input, scan: alt.$1, key: alt.$2);
        await _rawKey(input, scan: 39, key: 0x73);
        await pumpEventQueue();

        expect(binding.keys, [
          (alt.$3, true, true),
          if (peer == kPeerPlatformWindows) ('VK_CONTROL', true, true),
          // GTK reports Alt without a side on the letter event.
          ('VK_MENU', false, false),
          ('RAlt', false, false),
          if (peer == kPeerPlatformWindows) ('VK_CONTROL', false, false),
        ]);
        expect(firedAt, [binding.keys.length]);
        await _rawKey(input, scan: 39, key: 0x73, down: false);
        await _rawKey(input, scan: 38, key: 0x61);
        expect(binding.keys.last, ('VK_A', true, true));
        await _rawKey(input, scan: 38, key: 0x61, down: false);
        await _rawKey(input,
            scan: alt.$1, key: alt.$2, down: false, alt: false);
        await _rawKey(input, scan: 38, key: 0x61, alt: false);
        expect(binding.keys.last, ('VK_A', true, false));
        await _rawKey(input, scan: 38, key: 0x61, down: false, alt: false);
        expect(binding.keys.where((key) => key.$1 == 'VK_S'), isEmpty);
      });
    }
  }

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
    expect(
        chat.messages.keys, [MessageKey(session.id, ChatModel.clientModeID)]);
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

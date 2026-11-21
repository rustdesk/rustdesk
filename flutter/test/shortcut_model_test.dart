import 'dart:async';
import 'dart:convert';

import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show KeyEventResult;
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
  @override
  int versionToNumber({required String v, dynamic hint}) => switch (v) {
        '1.4.0' => 1004000,
        '1.2.0' => 1002000,
        _ => throw UnimplementedError(v),
      };

  final keys = <(String, bool, bool)>[];

  @override
  Future<void> sessionHandleFlutterRawKeyEvent(
      {required SessionID sessionId,
      required String name,
      required int platformCode,
      required int positionCode,
      required int lockModes,
      required bool downOrUp,
      dynamic hint}) async {}

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
    bool alt = true,
    int? modifiers}) async {
  final data = RawKeyEventDataLinux(
      keyHelper: GtkKeyHelper(),
      scanCode: scan,
      keyCode: key,
      unicodeScalarValues: key < 256 ? key : 0,
      modifiers: modifiers ?? (alt ? GtkKeyHelper.modifierMod1 : 0),
      isDown: down);
  final event = down
      ? RawKeyDownEvent(
          data: data,
          repeat: RawKeyboard.instance.physicalKeysPressed
              .contains(data.physicalKey))
      : RawKeyUpEvent(data: data);
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

InputModel _modifierInput(_Session session) {
  platformFFI.setBindingForTesting(_LegacyBinding());
  session.ffiModel.pi.platform = kPeerPlatformWindows;
  final input = InputModel(WeakReference(session))..keyboardMode = kKeyMapMode;
  addTearDown(() {
    session.closed = true;
    input.resetModifiers();
    input.disposeRelativeMouseMode();
    RawKeyboard.instance.clearKeysPressed();
    LocalFlutterShortcutDispatcher.resetFiredKeys();
    session.ffiModel.dispose();
  });
  return input;
}

void _expectNoRawReleases(InputModel input) {
  final released = <RawKeyEvent>[];
  input.toReleaseRawKeys.release((event) {
    released.add(event);
    return KeyEventResult.handled;
  });
  expect(released, isEmpty);
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

  test('raw releases in view-only do not leave mouse modifiers held', () async {
    final input = _modifierInput(session);
    session.ffiModel.pi.version = '1.4.0';
    await _rawKey(input, scan: 64, key: 0xffe9, modifiers: 0);
    session.ffiModel.setViewOnly(session.id, true);
    await _rawKey(input, scan: 64, key: 0xffe9, down: false);
    session.ffiModel.setViewOnly(session.id, false);
    await _rawKey(input, scan: 108, key: 0xffea, modifiers: 0);
    await _rawKey(input, scan: 108, key: 0xffea, down: false);
    expect(input.modify({}), isEmpty);
  });

  test('raw mouse modifiers survive a remapped non-modifier release', () async {
    final input = _modifierInput(session);
    for (final mapping in [
      (
        heldScan: 66,
        heldKey: 0xffe3,
        tapScan: 37,
        tapKey: 0xffe5,
        mask: GtkKeyHelper.modifierControl,
        name: 'ctrl'
      ),
      (
        heldScan: 64,
        heldKey: 0xffe9,
        tapScan: 108,
        tapKey: 0xff20,
        mask: GtkKeyHelper.modifierMod1,
        name: 'alt'
      ),
    ]) {
      await _rawKey(input,
          scan: mapping.heldScan, key: mapping.heldKey, modifiers: 0);
      await _rawKey(input,
          scan: mapping.tapScan, key: mapping.tapKey, modifiers: mapping.mask);
      await _rawKey(input,
          scan: mapping.tapScan,
          key: mapping.tapKey,
          down: false,
          modifiers: mapping.mask);
      expect(input.modify({}), {mapping.name: 'true'});
      await _rawKey(input,
          scan: mapping.heldScan,
          key: mapping.heldKey,
          down: false,
          modifiers: mapping.mask);
      expect(input.modify({}), isEmpty);
      _expectNoRawReleases(input);
    }
  });

  test('raw releases match the down event when GTK changes its logical key',
      () async {
    final input = _modifierInput(session);
    for (final releaseKey in [0xfe0a, 0xffe7]) {
      await _rawKey(input, scan: 64, key: 0xffe9, modifiers: 0);
      await _rawKey(input,
          scan: 50, key: 0xffe1, modifiers: GtkKeyHelper.modifierMod1);
      await _rawKey(input,
          scan: 64,
          key: releaseKey,
          down: false,
          modifiers: GtkKeyHelper.modifierMod1 | GtkKeyHelper.modifierShift);
      expect(input.modify({}), {'shift': 'true'});
      await _rawKey(input,
          scan: 50,
          key: 0xffe1,
          down: false,
          modifiers: GtkKeyHelper.modifierShift);
      expect(input.modify({}), isEmpty);
      _expectNoRawReleases(input);
    }
  });

  test('shifted Alt releases retain another held Alt, including remapped keys',
      () async {
    final input = _modifierInput(session);
    const alt = GtkKeyHelper.modifierMod1;
    const shift = GtkKeyHelper.modifierShift;
    for (final keys in [
      (left: 64, right: null),
      (left: 37, right: 108),
      (left: 133, right: 134),
    ]) {
      await _rawKey(input, scan: 50, key: 0xffe1, modifiers: 0);
      await _rawKey(input, scan: keys.left, key: 0xffe7, modifiers: shift);
      if (keys.right != null) {
        await _rawKey(input,
            scan: keys.right!, key: 0xffe8, modifiers: shift | alt);
      }
      for (final down in [true, false]) {
        await _rawKey(input,
            scan: 41, key: 0x46, down: down, modifiers: shift | alt);
      }
      await _rawKey(input,
          scan: 50, key: 0xffe1, down: false, modifiers: shift | alt);
      await _rawKey(input,
          scan: keys.left, key: 0xffe9, down: false, modifiers: alt);
      expect(input.modify({}), keys.right == null ? {} : {'alt': 'true'},
          reason: '$keys');
      if (keys.right != null) {
        await _rawKey(input,
            scan: keys.right!, key: 0xffea, down: false, modifiers: alt);
      }
      expect(input.modify({}), isEmpty);
      _expectNoRawReleases(input);
    }
  });

  test('remapped shifted Alt does not retain a released Super modifier',
      () async {
    final input = _modifierInput(session);
    for (final (superMask, numLock) in [
      (1 << 6, 0),
      ((1 << 6) | GtkKeyHelper.modifierMeta, 0),
      (1 << 6, GtkKeyHelper.modifierMod2),
    ]) {
      final alt = GtkKeyHelper.modifierMod1 | numLock;
      final shift = GtkKeyHelper.modifierShift | numLock;
      await _rawKey(input, scan: 50, key: 0xffe1, modifiers: numLock);
      await _rawKey(input, scan: 37, key: 0xffe7, modifiers: shift);
      for (final down in [true, false]) {
        await _rawKey(input,
            scan: 41, key: 0x46, down: down, modifiers: shift | alt);
      }
      await _rawKey(input, scan: 134, key: 0xffec, modifiers: shift | alt);
      await _rawKey(input,
          scan: 134,
          key: 0xffec,
          down: false,
          modifiers: shift | alt | superMask);
      expect(input.modify({}), {'shift': 'true', 'alt': 'true'});
      await _rawKey(input,
          scan: 50, key: 0xffe1, down: false, modifiers: shift | alt);
      await _rawKey(input, scan: 37, key: 0xffe9, down: false, modifiers: alt);
      expect(input.modify({}), isEmpty);
      _expectNoRawReleases(input);
    }
  });

  test('a held GTK Meta mapping does not keep Alt pressed', () async {
    final input = _modifierInput(session);
    const gtkMeta = 1 << 28;
    const gtkMod4 = 1 << 6;
    const alt = GtkKeyHelper.modifierMod1;
    const shift = GtkKeyHelper.modifierShift;
    // Wayland omits the virtual Meta bit while Mod1 is active.
    for (final meta in [gtkMeta | gtkMod4, gtkMod4]) {
      await _rawKey(input, scan: 64, key: 0xffe9, modifiers: 0);
      await _rawKey(input, scan: 108, key: 0xffea, modifiers: alt);
      await _rawKey(input, scan: 50, key: 0xffe1, modifiers: alt);
      await _rawKey(input, scan: 133, key: 0xffe7, modifiers: alt | shift);
      await _rawKey(input,
          scan: 50, key: 0xffe1, down: false, modifiers: alt | shift | meta);
      await _rawKey(input,
          scan: 64, key: 0xffe9, down: false, modifiers: alt | meta);
      expect(input.modify({}), {'alt': 'true'});
      await _rawKey(input,
          scan: 108, key: 0xffea, down: false, modifiers: alt | meta);
      expect(input.modify({}), isEmpty);
      await _rawKey(input,
          scan: 133, key: 0xffe7, down: false, modifiers: meta);
      _expectNoRawReleases(input);
    }
  });

  test('raw group releases retain only modifiers held by logical modifier keys',
      () async {
    final input = _modifierInput(session);
    for (final releaseLeftFirst in [false, true]) {
      await _rawKey(input, scan: 64, key: 0xffe9, modifiers: 0);
      await _rawKey(input,
          scan: 108, key: 0xfe08, modifiers: GtkKeyHelper.modifierMod1);
      await _rawKey(input,
          scan: releaseLeftFirst ? 64 : 108,
          key: releaseLeftFirst ? 0xfe0a : 0xfe08,
          down: false,
          modifiers: GtkKeyHelper.modifierMod1);
      // The right key pressed as a group switch, so only the left key holds Alt.
      expect(input.modify({}), releaseLeftFirst ? {} : {'alt': 'true'});
      await _rawKey(input,
          scan: releaseLeftFirst ? 108 : 64,
          key: releaseLeftFirst ? 0xffea : 0xfe0a,
          down: false,
          modifiers: releaseLeftFirst ? 0 : GtkKeyHelper.modifierMod1);
      expect(input.modify({}), isEmpty);
      _expectNoRawReleases(input);
    }
  });

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

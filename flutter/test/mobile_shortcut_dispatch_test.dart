import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show SessionID;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mobile/utils/mobile_input_session.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/local_flutter_shortcuts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';
import 'package:uuid/uuid.dart';

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
  late final shortcutModel = ShortcutModel(WeakReference(this));
  @override
  ConnType get connType => ConnType.defaultConn;
}

class _Binding extends Fake implements RustdeskImpl {
  _Binding(this.action);
  final String action;
  final keys = <(String, bool, bool)>[];

  @override
  String mainGetInputSource({dynamic hint}) => 'Input source 2';
  @override
  bool mainCurrentIsWayland({dynamic hint}) => false;
  @override
  bool sessionGetToggleOptionSync(
          {required SessionID sessionId, required String arg, dynamic hint}) =>
      false;
  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      '{"enabled":true,"bindings":[{"action":"$action",'
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
    keys.add((name, down, alt));
  }

  @override
  Future<void> sessionSendMouse(
      {required SessionID sessionId,
      required String msg,
      dynamic hint}) async {}
}

// Raw events exercise the Legacy path shared by mobile Android peers.
void _key(InputModel input,
    {required int scan, required int key, bool alt = true}) {
  final event = RawKeyDownEvent(
      data: RawKeyEventDataLinux(
          keyHelper: GtkKeyHelper(),
          scanCode: scan,
          keyCode: key,
          unicodeScalarValues: key < 256 ? key : 0,
          modifiers: alt ? GtkKeyHelper.modifierMod1 : 0,
          isDown: true));
  RawKeyboard.instance.handleRawKeyEvent(event);
  input.handleRawKeyEvent(event);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  for (final scenario in [
    (kPeerPlatformAndroid, false),
    (kPeerPlatformWindows, false),
    (kPeerPlatformAndroid, true),
  ]) {
    test(
        'mobile ${scenario.$1} cleanup follows text, held letter=${scenario.$2}',
        () => verifyShortcutOrder(peer: scenario.$1, holdLetter: scenario.$2));
  }
}

Future<void> verifyShortcutOrder(
    {required String peer, required bool holdLetter}) async {
  final session = _Session();
  final action =
      holdLetter ? kShortcutActionToggleViewOnly : kShortcutActionToggleMute;
  final binding = _Binding(action);
  platformFFI.setBindingForTesting(binding);
  session.ffiModel.pi.platform = peer;
  final input = InputModel(WeakReference(session))
    ..keyboardMode = kKeyLegacyMode;
  final accepted = Completer<void>();
  final mobile = MobileInputSession(
      canSend: () => !session.closed, sendText: (_) => accepted.future);
  input.mobileKeyboardDispatcher = (send, {isKeyRelease = false}) =>
      mobile.dispatch(send, isKeyRelease: isKeyRelease);
  addTearDown(() => _disposeInput(session, input));
  final firedAt = <int>[];
  session.shortcutModel
      .register(action, () => firedAt.add(binding.keys.length));
  final text = mobile.submit('pending text');
  if (holdLetter) _key(input, scan: 38, key: 0x61, alt: false);
  _key(input, scan: 64, key: 0xffe9);
  _key(input, scan: 39, key: 0x73);
  await pumpEventQueue();
  final earlyKeys = List.of(binding.keys);
  final earlyActions = List.of(firedAt);
  accepted.complete();
  await text;
  await pumpEventQueue();
  expect(earlyKeys, isEmpty,
      reason: 'Shortcut cleanup must wait for queued input');
  expect(earlyActions, isEmpty);
  expect(binding.keys, [
    if (holdLetter) ('VK_A', true, false),
    ('VK_MENU', true, true),
    if (peer == kPeerPlatformWindows) ('VK_CONTROL', true, true),
    ('VK_MENU', false, false),
    ('RAlt', false, false),
    if (peer == kPeerPlatformWindows) ('VK_CONTROL', false, false),
    if (holdLetter) ('VK_A', false, false),
  ]);
  expect(firedAt, [binding.keys.length]);
}

void _disposeInput(_Session session, InputModel input) {
  session.closed = true;
  input.mobileKeyboardDispatcher = null;
  input.resetModifiers();
  input.disposeRelativeMouseMode();
  RawKeyboard.instance.clearKeysPressed();
  LocalFlutterShortcutDispatcher.resetFiredKeys();
  session.shortcutModel.clear();
  session.ffiModel.dispose();
}

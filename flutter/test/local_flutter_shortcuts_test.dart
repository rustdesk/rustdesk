import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/local_flutter_shortcuts.dart';

LocalFlutterShortcutDispatcher dispatcherFor(
    void Function(String) onTriggered) {
  final dispatcher = LocalFlutterShortcutDispatcher(onTriggered: onTriggered);
  addTearDown(dispatcher.clear);
  return dispatcher;
}

class _ShortcutHarness {
  final String? Function(KeyEvent) match;
  final Future<void> Function() releaseModifiers;
  final LocalFlutterShortcutDispatcher dispatcher;

  _ShortcutHarness({
    required this.match,
    required this.releaseModifiers,
    required void Function(String) onTriggered,
  }) : dispatcher = dispatcherFor(onTriggered);

  bool tryDispatch(KeyEvent event) => dispatcher.tryDispatch(
        event,
        match: () => match(event),
        releaseModifiers: releaseModifiers,
      );

  void clear() => dispatcher.clear();
}

KeyEvent down(PhysicalKeyboardKey key,
        {LogicalKeyboardKey logicalKey = LogicalKeyboardKey.keyP}) =>
    KeyDownEvent(
      physicalKey: key,
      logicalKey: logicalKey,
      timeStamp: Duration.zero,
    );

KeyEvent repeat(PhysicalKeyboardKey key) => KeyRepeatEvent(
      physicalKey: key,
      logicalKey: LogicalKeyboardKey.keyP,
      timeStamp: Duration.zero,
    );

KeyEvent up(PhysicalKeyboardKey key,
        {LogicalKeyboardKey logicalKey = LogicalKeyboardKey.keyP}) =>
    KeyUpEvent(
      physicalKey: key,
      logicalKey: logicalKey,
      timeStamp: Duration.zero,
    );

RawKeyEvent rawP({required bool down, bool repeat = false, bool ctrl = false}) {
  final data = RawKeyEventDataLinux(
    keyHelper: GtkKeyHelper(),
    scanCode: 33,
    keyCode: 112,
    unicodeScalarValues: 112,
    modifiers: ctrl ? GtkKeyHelper.modifierControl : 0,
    isDown: down,
  );
  return down
      ? RawKeyDownEvent(data: data, repeat: repeat)
      : RawKeyUpEvent(data: data);
}

void main() {
  test('owns repeats and release after the chord modifiers are released',
      () async {
    var modifiersPressed = true;
    final actions = <String>[];
    final dispatcher = _ShortcutHarness(
      match: (_) => modifiersPressed ? 'screenshot' : null,
      releaseModifiers: () async {},
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    modifiersPressed = false;
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isFalse);
    await Future<void>.value();
    expect(actions, ['screenshot']);
  });

  test('owns each overlapping shortcut key until its own release', () async {
    var modifiersPressed = true;
    final actions = <String>[];
    final dispatcher = _ShortcutHarness(
      match: (event) => modifiersPressed
          ? event.physicalKey == PhysicalKeyboardKey.keyP
              ? 'screenshot'
              : 'chat'
          : null,
      releaseModifiers: () async {},
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyC)), isTrue);
    modifiersPressed = false;
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyC)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isFalse);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyC)), isTrue);
    await Future<void>.value();
    expect(actions, ['screenshot', 'chat']);
  });

  test('binding changes do not forward a previously consumed key', () async {
    var enabled = true;
    final actions = <String>[];
    final dispatcher = _ShortcutHarness(
      match: (_) => enabled ? 'screenshot' : null,
      releaseModifiers: () async {},
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    enabled = false;
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isFalse);
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isFalse);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isFalse);
    await Future<void>.value();
    expect(actions, ['screenshot']);
  });

  test('unmatched key stays forwarded when modifiers change during repeat',
      () async {
    var modifiersPressed = false;
    final actions = <String>[];
    final dispatcher = _ShortcutHarness(
      match: (_) => modifiersPressed ? 'screenshot' : null,
      releaseModifiers: () async {},
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isFalse);
    modifiersPressed = true;
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isFalse);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isFalse);
    await Future<void>.value();
    expect(actions, isEmpty);
  });

  test('auto-repeat cannot trigger an action again', () async {
    final actions = <String>[];
    final dispatcher = _ShortcutHarness(
      match: (_) => 'screenshot',
      releaseModifiers: () async {},
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    await Future<void>.value();
    expect(actions, ['screenshot', 'screenshot']);
  });

  test('releases remote modifiers before running the action', () async {
    final released = Completer<void>();
    final actions = <String>[];
    var releases = 0;
    final dispatcher = _ShortcutHarness(
      match: (_) => 'screenshot',
      releaseModifiers: () {
        releases++;
        return released.future;
      },
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    expect(releases, 1);
    expect(actions, isEmpty);
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(releases, 1);
    released.complete();
    await Future<void>.value();
    expect(actions, ['screenshot']);
  });

  test('closing the session clears owned keys and pending actions', () async {
    final released = Completer<void>();
    final actions = <String>[];
    var enabled = true;
    final dispatcher = _ShortcutHarness(
      match: (_) => enabled ? 'screenshot' : null,
      releaseModifiers: () => released.future,
      onTriggered: actions.add,
    );

    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    dispatcher.clear();
    enabled = false;
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isFalse);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isFalse);
    released.complete();
    await Future<void>.value();
    expect(actions, isEmpty);

    enabled = true;
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isTrue);
    await Future<void>.value();
    expect(actions, ['screenshot']);
  });

  test('raw legacy shortcuts own repeats and release after modifiers change',
      () async {
    addTearDown(RawKeyboard.instance.clearKeysPressed);
    final actions = <String>[];
    final dispatcher = dispatcherFor(actions.add);
    bool dispatch(RawKeyEvent event) {
      RawKeyboard.instance.handleRawKeyEvent(event);
      return dispatcher.tryDispatchRaw(
        event,
        match: () => event.isControlPressed ? 'screenshot' : null,
        releaseModifiers: () async {},
      );
    }

    expect(dispatch(rawP(down: true, ctrl: true)), isTrue);
    expect(dispatch(rawP(down: true, repeat: true)), isTrue);
    expect(dispatch(rawP(down: false)), isTrue);
    expect(dispatch(rawP(down: true)), isFalse);
    expect(dispatch(rawP(down: true, repeat: true, ctrl: true)), isFalse);
    expect(dispatch(rawP(down: false, ctrl: true)), isFalse);
    await Future<void>.value();
    expect(actions, ['screenshot']);
  });

  test('read-only shortcut release is consumed after local matching stops', () {
    var viewOnly = true;
    final actions = <String>[];
    final dispatcher = dispatcherFor((action) {
      actions.add(action);
      viewOnly = false;
    });

    expect(
      dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP),
          match: () => viewOnly ? 'toggle_view_only' : null),
      isTrue,
    );
    expect(viewOnly, isFalse);
    expect(dispatcher.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(dispatcher.tryDispatch(down(PhysicalKeyboardKey.keyP)), isFalse);
    expect(actions, ['toggle_view_only']);
  });

  test('raw read-only shortcut keeps ownership when input routing changes', () {
    final actions = <String>[];
    final dispatcher = dispatcherFor(actions.add);

    expect(
      dispatcher.tryDispatchRaw(rawP(down: true, ctrl: true),
          match: () => 'toggle_view_only'),
      isTrue,
    );
    expect(dispatcher.tryDispatchRaw(rawP(down: true, repeat: true)), isTrue);
    expect(dispatcher.tryDispatchRaw(rawP(down: false)), isTrue);
    expect(dispatcher.tryDispatchRaw(rawP(down: true)), isFalse);
    expect(actions, ['toggle_view_only']);
  });

  test('shortcut key ownership follows focus to another session', () {
    final actions = <String>[];
    final first = dispatcherFor(actions.add);
    final second = dispatcherFor(actions.add);

    expect(
      first.tryDispatch(down(PhysicalKeyboardKey.keyP),
          match: () => 'next_tab'),
      isTrue,
    );
    expect(second.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(second.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(
      first.tryDispatch(down(PhysicalKeyboardKey.keyP),
          match: () => 'next_tab'),
      isTrue,
    );
    expect(actions, ['next_tab', 'next_tab']);
  });

  test('closing another session preserves the original session owned key', () {
    final first = dispatcherFor((_) {});
    final second = dispatcherFor((_) {});
    first.tryDispatch(down(PhysicalKeyboardKey.keyP),
        match: () => 'screenshot');
    second.tryDispatch(down(PhysicalKeyboardKey.keyC), match: () => 'chat');

    second.clear();
    expect(second.tryDispatch(repeat(PhysicalKeyboardKey.keyP)), isTrue);
    expect(first.tryDispatch(repeat(PhysicalKeyboardKey.keyC)), isFalse);
    expect(second.tryDispatch(up(PhysicalKeyboardKey.keyP)), isTrue);
    expect(first.tryDispatch(up(PhysicalKeyboardKey.keyP)), isFalse);
  });

  test('restores released modifiers before the next ordinary map key', () {
    final dispatcher = dispatcherFor((_) {});
    dispatcher.recordReleasedModifiers([
      PhysicalKeyboardKey.controlLeft,
      PhysicalKeyboardKey.altRight,
    ]);
    final pressed = {PhysicalKeyboardKey.controlLeft, PhysicalKeyboardKey.keyC};

    expect(
      dispatcher.takeModifiersToRestore(
          down(PhysicalKeyboardKey.keyC), pressed),
      [PhysicalKeyboardKey.controlLeft],
    );
    expect(
      dispatcher.takeModifiersToRestore(
          repeat(PhysicalKeyboardKey.keyC), pressed),
      isEmpty,
    );
  });

  test('modifier events do not replay previously released modifiers', () {
    final dispatcher = dispatcherFor((_) {});
    dispatcher.recordReleasedModifiers([PhysicalKeyboardKey.controlLeft]);
    expect(
      dispatcher.takeModifiersToRestore(down(PhysicalKeyboardKey.shiftLeft),
          {PhysicalKeyboardKey.controlLeft, PhysicalKeyboardKey.shiftLeft}),
      isEmpty,
    );
    expect(
      dispatcher.takeModifiersToRestore(
          up(PhysicalKeyboardKey.controlLeft), {PhysicalKeyboardKey.shiftLeft}),
      isEmpty,
    );
    expect(
      dispatcher.takeModifiersToRestore(down(PhysicalKeyboardKey.keyC),
          {PhysicalKeyboardKey.shiftLeft, PhysicalKeyboardKey.keyC}),
      isEmpty,
    );
  });

  test('session closure drops pending modifier replay', () {
    final dispatcher = dispatcherFor((_) {});
    dispatcher.recordReleasedModifiers([PhysicalKeyboardKey.controlLeft]);
    dispatcher.clear();
    expect(
      dispatcher.takeModifiersToRestore(down(PhysicalKeyboardKey.keyC),
          {PhysicalKeyboardKey.controlLeft, PhysicalKeyboardKey.keyC}),
      isEmpty,
    );
  });

  for (final releaseControl in [false, true]) {
    test(
        'view-only resume restores only still-held modifiers '
        '(releaseControl=$releaseControl)', () {
      final keyboard = HardwareKeyboard();
      var viewOnly = true;
      final dispatcher = dispatcherFor((_) => viewOnly = false);
      final controlDown = down(PhysicalKeyboardKey.controlLeft,
          logicalKey: LogicalKeyboardKey.controlLeft);
      keyboard.handleKeyEvent(controlDown);
      expect(dispatcher.tryDispatch(controlDown, viewOnly: true), isFalse);

      final shortcutDown = down(PhysicalKeyboardKey.keyP);
      keyboard.handleKeyEvent(shortcutDown);
      expect(
        dispatcher.tryDispatch(shortcutDown,
            viewOnly: true, match: () => 'toggle_view_only'),
        isTrue,
      );
      expect(viewOnly, isFalse);
      final shortcutUp = up(PhysicalKeyboardKey.keyP);
      keyboard.handleKeyEvent(shortcutUp);
      expect(dispatcher.tryDispatch(shortcutUp), isTrue);
      if (releaseControl) {
        final controlUp = up(PhysicalKeyboardKey.controlLeft,
            logicalKey: LogicalKeyboardKey.controlLeft);
        keyboard.handleKeyEvent(controlUp);
      }

      final copyDown =
          down(PhysicalKeyboardKey.keyC, logicalKey: LogicalKeyboardKey.keyC);
      keyboard.handleKeyEvent(copyDown);
      expect(dispatcher.tryDispatch(copyDown), isFalse);
      expect(
        dispatcher.resumeModifiersAfterViewOnly(
            copyDown.physicalKey, keyboard.physicalKeysPressed),
        isTrue,
      );
      expect(
        dispatcher.takeModifiersToRestore(
            copyDown, keyboard.physicalKeysPressed),
        releaseControl ? isEmpty : [PhysicalKeyboardKey.controlLeft],
      );
      expect(keyboard.isControlPressed, !releaseControl);
    });
  }
}

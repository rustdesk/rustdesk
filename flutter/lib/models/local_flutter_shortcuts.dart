import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../common/widgets/keyboard_shortcuts/shortcut_constants.dart';

class LocalFlutterShortcutDispatcher {
  final void Function(String) onTriggered;
  // Keys whose press fired a shortcut and whose release has not arrived yet.
  // Ownership is physical: it is shared by every session, because a shortcut
  // can switch focus or close its own session before the key is released, and
  // it is never dropped with a session. A key whose release is missed heals
  // itself: its next press is consumed as a repeat and that release removes it.
  static final _firedKeys = <PhysicalKeyboardKey>{};
  // Actions waiting for the release of the key that fired them, with the
  // session that fired them. See [kShortcutActionsRunOnKeyUp].
  static final _keyUpActions =
      <PhysicalKeyboardKey, (LocalFlutterShortcutDispatcher, String)>{};
  static final _clipboardModifierReleases =
      <PhysicalKeyboardKey, (LocalFlutterShortcutDispatcher, Future<void>)>{};
  final _releasedModifiers = <PhysicalKeyboardKey>{};
  final _forwardedKeyReleases =
      <PhysicalKeyboardKey, Future<void> Function()>{};
  bool _viewOnlyShortcutPending = false;
  int _generation = 0;

  LocalFlutterShortcutDispatcher({required this.onTriggered});

  /// Drops this session's pending actions and modifier replay. Fired keys
  /// stay owned until their release, whichever session receives it.
  void clear() {
    _keyUpActions.removeWhere((_, pending) => identical(pending.$1, this));
    _clipboardModifierReleases
        .removeWhere((_, pending) => identical(pending.$1, this));
    _releasedModifiers.clear();
    _forwardedKeyReleases.clear();
    _viewOnlyShortcutPending = false;
    _generation++;
  }

  @visibleForTesting
  static void resetFiredKeys() {
    _firedKeys.clear();
    _keyUpActions.clear();
    _clipboardModifierReleases.clear();
  }

  void recordReleasedModifiers(Iterable<PhysicalKeyboardKey> keys) {
    _releasedModifiers.addAll(keys);
  }

  void recordForwardedKey(PhysicalKeyboardKey key,
      {required bool down, required Future<void> Function() release}) {
    if (_isModifier(key)) return;
    if (down) {
      _forwardedKeyReleases.putIfAbsent(key, () => release);
    } else {
      _forwardedKeyReleases.remove(key);
    }
  }

  bool resumeModifiersAfterViewOnly(
      PhysicalKeyboardKey incoming, Set<PhysicalKeyboardKey> pressed) {
    if (!_viewOnlyShortcutPending) return false;
    _viewOnlyShortcutPending = false;
    _releasedModifiers
        .addAll(pressed.where((key) => key != incoming && _isModifier(key)));
    return true;
  }

  List<PhysicalKeyboardKey> takeModifiersToRestore(
          KeyEvent event, Set<PhysicalKeyboardKey> pressed) =>
      _takeModifiersToRestore(event.physicalKey,
          event is KeyDownEvent || event is KeyRepeatEvent, pressed);

  List<PhysicalKeyboardKey> takeRawModifiersToRestore(
          RawKeyEvent event, Set<PhysicalKeyboardKey> pressed) =>
      _takeModifiersToRestore(
          event.physicalKey, event is RawKeyDownEvent, pressed);

  List<PhysicalKeyboardKey> _takeModifiersToRestore(
      PhysicalKeyboardKey incoming,
      bool down,
      Set<PhysicalKeyboardKey> pressed) {
    _releasedModifiers.removeWhere((key) => !pressed.contains(key));
    if (!down || _isModifier(incoming)) return [];
    final keys = _releasedModifiers.toList();
    _releasedModifiers.clear();
    return keys;
  }

  bool _isModifier(PhysicalKeyboardKey key) {
    // HID keyboard modifier usages are contiguous.
    return key.usbHidUsage >= PhysicalKeyboardKey.controlLeft.usbHidUsage &&
        key.usbHidUsage <= PhysicalKeyboardKey.metaRight.usbHidUsage;
  }

  bool tryDispatch(
    KeyEvent event, {
    bool viewOnly = false,
    String? Function()? match,
    Future<void> Function()? releaseModifiers,
  }) =>
      _tryDispatch(
        event.physicalKey,
        down: event is KeyDownEvent,
        up: event is KeyUpEvent,
        viewOnly: viewOnly,
        match: match,
        releaseModifiers: releaseModifiers,
      );

  bool tryDispatchRaw(
    RawKeyEvent event, {
    bool viewOnly = false,
    String? Function()? match,
    Future<void> Function()? releaseModifiers,
  }) =>
      _tryDispatch(
        event.physicalKey,
        down: event is RawKeyDownEvent && !event.repeat,
        up: event is RawKeyUpEvent,
        viewOnly: viewOnly,
        match: match,
        releaseModifiers: releaseModifiers,
      );

  bool _tryDispatch(
    PhysicalKeyboardKey key, {
    required bool down,
    required bool up,
    required bool viewOnly,
    String? Function()? match,
    Future<void> Function()? releaseModifiers,
  }) {
    if (up) {
      if (!_firedKeys.remove(key)) return false;
      final pending = _keyUpActions.remove(key);
      final release = _clipboardModifierReleases.remove(key)?.$2;
      if (pending != null) {
        final (owner, action) = pending;
        unawaited(
            owner._trigger(action, release == null ? null : () => release));
      }
      return true;
    }
    // Safari can omit the previous Cmd-held keyup. A fresh browser keydown
    // starts a new press; KeyRepeatEvent must still belong to the old chord.
    if (kIsWeb && defaultTargetPlatform == TargetPlatform.macOS && down) {
      _firedKeys.remove(key);
    }
    if (_firedKeys.contains(key)) return true;
    if (!down) return false;
    final action = match?.call();
    if (action == null) return false;
    _firedKeys.add(key);
    if (viewOnly) _viewOnlyShortcutPending = true;
    // macOS browsers can omit letter keyups while Cmd is held. Like the JS
    // input source, dispatch on keydown there; remote cleanup still runs first.
    // Web clipboard reads also need the key-down's browser user activation.
    final runOnKeyUp =
        !(kIsWeb && defaultTargetPlatform == TargetPlatform.macOS) &&
            (kShortcutActionsRunOnKeyUp.contains(action) ||
                (!kIsWeb && action == kShortcutActionSendClipboardKeystrokes));
    if (runOnKeyUp) {
      _keyUpActions[key] = (this, action);
    } else {
      _keyUpActions.remove(key);
    }
    if (runOnKeyUp &&
        action == kShortcutActionSendClipboardKeystrokes &&
        releaseModifiers != null) {
      // The release can precede completion of the asynchronous FFI calls.
      final release = Future<void>.sync(releaseModifiers);
      _clipboardModifierReleases[key] = (this, release);
      unawaited(_trigger(null, () => release));
    } else {
      unawaited(_trigger(runOnKeyUp ? null : action, releaseModifiers));
    }
    return true;
  }

  /// Releases the remote modifiers, then runs [action] unless it is null or
  /// this session was cleared meanwhile.
  Future<void> _trigger(
      String? action, Future<void> Function()? releaseModifiers) async {
    final generation = _generation;
    try {
      if (releaseModifiers != null) await releaseModifiers();
      if (action == null || generation != _generation) return;
      if ((action == kShortcutActionToggleViewOnly ||
              action == kShortcutActionToggleShowMyCursor ||
              action == kShortcutActionKeyboardModeMap ||
              action == kShortcutActionKeyboardModeTranslate ||
              action == kShortcutActionKeyboardModeLegacy ||
              kShortcutActionsRunOnKeyUp.contains(action) ||
              (!kIsWeb && action == kShortcutActionSendClipboardKeystrokes)) &&
          _forwardedKeyReleases.isNotEmpty) {
        // Focus changes and view-only can keep physical keyups from the peer.
        // Mode changes can encode later keyups differently from their keydowns.
        final releases = _forwardedKeyReleases.values.toList();
        _forwardedKeyReleases.clear();
        await Future.wait(releases.map((release) => Future<void>.sync(release)));
      }
      if (generation == _generation) onTriggered(action);
    } catch (e, st) {
      debugPrint('Local shortcut failed for $action: $e\n$st');
    }
  }
}

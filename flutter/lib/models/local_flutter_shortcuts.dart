import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

class LocalFlutterShortcutDispatcher {
  final void Function(String) onTriggered;
  // Keys whose press fired a shortcut and whose release has not arrived yet.
  // Ownership is physical: it is shared by every session, because a shortcut
  // can switch focus or close its own session before the key is released, and
  // it is never dropped with a session. A key whose release is missed heals
  // itself: its next press is consumed as a repeat and that release removes it.
  static final _firedKeys = <PhysicalKeyboardKey>{};
  final _releasedModifiers = <PhysicalKeyboardKey>{};
  bool _viewOnlyShortcutPending = false;
  int _generation = 0;

  LocalFlutterShortcutDispatcher({required this.onTriggered});

  /// Drops this session's pending action and modifier replay. Fired keys
  /// stay owned until their release, whichever session receives it.
  void clear() {
    _releasedModifiers.clear();
    _viewOnlyShortcutPending = false;
    _generation++;
  }

  @visibleForTesting
  static void resetFiredKeys() => _firedKeys.clear();

  void recordReleasedModifiers(Iterable<PhysicalKeyboardKey> keys) {
    _releasedModifiers.addAll(keys);
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
    if (up) return _firedKeys.remove(key);
    if (_firedKeys.contains(key)) return true;
    if (!down) return false;
    final action = match?.call();
    if (action == null) return false;
    _firedKeys.add(key);
    if (viewOnly) _viewOnlyShortcutPending = true;
    unawaited(_trigger(action, releaseModifiers));
    return true;
  }

  Future<void> _trigger(
      String action, Future<void> Function()? releaseModifiers) async {
    final generation = _generation;
    try {
      if (releaseModifiers != null) await releaseModifiers();
      if (generation == _generation) onTriggered(action);
    } catch (e, st) {
      debugPrint('Local shortcut failed for $action: $e\n$st');
    }
  }
}

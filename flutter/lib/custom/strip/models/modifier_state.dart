import 'package:flutter/foundation.dart';

enum ModifierMode { off, oneShot, sticky, held }

/// Tracks held-modifier state without emitting key events. Modifier state is
/// applied as flags on the next regular KeyEvent (via InputBridge.tapKey's
/// `modifiers` argument), matching upstream RustDesk's `inputModel.command`
/// pattern. RustDesk's macOS server reads these flags from
/// `key_event.modifiers` to set CGEventFlagCommand on the synthetic event;
/// emitting Meta as a separate keyDown does not translate to that flag.
class ModifierController extends ChangeNotifier {
  final Map<String, ModifierMode> _state = {};

  Set<String> get heldModifiers => _state.entries
      .where((e) => e.value != ModifierMode.off)
      .map((e) => e.key)
      .toSet();

  ModifierMode modeFor(String name) => _state[name] ?? ModifierMode.off;

  // Cycles: off → oneShot → sticky → off
  void cycleTap(String name) {
    final current = modeFor(name);
    switch (current) {
      case ModifierMode.off:
        _state[name] = ModifierMode.oneShot;
      case ModifierMode.oneShot:
        _state[name] = ModifierMode.sticky;
      case ModifierMode.sticky:
      case ModifierMode.held:
        _state[name] = ModifierMode.off;
    }
    notifyListeners();
  }

  void hold(String name) {
    _state[name] = ModifierMode.held;
    notifyListeners();
  }

  void release(String name) {
    if (modeFor(name) == ModifierMode.held) {
      _state[name] = ModifierMode.off;
      notifyListeners();
    }
  }

  void releaseOneShot() {
    final cleared = _state.entries
        .where((e) => e.value == ModifierMode.oneShot)
        .map((e) => e.key)
        .toList();
    for (final k in cleared) {
      _state[k] = ModifierMode.off;
    }
    if (cleared.isNotEmpty) notifyListeners();
  }
}

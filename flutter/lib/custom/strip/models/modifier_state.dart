import 'package:flutter/foundation.dart';

import '../../input/input_bridge.dart';

enum ModifierMode { off, oneShot, sticky, held }

class ModifierController extends ChangeNotifier {
  final InputBridge bridge;
  final Map<String, ModifierMode> _state = {};

  ModifierController(this.bridge);

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
        bridge.keyDown(name);
      case ModifierMode.oneShot:
        _state[name] = ModifierMode.sticky;
        // key already held down, no additional FFI call needed
      case ModifierMode.sticky:
      case ModifierMode.held:
        _state[name] = ModifierMode.off;
        bridge.keyUp(name);
    }
    notifyListeners();
  }

  void hold(String name) {
    _state[name] = ModifierMode.held;
    bridge.keyDown(name);
    notifyListeners();
  }

  void release(String name) {
    if (modeFor(name) == ModifierMode.held) {
      _state[name] = ModifierMode.off;
      bridge.keyUp(name);
      notifyListeners();
    }
  }

  void releaseOneShot() {
    final toRelease = _state.entries
        .where((e) => e.value == ModifierMode.oneShot)
        .map((e) => e.key)
        .toList();
    for (final k in toRelease) {
      _state[k] = ModifierMode.off;
      bridge.keyUp(k);
    }
    if (toRelease.isNotEmpty) notifyListeners();
  }
}

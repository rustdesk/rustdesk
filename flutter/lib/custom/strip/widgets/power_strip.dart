import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../input/input_bridge.dart';
import '../../theme/tokens.dart';
import '../layouts/default_strip.dart';
import '../models/key_def.dart';
import '../models/modifier_state.dart';
import 'key_cell.dart';

class PowerStrip extends StatelessWidget {
  final InputBridge inputBridge;
  final ModifierController modifierController;
  final VoidCallback onMacrosTap;
  final VoidCallback onKeyboardTap;
  final VoidCallback onDisconnect;
  final bool leftHanded;

  const PowerStrip({
    super.key,
    required this.inputBridge,
    required this.modifierController,
    required this.onMacrosTap,
    required this.onKeyboardTap,
    required this.onDisconnect,
    this.leftHanded = false,
  });

  @override
  Widget build(BuildContext context) {
    final layout =
        leftHanded ? defaultStripLayout.mirrored() : defaultStripLayout;

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceSm,
        vertical: AppTokens.spaceXs,
      ),
      decoration: const BoxDecoration(
        color: AppTokens.colorBgSurface,
        boxShadow: [
          BoxShadow(
            blurRadius: 8,
            color: Colors.black26,
            offset: Offset(0, -2),
          ),
        ],
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: layout.rows.map((row) {
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                ...row.left.map(_wrap),
                const Spacer(),
                ...row.right.map(_wrap),
              ],
            ),
          );
        }).toList(),
      ),
    );
  }

  Widget _wrap(KeyDef k) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 2),
        child: KeyCell(
          keyDef: k,
          modifierController: modifierController,
          onTap: () => _handle(k),
          onPressStart: k.type == KeyType.regular
              ? () => _onRegularPressStart(k)
              : null,
          onPressEnd: k.type == KeyType.regular
              ? () => _onRegularPressEnd(k)
              : null,
        ),
      );

  void _handle(KeyDef k) {
    HapticFeedback.lightImpact();
    switch (k.type) {
      case KeyType.modifier:
        modifierController.cycleTap(k.keyName);
      case KeyType.macroOpener:
        onMacrosTap();
      case KeyType.keyboardToggle:
        onKeyboardTap();
      case KeyType.disconnect:
        onDisconnect();
      case KeyType.regular:
        // Regular keys go through onPressStart / onPressEnd in KeyCell so the
        // held modifier (if any) stays down until the in-flight tap finishes.
        break;
      case KeyType.layer:
        // Fn layer not implemented in v1 — use macros instead
        break;
    }
  }

  // Haptic fires once on touch-down inside _RepeatingKeyButton (not here),
  // so repeat ticks don't buzz on every fire. Held modifiers are passed
  // as flags on the KeyEvent — see ModifierController doc for the why.
  Future<void> _onRegularPressStart(KeyDef k) => inputBridge.tapKey(
        k.keyName,
        modifiers: modifierController.heldModifiers,
      );

  void _onRegularPressEnd(KeyDef k) {
    modifierController.releaseOneShot();
  }
}

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
  final bool leftHanded;

  const PowerStrip({
    super.key,
    required this.inputBridge,
    required this.modifierController,
    required this.onMacrosTap,
    required this.onKeyboardTap,
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
      case KeyType.regular:
        inputBridge.tapKey(k.keyName);
        modifierController.releaseOneShot();
      case KeyType.layer:
        // Fn layer not implemented in v1 — use macros instead
        break;
    }
  }
}

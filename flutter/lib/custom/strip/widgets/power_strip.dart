import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';

import '../../input/input_bridge.dart';
import '../../theme/tokens.dart';
import '../layouts/default_strip.dart';
import '../models/key_def.dart';
import '../models/modifier_state.dart';
import 'key_cell.dart';

class PowerStrip extends StatefulWidget {
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
  State<PowerStrip> createState() => _PowerStripState();
}

class _PowerStripState extends State<PowerStrip> {
  bool _collapsed = false;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: gFFI.ffiModel,
      builder: (context, _) => _buildStrip(gFFI.ffiModel.pi.platform),
    );
  }

  Widget _buildStrip(String platform) {
    final layout = widget.leftHanded
        ? stripLayoutForPlatform(platform).mirrored()
        : stripLayoutForPlatform(platform);

    // When collapsed only the first row (row 0) is shown so the user can
    // still reach the stripToggle key to expand again.
    final visibleRows = _collapsed ? layout.rows.take(1).toList() : layout.rows;

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
        children: visibleRows.map((row) {
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
          modifierController: widget.modifierController,
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
        widget.modifierController.cycleTap(k.keyName);
      case KeyType.macroOpener:
        widget.onMacrosTap();
      case KeyType.keyboardToggle:
        widget.onKeyboardTap();
      case KeyType.stripToggle:
        setState(() => _collapsed = !_collapsed);
      case KeyType.disconnect:
        widget.onDisconnect();
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
  Future<void> _onRegularPressStart(KeyDef k) => widget.inputBridge.tapKey(
        k.keyName,
        modifiers: widget.modifierController.heldModifiers,
      );

  void _onRegularPressEnd(KeyDef k) {
    widget.modifierController.releaseOneShot();
  }
}

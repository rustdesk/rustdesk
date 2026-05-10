import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:get/get.dart';

import '../../input/input_bridge.dart';
import '../../theme/tokens.dart';
import '../layouts/default_strip.dart';
import '../models/key_def.dart';
import '../models/modifier_state.dart';
import 'key_cell.dart';

class PowerStrip extends StatefulWidget {
  final InputBridge inputBridge;
  final ModifierController modifierController;
  final FfiModel ffiModel;
  final VoidCallback onMacrosTap;
  final VoidCallback onKeyboardTap;
  final VoidCallback onDisconnect;
  final VoidCallback onChatToggle;
  final VoidCallback onDisplaySwitch;
  final VoidCallback onZoomFit;
  final VoidCallback onMouseModeToggle;
  final VoidCallback onClipboardPaste;
  final VoidCallback onNextDisplay;
  final bool leftHanded;

  const PowerStrip({
    super.key,
    required this.inputBridge,
    required this.modifierController,
    required this.ffiModel,
    required this.onMacrosTap,
    required this.onKeyboardTap,
    required this.onDisconnect,
    required this.onChatToggle,
    required this.onDisplaySwitch,
    required this.onZoomFit,
    required this.onMouseModeToggle,
    required this.onClipboardPaste,
    required this.onNextDisplay,
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
      listenable: widget.ffiModel,
      builder: (context, _) => _buildStrip(widget.ffiModel.pi.platform),
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
            child: LayoutBuilder(
              builder: (context, constraints) {
                double leftW = row.left.fold(0.0, (s, k) => s + kKeyBaseWidth * k.widthFactor + 4);
                double rightW = row.right.fold(0.0, (s, k) => s + kKeyBaseWidth * k.widthFactor + 4);
                double totalW = leftW + rightW;
                double available = constraints.maxWidth;
                double scale = totalW > available ? available / totalW : 1.0;
                return Row(
                  children: [
                    ...row.left.map((k) => _wrapScaled(k, scale)),
                    const Spacer(),
                    ...row.right.map((k) => _wrapScaled(k, scale)),
                  ],
                );
              },
            ),
          );
        }).toList(),
      ),
    );
  }

  Widget _wrapScaled(KeyDef k, double scale) {
    if (k.type == KeyType.displaySwitch || k.type == KeyType.nextDisplay) {
      return Obx(() {
        if (widget.ffiModel.pi.displays.length <= 1) return const SizedBox.shrink();
        return _keyCell(k, scale);
      });
    }
    return _keyCell(k, scale);
  }

  Widget _keyCell(KeyDef k, double scale) {
    final scaled = scale < 1.0 ? k.copyWith(widthFactor: k.widthFactor * scale) : k;
    return Padding(
      padding: EdgeInsets.symmetric(horizontal: 2 * scale),
      child: KeyCell(
        keyDef: scaled,
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
  }

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
      case KeyType.chatToggle:
        widget.onChatToggle();
      case KeyType.displaySwitch:
        widget.onDisplaySwitch();
      case KeyType.zoomFit:
        widget.onZoomFit();
      case KeyType.mouseModeToggle:
        widget.onMouseModeToggle();
      case KeyType.clipboardPaste:
        widget.onClipboardPaste();
      case KeyType.nextDisplay:
        widget.onNextDisplay();
      case KeyType.typeString:
        if (k.keyString != null) widget.inputBridge.typeString(k.keyString!);
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

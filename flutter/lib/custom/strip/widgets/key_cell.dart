import 'package:flutter/material.dart';

import '../../theme/tokens.dart';
import '../models/key_def.dart';
import '../models/modifier_state.dart';

const _kBaseWidth = 44.0;
const _kHeight = 40.0;

class KeyCell extends StatelessWidget {
  final KeyDef keyDef;
  final ModifierController modifierController;
  final VoidCallback onTap;

  const KeyCell({
    super.key,
    required this.keyDef,
    required this.modifierController,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    if (keyDef.type == KeyType.modifier) {
      return ListenableBuilder(
        listenable: modifierController,
        builder: (ctx, _) {
          final mode = modifierController.modeFor(keyDef.keyName);
          return _KeyButton(
            label: keyDef.label,
            widthFactor: keyDef.widthFactor,
            mode: mode,
            onTap: onTap,
            onLongPressStart: (_) => modifierController.hold(keyDef.keyName),
            onLongPressEnd: (_) => modifierController.release(keyDef.keyName),
          );
        },
      );
    }

    return _KeyButton(
      label: keyDef.label,
      widthFactor: keyDef.widthFactor,
      mode: ModifierMode.off,
      onTap: onTap,
    );
  }
}

class _KeyButton extends StatelessWidget {
  final String label;
  final double widthFactor;
  final ModifierMode mode;
  final VoidCallback onTap;
  final GestureLongPressStartCallback? onLongPressStart;
  final GestureLongPressEndCallback? onLongPressEnd;

  const _KeyButton({
    required this.label,
    required this.widthFactor,
    required this.mode,
    required this.onTap,
    this.onLongPressStart,
    this.onLongPressEnd,
  });

  Color get _bg {
    return switch (mode) {
      ModifierMode.off => AppTokens.colorBgSurface,
      ModifierMode.oneShot =>
        AppTokens.colorPrimary.withValues(alpha: 0.45),
      ModifierMode.sticky || ModifierMode.held => AppTokens.colorPrimary,
    };
  }

  Color get _fg {
    return mode == ModifierMode.off
        ? AppTokens.colorTextHigh
        : Colors.white;
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      onLongPressStart: onLongPressStart,
      onLongPressEnd: onLongPressEnd,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 120),
        width: _kBaseWidth * widthFactor,
        height: _kHeight,
        decoration: BoxDecoration(
          color: _bg,
          borderRadius: BorderRadius.circular(AppTokens.radiusKey),
        ),
        alignment: Alignment.center,
        child: Text(
          label,
          style: AppTokens.fontKey.copyWith(color: _fg, fontSize: 14),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    );
  }
}

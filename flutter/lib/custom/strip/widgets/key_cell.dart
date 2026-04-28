import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../theme/tokens.dart';
import '../models/key_def.dart';
import '../models/modifier_state.dart';

const _kBaseWidth = 44.0;
const _kHeight = 40.0;
const _kRepeatDelay = Duration(milliseconds: 400);
const _kRepeatInterval = Duration(milliseconds: 60);

typedef AsyncCallback = Future<void> Function();

class KeyCell extends StatelessWidget {
  final KeyDef keyDef;
  final ModifierController modifierController;
  final VoidCallback onTap;

  // Used for KeyType.regular — split press lifecycle so the held modifier
  // can stay down until the in-flight tap finishes (avoids a race where the
  // modifier release outpaces the regular key's keyUp on the wire).
  final AsyncCallback? onPressStart;
  final VoidCallback? onPressEnd;

  const KeyCell({
    super.key,
    required this.keyDef,
    required this.modifierController,
    required this.onTap,
    this.onPressStart,
    this.onPressEnd,
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

    if (keyDef.type == KeyType.regular && onPressStart != null) {
      return _RepeatingKeyButton(
        label: keyDef.label,
        widthFactor: keyDef.widthFactor,
        onPressStart: onPressStart!,
        onPressEnd: onPressEnd,
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
      child: _KeyVisual(
        label: label,
        widthFactor: widthFactor,
        bg: _bg,
        fg: _fg,
      ),
    );
  }
}

// Press-down repeats the key after a delay, like a real keyboard.
// onPressStart is called once on touch-down and again every interval; the
// caller awaits it via the stored Future so onPressEnd can wait for the
// in-flight tap to flush before releasing one-shot modifiers.
class _RepeatingKeyButton extends StatefulWidget {
  final String label;
  final double widthFactor;
  final AsyncCallback onPressStart;
  final VoidCallback? onPressEnd;

  const _RepeatingKeyButton({
    required this.label,
    required this.widthFactor,
    required this.onPressStart,
    this.onPressEnd,
  });

  @override
  State<_RepeatingKeyButton> createState() => _RepeatingKeyButtonState();
}

class _RepeatingKeyButtonState extends State<_RepeatingKeyButton> {
  Timer? _delay;
  Timer? _repeat;
  Future<void>? _pending;

  @override
  void dispose() {
    _stopTimers();
    super.dispose();
  }

  void _start() {
    HapticFeedback.lightImpact();
    _pending = widget.onPressStart();
    _delay = Timer(_kRepeatDelay, () {
      _repeat = Timer.periodic(_kRepeatInterval, (_) {
        _pending = widget.onPressStart();
      });
    });
  }

  void _stopTimers() {
    _delay?.cancel();
    _delay = null;
    _repeat?.cancel();
    _repeat = null;
  }

  Future<void> _release() async {
    _stopTimers();
    final pending = _pending;
    if (pending != null) {
      await pending;
    }
    widget.onPressEnd?.call();
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTapDown: (_) => _start(),
      onTapUp: (_) => unawaited(_release()),
      onTapCancel: () => unawaited(_release()),
      child: _KeyVisual(
        label: widget.label,
        widthFactor: widget.widthFactor,
        bg: AppTokens.colorBgSurface,
        fg: AppTokens.colorTextHigh,
      ),
    );
  }
}

class _KeyVisual extends StatelessWidget {
  final String label;
  final double widthFactor;
  final Color bg;
  final Color fg;

  const _KeyVisual({
    required this.label,
    required this.widthFactor,
    required this.bg,
    required this.fg,
  });

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 120),
      width: _kBaseWidth * widthFactor,
      height: _kHeight,
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(AppTokens.radiusKey),
      ),
      alignment: Alignment.center,
      child: Text(
        label,
        style: AppTokens.fontKey.copyWith(color: fg, fontSize: 14),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

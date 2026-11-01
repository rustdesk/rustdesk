import 'package:flutter/material.dart';

/// Local surface tokens for the foldable input deck, following the app theme.
class SessionPalette {
  SessionPalette.of(BuildContext context, {Color? accent})
      : dark = Theme.of(context).brightness == Brightness.dark,
        accent = accent ?? Theme.of(context).colorScheme.secondary;

  final bool dark;
  final Color accent;

  Color get deck => dark ? const Color(0xFF1D2025) : const Color(0xFFE9ECF0);
  Color get key => dark ? const Color(0xFF353A42) : const Color(0xFFFFFFFF);
  Color get utility => dark ? const Color(0xFF292E35) : const Color(0xFFDCE1E7);
  Color get foreground =>
      dark ? const Color(0xFFEDF0F5) : const Color(0xFF222A35);
  Color get secondary =>
      dark ? const Color(0xFFADB7C5) : const Color(0xFF596575);
  Color get line =>
      dark ? Colors.white.withOpacity(.09) : Colors.black.withOpacity(.10);
  Color get pad => dark ? const Color(0xFF242830) : const Color(0xFFF3F5F7);
  Color get shadow => Colors.black.withOpacity(dark ? .30 : .12);
  Color get selected =>
      Color.alphaBlend(accent.withOpacity(dark ? .28 : .13), key);
}

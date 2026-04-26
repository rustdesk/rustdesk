import 'package:flutter/material.dart';

import 'tokens.dart';

class AppTheme {
  AppTheme._();

  static ThemeData get dark {
    final scheme = ColorScheme.fromSeed(
      seedColor: AppTokens.colorPrimary,
      brightness: Brightness.dark,
    ).copyWith(
      surface: AppTokens.colorBgBase,
      surfaceContainerHighest: AppTokens.colorBgSurface,
      onSurface: AppTokens.colorTextHigh,
      onSurfaceVariant: AppTokens.colorTextMid,
    );

    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: scheme,
      scaffoldBackgroundColor: AppTokens.colorBgBase,
      textTheme: const TextTheme(
        headlineSmall: AppTokens.fontTitle,
        bodyMedium: AppTokens.fontBody,
        labelLarge: AppTokens.fontKey,
        labelSmall: AppTokens.fontKeySmall,
      ),
    );
  }
}

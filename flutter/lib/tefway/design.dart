// Tefway Remote design system: tokens and theme.
//
// Every Tefway surface reads its colours, spacing and type scale from here, so
// no widget hard-codes a hex value. Stock RustDesk keeps `MyTheme` untouched.

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

/// Brand and semantic colours. Values follow the Tefway institutional palette
/// and are the only colour literals the Tefway UI is allowed to use.
class TefwayColors {
  TefwayColors._();

  static const brandBlue = Color(0xFF164193);
  static const actionBlue = Color(0xFF2878E5);
  static const deepNavy = Color(0xFF101C30);
  static const surface = Color(0xFF1B2B43);
  static const surfaceElevated = Color(0xFF233B56);
  static const textPrimary = Color(0xFFF5F8FD);
  static const textSecondary = Color(0xFFA9BCD7);
  static const border = Color(0xFF37516F);
  static const positive = Color(0xFF59C99D);
  static const attention = Color(0xFFFFA700);
  static const critical = Color(0xFFE76767);

  /// Filled surface used for inputs and hovering rows.
  static const inputFill = Color(0xFF16253A);
}

/// Spacing scale, in logical pixels.
class TefwaySpacing {
  TefwaySpacing._();

  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 24;
  static const double xxl = 32;
}

/// Corner radii. Discreetly rounded, never pill-shaped.
class TefwayRadius {
  TefwayRadius._();

  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;

  static const BorderRadius smAll = BorderRadius.all(Radius.circular(sm));
  static const BorderRadius mdAll = BorderRadius.all(Radius.circular(md));
  static const BorderRadius lgAll = BorderRadius.all(Radius.circular(lg));
}

/// Type scale. Identification codes use tabular figures so a changing digit
/// never shifts the rest of the string.
class TefwayTextStyles {
  TefwayTextStyles._();

  static const List<FontFeature> _tabular = [FontFeature.tabularFigures()];

  static const TextStyle appTitle = TextStyle(
    fontSize: 18,
    height: 1.2,
    fontWeight: FontWeight.w700,
    letterSpacing: 0.1,
    color: TefwayColors.textPrimary,
  );

  static const TextStyle sectionTitle = TextStyle(
    fontSize: 15,
    height: 1.25,
    fontWeight: FontWeight.w600,
    color: TefwayColors.textPrimary,
  );

  static const TextStyle label = TextStyle(
    fontSize: 12,
    height: 1.3,
    fontWeight: FontWeight.w600,
    letterSpacing: 0.2,
    color: TefwayColors.textSecondary,
  );

  static const TextStyle body = TextStyle(
    fontSize: 13,
    height: 1.45,
    color: TefwayColors.textPrimary,
  );

  static const TextStyle bodySecondary = TextStyle(
    fontSize: 13,
    height: 1.45,
    color: TefwayColors.textSecondary,
  );

  static const TextStyle caption = TextStyle(
    fontSize: 11,
    height: 1.4,
    color: TefwayColors.textSecondary,
  );

  static const TextStyle code = TextStyle(
    fontSize: 26,
    height: 1.15,
    fontWeight: FontWeight.w600,
    letterSpacing: 1.5,
    fontFeatures: _tabular,
    color: TefwayColors.textPrimary,
  );

  static const TextStyle statusLabel = TextStyle(
    fontSize: 12,
    height: 1.3,
    fontWeight: FontWeight.w600,
    fontFeatures: _tabular,
  );
}

/// Theme applied to the whole app when the running build is a Tefway profile.
/// It extends the stock dark theme so every widget that expects
/// `ColorThemeExtension` keeps working, and only re-points the brand colours.
class TefwayTheme {
  TefwayTheme._();

  static ThemeData data() {
    final base = MyTheme.darkTheme;
    final scheme = base.colorScheme.copyWith(
      primary: TefwayColors.actionBlue,
      secondary: TefwayColors.brandBlue,
      surface: TefwayColors.surface,
      onSurface: TefwayColors.textPrimary,
      error: TefwayColors.critical,
    );
    return base.copyWith(
      colorScheme: scheme,
      scaffoldBackgroundColor: TefwayColors.deepNavy,
      canvasColor: TefwayColors.surface,
      dialogTheme: base.dialogTheme.copyWith(
        backgroundColor: TefwayColors.surface,
        shape: const RoundedRectangleBorder(borderRadius: TefwayRadius.lgAll),
      ),
      dividerColor: TefwayColors.border,
      hintColor: TefwayColors.textSecondary,
      hoverColor: TefwayColors.surfaceElevated,
      textTheme: base.textTheme.apply(
        bodyColor: TefwayColors.textPrimary,
        displayColor: TefwayColors.textPrimary,
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: TefwayColors.inputFill,
        isDense: true,
        hintStyle: TefwayTextStyles.bodySecondary,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: TefwaySpacing.md,
          vertical: TefwaySpacing.md,
        ),
        border: OutlineInputBorder(
          borderRadius: TefwayRadius.smAll,
          borderSide: const BorderSide(color: TefwayColors.border, width: 1),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: TefwayRadius.smAll,
          borderSide: const BorderSide(color: TefwayColors.border, width: 1),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: TefwayRadius.smAll,
          borderSide: const BorderSide(color: TefwayColors.actionBlue, width: 1.5),
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: TefwayColors.actionBlue,
          foregroundColor: TefwayColors.textPrimary,
          disabledBackgroundColor: TefwayColors.surfaceElevated,
          disabledForegroundColor: TefwayColors.textSecondary,
          elevation: 0,
          shape: const RoundedRectangleBorder(borderRadius: TefwayRadius.smAll),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          backgroundColor: Colors.transparent,
          foregroundColor: TefwayColors.textPrimary,
          side: const BorderSide(color: TefwayColors.border, width: 1),
          shape: const RoundedRectangleBorder(borderRadius: TefwayRadius.smAll),
        ),
      ),
    );
  }
}

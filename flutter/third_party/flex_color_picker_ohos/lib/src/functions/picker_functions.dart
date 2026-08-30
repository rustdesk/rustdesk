import 'dart:math' as math;

import 'package:flex_seed_scheme/flex_seed_scheme.dart';
import 'package:flutter/material.dart';

import '../models/color_picker_type.dart';

/// These functions are not library exposed, they are private to the library.

/// Privately used extensions on [Color] to lighten a color.
extension FlexPickerColorExtensions on Color {
  /// Lightens the color with the given integer percentage amount.
  /// Defaults to 10%.
  Color lighten([final int amount = 10]) {
    if (amount <= 0) return this;
    if (amount > 100) return Colors.white;
    // HSLColor returns saturation 1 for black, we want 0 instead to be able
    // lighten black color up along the grey scale from black.
    final HSLColor hsl = this == const Color(0xFF000000)
        ? HSLColor.fromColor(this).withSaturation(0)
        : HSLColor.fromColor(this);
    return hsl
        .withLightness(math.min(1, math.max(0, hsl.lightness + amount / 100)))
        .toColor();
  }

  /// Darkens the color with the given integer percentage amount.
  /// Defaults to 10%.
  Color darken([final int amount = 10]) {
    if (amount <= 0) return this;
    if (amount > 100) return Colors.black;
    final HSLColor hsl = HSLColor.fromColor(this);
    return hsl
        .withLightness(math.min(1, math.max(0, hsl.lightness - amount / 100)))
        .toColor();
  }
}

/// Returns the control key label for the current platform.
///
/// Windows, Linux: CTRL
/// Mac: CMD (Tried using the ⌘ symbol, did not show up on Web though.)
/// Others: Empty string
String platformControlKey(TargetPlatform platform, String key) {
  switch (platform) {
    case TargetPlatform.android:
    case TargetPlatform.ohos:
    case TargetPlatform.iOS:
    case TargetPlatform.fuchsia:
      return '';
    case TargetPlatform.linux:
    case TargetPlatform.windows:
      return ' (CTRL-$key)';
    case TargetPlatform.macOS:
      return ' (CMD-$key)';
  }
}

/// Returns true if the platform is a desktop.
bool isDesktop(TargetPlatform platform) {
  switch (platform) {
    case TargetPlatform.android:
    case TargetPlatform.ohos:
    case TargetPlatform.iOS:
    case TargetPlatform.fuchsia:
      return false;
    case TargetPlatform.linux:
    case TargetPlatform.windows:
    case TargetPlatform.macOS:
      return true;
  }
}

/// Locate in which available picker with its color swatches a
/// given color can be found in and return that pickers enum type.
/// This is used to activate the correct Cupertino segment for the provided
/// color, so that it can be selected and shown as selected.
ColorPickerType findColorInSelector({
  required Color color,
  required Map<ColorPickerType, List<ColorSwatch<Object>>> typeToSwatchMap,
  required Map<ColorPickerType, bool> pickersEnabled,
  required bool lookInShades,
  required bool include850,
}) {
  // Search for the given color in any of the swatches that are set
  // as available in the selector and return the swatch where we find
  // the color.
  for (final ColorPickerType key in typeToSwatchMap.keys) {
    if (pickersEnabled[key]!) {
      for (final ColorSwatch<Object> swatch in typeToSwatchMap[key]!) {
        if (lookInShades) {
          if (isShadeOfMain(swatch, color, include850)) return key;
        } else {
          if (swatch.value == color.value) return key;
        }
      }
    }
  }
  // If we did not find the color, we try once more with any opacity.
  for (final ColorPickerType key in typeToSwatchMap.keys) {
    if (pickersEnabled[key]!) {
      for (final ColorSwatch<Object> swatch in typeToSwatchMap[key]!) {
        if (lookInShades) {
          if (isShadeOfMain(swatch, color.withAlpha(0xFF), include850)) {
            return key;
          }
        } else {
          if (swatch.value == color.withAlpha(0xFF).value) return key;
        }
      }
    }
  }
  // If we did not find the color in any of the swatches in the selector, we
  // will just return the first swatch available in the selector.
  for (final ColorPickerType key in typeToSwatchMap.keys) {
    if (pickersEnabled[key]!) return key;
  }
  // And finally if no selector was set to enabled, we return material anyway.
  return ColorPickerType.primary;
}

/// Find and return the ColorSwatch in a List of ColorSwatches that contains
/// a given color.
ColorSwatch<Object?>? findColorSwatch(
    Color color, List<ColorSwatch<Object>> swatches, bool include850) {
  for (final ColorSwatch<Object> mainColor in swatches) {
    if (isShadeOfMain(mainColor, color, include850)) {
      return mainColor;
    }
  }
  return (color is ColorSwatch && swatches.contains(color)) ? color : null;
}

/// Check if a given color is a shade of the main color, return true if it is.
///
/// Used by wheel picker to check if a given color is a member of
/// a standard Material color swatch, including custom made swatches.
/// If it is not, only then will it compute a custom swatch.
bool isShadeOfMain(
  ColorSwatch<Object> mainColor,
  Color shadeColor,
  bool include850,
) {
  for (final Color shade in getMaterialColorShades(mainColor, include850)) {
    if (shade == shadeColor || shade.value == shadeColor.value) return true;
  }
  return false;
}

/// Return a List of colors with all the colors that exist in a given
/// ColorSwatch. Include the 850 index for grey color that has this value,
/// it is the only ColorSwatch that has 850. This function works both
/// for MaterialColor and AccentColor, and for custom color swatches that
/// uses the ColorSwatch indexes below.
List<Color> getMaterialColorShades(ColorSwatch<Object> color, bool include850) {
  return <Color>[
    if (color[50] != null) color[50]!,
    if (color[100] != null) color[100]!,
    if (color[200] != null) color[200]!,
    if (color[300] != null) color[300]!,
    if (color[400] != null) color[400]!,
    if (color[500] != null) color[500]!,
    if (color[600] != null) color[600]!,
    if (color[700] != null) color[700]!,
    if (color[800] != null) color[800]!,
    if (color[850] != null && include850) color[850]!,
    if (color[900] != null) color[900]!,
  ];
}

/// Return the M3 tonal palette for a passed in color as a list of Colors.
List<Color> getTonalColors(Color color, bool fixedMinChroma) {
  final Cam16 camColor = Cam16.fromInt(color.value);
  final FlexTonalPalette tonalColors = FlexTonalPalette.of(camColor.hue,
      fixedMinChroma ? math.max(48, camColor.chroma) : camColor.chroma);

  // ignore: unnecessary_lambdas
  return tonalColors.asList.map((int e) => Color(e)).toList();
}

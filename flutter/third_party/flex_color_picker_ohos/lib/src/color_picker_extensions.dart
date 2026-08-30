import 'package:flutter/material.dart';

/// Extensions on non nullable [Color] to return it's color value as strings.
///
/// The color extension also include getting a color's RGB hex code as a string
/// in two different formats. Extension [hexAlpha] returns a HEX code string of
/// a Color value including alpha channel.
/// The [hex] extension returns the hex color value as RGB string without the
/// alpha channel value.
extension FlexPickerNoNullColorExtensions on Color {
  /// Return color's uppercase RGB hex string, including alpha channel.
  String get hexAlpha {
    return value.toRadixString(16).toUpperCase().padLeft(8, '0');
  }

  /// Return color's uppercase RGB hex string, excluding alpha channel.
  String get hex {
    return value.toRadixString(16).toUpperCase().padLeft(8, '0').substring(2);
  }
}

/// Extensions on [String].
///
/// Included extensions are, [toColor] to convert a String to a Color.
/// To [capitalize] the first letter in a String and [dotTail] to get
/// remaining string after first dot "." in a String.
extension FlexPickerNoNullStringExtensions on String {
  //
  /// Convert a HEX value encoded (A)RGB string to a Dart Color.
  ///
  /// * The string may include the '#' char, but does not have to.
  /// * String may also include '0x' Dart Hex indicator, but does not have to.
  /// * Any '#' '0x' patterns are trimmed out and String is assumed to be Hex.
  /// * The String may start with alpha channel hex value, but does not have to,
  ///   if alpha value is missing "FF" is used for alpha.
  /// * String may be longer than 8 chars, after trimming out # and 0x, it will
  ///   be RIGHT truncated to max 8 chars before parsing.
  /// * If [enableShortRGB] is true a CSS style 3 char RGB value is interpreted
  ///   as RRGGBB, if false it used it as a partial color value.
  ///
  /// IF the resulting string cannot be parsed to a Color, is empty or null
  /// THEN fully opaque black color is returned ELSE the Color is returned.
  ///
  /// To give caller a chance to handle parsing errors, use the same
  /// extension on nullable Color [toColorMaybeNull]. It returns null when
  /// there is a string that cannot be parsed to color value.
  /// You can then decide what to do with the error instead of just receiving
  /// fully opaque black color.
  Color toColorShort(bool enableShortRGB) {
    // If String was zero length, then we return transparent, cannot parse.
    if (this == '') return const Color(0xFF000000);
    // If String length is > 200 we as a safety precaution will not try to
    // parse it to a color, for shorter lengths the last 8 chars will be used.
    if (this.length > 200) return const Color(0xFF000000);
    // Remove all num signs, we allow them, but disregard them all.
    String hexColor = replaceAll('#', '');
    if (hexColor == '') return const Color(0xFF000000);
    // Remove all spaces, we allow them, but disregard them all.
    hexColor = hexColor.replaceAll(' ', '');
    if (hexColor == '') return const Color(0xFF000000);
    // Remove all '0x' Hex code marks, we allow them, but disregard them all.
    hexColor = hexColor.replaceAll('0x', '');
    if (hexColor == '') return const Color(0xFF000000);
    // If the input is exactly 3 chars long, we may have a short Web hex code,
    // let's make the potential 'RGB' code to a 'RRGGBB' code.
    if (hexColor.length == 3 && enableShortRGB) {
      hexColor = hexColor[0] +
          hexColor[0] +
          hexColor[1] +
          hexColor[1] +
          hexColor[2] +
          hexColor[2];
    }
    // Pad anything shorter than 7 with left 0 -> fill non spec channels with 0.
    hexColor = hexColor.padLeft(6, '0');
    // Pad anything shorter than 9 with left F -> fill with opaque alpha.
    hexColor = hexColor.padLeft(8, 'F');

    // We only try to parse the last 8 chars in the remaining string, rest can
    // still be whatever.
    final int length = hexColor.length;
    return Color(int.tryParse('0x${hexColor.substring(length - 8, length)}') ??
        0xFF000000);
  }

  /// Returns [toColorShort] with `enableShortRGB` set to true.
  ///
  /// Available for backwards compatibility with previous API.
  Color get toColor => toColorShort(true);

  /// Capitalize the first letter in a string.
  String get capitalize {
    return (length > 1) ? this[0].toUpperCase() + substring(1) : toUpperCase();
  }

  /// Return the string remaining in a string after the last "." in a String,
  /// if there is no "." the string itself is returned.
  ///
  /// This function can be used to e.g. return the enum tail value from an
  /// enum's standard toString method.
  String get dotTail {
    return split('.').last;
  }
}

/// Extensions on [String].
///
/// Included extensions are, [toColorMaybeNull] to convert a String to a Color.
/// To [capitalizeMaybeNull] the first letter in a String and
/// [dotTailMaybeNull] to get remaining string after first dot "." in a String.
extension FlexPickerNullableStringExtensions on String? {
  //
  /// Convert a HEX value encoded (A)RGB string to a Dart Color.
  ///
  /// * The string may include the '#' char, but does not have to.
  /// * String may also include '0x' Dart Hex indicator, but does not have to.
  /// * Any '#' '0x' patterns are trimmed out and String is assumed to be Hex.
  /// * The String may start with alpha channel hex value, but does not have to,
  ///   if alpha value is missing "FF" is used for alpha.
  /// * String may be longer than 8 chars, after trimming out # and 0x, it will
  ///   be RIGHT truncated to max 8 chars before parsing.
  /// * If [enableShortRGB] is true a CSS style 3 char RGB value is interpreted
  ///   as RRGGBB, if false it used it as a partial color value.
  ///
  /// IF the resulting string cannot be parsed to a Color, is empty or null
  /// THEN NULL is returned ELSE the Color is returned.
  Color? toColorShortMaybeNull(bool enableShortRGB) {
    // If String was null or zero length, then we return null, cannot parse.
    if (this == null || this == '') return null;
    // If String length is > 200 we as a safety precaution will not try to
    // parse it to a color, for shorter lengths the last 8 chars will be used.
    if ((this?.length ?? 200) > 200) return null;
    // Remove all num signs, we allow them, but disregard them all.
    String hexColor = this?.replaceAll('#', '') ?? '';
    if (hexColor == '') return null;
    // Remove all spaces, we allow them, but disregard them all.
    hexColor = hexColor.replaceAll(' ', '');
    if (hexColor == '') return null;
    // Remove all '0x' Hex code marks, we allow them, but disregard them all.
    hexColor = hexColor.replaceAll('0x', '');
    if (hexColor == '') return null;
    // If the input is exactly 3 chars long, we may have a short Web hex code,
    // let's make the potential 'RGB' code to a 'RRGGBB' code.
    if (hexColor.length == 3 && enableShortRGB) {
      hexColor = hexColor[0] +
          hexColor[0] +
          hexColor[1] +
          hexColor[1] +
          hexColor[2] +
          hexColor[2];
    }
    // Pad anything shorter than 7 with left 0 -> fill non spec channels with 0.
    hexColor = hexColor.padLeft(6, '0');
    // Pad anything shorter than 9 with left F -> fill with opaque alpha.
    hexColor = hexColor.padLeft(8, 'F');

    // We only try to parse the last 8 chars in the remaining string, rest can
    // still be whatever.
    final int length = hexColor.length;
    final int? intColor =
        int.tryParse('0x${hexColor.substring(length - 8, length)}');
    return intColor != null ? Color(intColor) : null;
  }

  /// Returns [toColorShortMaybeNull] with `enableShortRGB` set to true.
  ///
  /// Available for backwards compatibility with previous API.
  Color? get toColorMaybeNull => toColorShortMaybeNull(true);

  /// Capitalize the first letter in a string. If string is null, we get null
  /// back.
  String? get capitalizeMaybeNull {
    if (this == null) {
      return this;
    } else {
      return ((this?.length ?? 0) > 1)
          ? (this?[0].toUpperCase() ?? '') + (this?.substring(1) ?? '')
          : this?.toUpperCase() ?? '';
    }
  }

  /// Return the string remaining in a string after the last "." in a String,
  /// if there is no "." the string itself is returned. If string is null
  /// we get null back.
  ///
  /// This function can be used to e.g. return the enum tail value from an
  /// enum's standard toString method.
  String? get dotTailMaybeNull {
    if (this == null) {
      return this;
    } else {
      return this?.split('.').last;
    }
  }
}

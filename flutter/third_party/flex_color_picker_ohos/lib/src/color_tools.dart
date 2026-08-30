import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'functions/picker_functions.dart';

/// Static color tool functions used internally by FlexColorPicker.
///
/// The functions have a public API and can also be used on their own
/// outside FlexColorPicker if so desired. Available functions include:
///
/// * Get English name strings for the Material primary and accent colors.
/// * Maps of color swatches to their names.
/// * Functions to check if a given color belongs to a given color swatch.
/// * Find the swatch a color belongs to.
/// * Create material and accent like swatches of a single color.
/// * Get the color name of a color belonging to a swatch and its swatch index.
/// * A "name that color" function that gives an English name to any color.
///
/// The static color names are not const on purpose, they have default values
/// for their English Material color names. If you need to translate them
/// setup a function that modifies them as needed in your app,
/// something simple like this may be all you need:
///
/// ```
/// void main() {
///   translateColorNames();
///   runApp(const ColorPickerDemo());
/// }
///
/// void translateColorNames() {
/// ColorTools.redName = 'Röd';
/// ColorTools.blueName = 'Blå';
///   :
/// }
/// ```
/// In a translated app you would be using translated strings from your used
/// translation tool, and call it whenever the used language is changed.
class ColorTools {
  /// Private constructor, does not show up in code completion, useful when
  /// there are only static functions and we have nothing to construct.
  ColorTools._(); // coverage:ignore-line

  /// List of all the standard Material primary color swatches.
  ///
  /// A Material primary colors swatch list also exists in the Flutter
  /// SDK in colors.dart as a static const [Colors.primaries],
  /// but that list excludes grey. To make the grey color available to the
  /// color picker, this list includes it as well.
  static const List<ColorSwatch<Object>> primaryColors = <ColorSwatch<Object>>[
    Colors.red,
    Colors.pink,
    Colors.purple,
    Colors.deepPurple,
    Colors.indigo,
    Colors.blue,
    Colors.lightBlue,
    Colors.cyan,
    Colors.teal,
    Colors.green,
    Colors.lightGreen,
    Colors.lime,
    Colors.yellow,
    Colors.amber,
    Colors.orange,
    Colors.deepOrange,
    Colors.brown,
    Colors.blueGrey,
    Colors.grey,
  ];

  // Static string names for all the material primary colors

  /// Name of Material red color. Default value is its English name.
  static String redName = 'Red';

  /// Name of Material pink color. Default value is its English name.
  static String pinkName = 'Pink';

  /// English name for Material purple color. Default value is its English name.
  static String purpleName = 'Purple';

  /// Name of Material deep purple color. Default value is its English name.
  static String deepPurpleName = 'Deep purple';

  /// Name of Material indigo color. Default value is its English name.
  static String indigoName = 'Indigo';

  /// English name for Material blue color. Default value is its English name.
  static String blueName = 'Blue';

  /// Name of Material light blue color. Default value is its English name.
  static String lightBlueName = 'Light blue';

  /// Name of Material cyan color. Default value is its English name.
  static String cyanName = 'Cyan';

  /// Name of Material teal color. Default value is its English name.
  static String tealName = 'Teal';

  /// Name of Material green color. Default value is its English name.
  static String greenName = 'Green';

  /// Name of Material light green color. Default value is its English name.
  static String lightGreenName = 'Light green';

  /// Name of Material lime color. Default value is its English name.
  static String limeName = 'Lime';

  /// Name of Material yellow color. Default value is its English name.
  static String yellowName = 'Yellow';

  /// Name of Material amber color. Default value is its English name.
  static String amberName = 'Amber';

  /// Name of Material orange color. Default value is its English name.
  static String orangeName = 'Orange';

  /// Name of Material deep orange color. Default value is its English name.
  static String deepOrangeName = 'Deep orange';

  /// Name of Material brown color. Default value is its English name.
  static String brownName = 'Brown';

  /// Name of Material blue grey color. Default value is its English name.
  static String blueGreyName = 'Blue-grey';

  /// Name of Material grey color. Default value is its English name.
  static String greyName = 'Grey';

  /// Map of Material primary color swatches and their names.
  ///
  /// Use a primary [ColorSwatch] as key, to get its current name string.
  static Map<ColorSwatch<Object>, String> primaryColorNames =
      <ColorSwatch<Object>, String>{
    Colors.red: redName,
    Colors.pink: pinkName,
    Colors.purple: purpleName,
    Colors.deepPurple: deepPurpleName,
    Colors.indigo: indigoName,
    Colors.blue: blueName,
    Colors.lightBlue: lightBlueName,
    Colors.cyan: cyanName,
    Colors.teal: tealName,
    Colors.green: greenName,
    Colors.lightGreen: lightGreenName,
    Colors.lime: limeName,
    Colors.yellow: yellowName,
    Colors.amber: amberName,
    Colors.orange: orangeName,
    Colors.deepOrange: deepOrangeName,
    Colors.brown: brownName,
    Colors.blueGrey: blueGreyName,
    Colors.grey: greyName,
  };

  /// Frequently used index list for Material primary color handling.
  static const List<int> _indexPrimary = <int>[
    50,
    100,
    200,
    300,
    400,
    500,
    600,
    700,
    800,
    900,
  ];

  /// Frequently used index list for Material primary color handling, with the
  /// [850] index.
  static const List<int> _indexPrimaryWith850 = <int>[
    50,
    100,
    200,
    300,
    400,
    500,
    600,
    700,
    800,
    850,
    900,
  ];

  /// Frequently used index list for Material accent color handling.
  static const List<int> _indexAccent = <int>[100, 200, 400, 700];

  /// Check if the given color is included in any Material primary color swatch.
  ///
  /// Returns true if the color is a Material primary color, otherwise false.
  static bool isPrimaryColor(Color color) {
    for (final ColorSwatch<Object> swatch in primaryColors) {
      for (final int i in _indexPrimaryWith850) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return true; // Color found in a swatch, return true.
        }
      }
    }
    // Color was not in any primary color swatch, return false.
    return false;
  }

  /// Returns true if the [swatch] contains [color].
  ///
  /// Used by the ColorPicker to help reduce rebuilds in certain cases.
  /// But you can also use it as help to determine if a [color] belongs to
  /// a given [ColorSwatch].
  static bool swatchContainsColor(ColorSwatch<Object> swatch, Color color) {
    // We use index list with 850 index included to check all possible indexes
    // in any swatch/ use case that might be used. This covers normal primary,
    // primaries with the 850 index, and via primary also accent index.
    for (final int i in _indexPrimaryWith850) {
      if (swatch[i] == color || swatch[i]?.value == color.value) {
        return true;
      }
    }
    return false;
  }

  /// Returns a Material primary color swatch for the color given to it.
  ///
  /// If the color is a part of a standard material primary color swatch,
  /// then the standard primary color swatch is returned.
  ///
  /// If the color is not a Material standard primary color, create a material
  /// primary swatch for the given color using the given color as the mid 500
  /// index value and return this created custom primary color swatch.
  /// This color swatch can then be used as a primary Material color swatch.
  static MaterialColor primarySwatch(Color color) {
    for (final ColorSwatch<Object> swatch in primaryColors) {
      for (final int i in _indexPrimaryWith850) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return swatch as MaterialColor; // Color found in a swatch, return it.
        }
      }
    }
    // Did not find the given color in a standard Material color swatch, make
    // a custom material primary swatch based on the given color and return it.
    return createPrimarySwatch(color);
  }

  /// Create a primary color Material swatch from a given color value.
  ///
  /// The original version of this swatch calculation is based on this
  /// https://medium.com/@filipvk/creating-a-custom-color-swatch-in-flutter-554bcdcb27f3
  /// by Filip Velickovic https://filipvk.bitbucket.io/
  /// It has been slightly modified.
  ///
  /// The provided color value is used as the Material swatch default color 500
  /// in the returned swatch, with lighter hues for lower indexes and darker
  /// shades for higher index values.
  ///
  /// If you give this function a standard Material color index 500 value,
  /// eg `Colors.red[500]` it will not return the same swatch as `Colors.red`.
  /// This function is an approximation and gives an automated way of creating
  /// a material like primary swatch.
  ///
  /// The official Material colors contain hand picked references colors.
  /// To get an official Material swatch from a color use [primarySwatch] that
  /// returns the real Material swatch first for a color, if it is a standard
  /// Material primary color, and then creates a Material like swatch with this
  /// function, if it was not a standard material primary color hue.
  ///
  /// There are reversed engineered JS versions of the official Material Color
  /// algorithm made from the Material Guide web tools. If anybody has the
  /// energy to make a Dart version of it, that would be fabulous.
  /// SO discussion here:
  /// https://stackoverflow.com/questions/32942503/material-design-color-palette
  ///
  /// Starting points here:
  /// - https://github.com/mbitson/mcg/issues/19
  /// - Best one: https://github.com/eugeneford/material-palette-generator
  /// - https://github.com/edelstone/material-palette-generator
  ///
  /// The official M2 guide it should match:
  /// https://material.io/design/color/the-color-system.html#tools-for-picking-colors
  /// And its color tool where the JS reverse engineered version is from:
  /// https://material.io/resources/color/#!/?view.left=0&view.right=0&primary.color=6002ee
  static MaterialColor createPrimarySwatch(Color color) {
    final Map<int, Color> swatch = <int, Color>{};
    final int a = color.alpha;
    final int r = color.red;
    final int g = color.green;
    final int b = color.blue;
    for (final int strength in _indexPrimary) {
      final double ds = 0.5 - strength / 1000;
      swatch[strength] = Color.fromARGB(
        a,
        r + ((ds < 0 ? r : (255 - r)) * ds).round(),
        g + ((ds < 0 ? g : (255 - g)) * ds).round(),
        b + ((ds < 0 ? b : (255 - b)) * ds).round(),
      );
    }
    // The above gives a starting point, this tunes it a bit better, still far
    // from the real algorithm.
    swatch[50] = swatch[50]!.lighten(18);
    swatch[100] = swatch[100]!.lighten(16);
    swatch[200] = swatch[200]!.lighten(14);
    swatch[300] = swatch[300]!.lighten(10);
    swatch[400] = swatch[400]!.lighten(6);
    swatch[700] = swatch[700]!.darken(2);
    swatch[800] = swatch[800]!.darken(3);
    swatch[900] = swatch[900]!.darken(4);
    return MaterialColor(color.value, swatch);
  }

  /// List of all the standard Material accent color swatches.
  ///
  /// A Material accents colors swatch list also exists in the Flutter
  /// SDK in colors.dart as a static const [Colors.accents].
  static const List<ColorSwatch<Object>> accentColors = <ColorSwatch<Object>>[
    Colors.redAccent,
    Colors.pinkAccent,
    Colors.purpleAccent,
    Colors.deepPurpleAccent,
    Colors.indigoAccent,
    Colors.blueAccent,
    Colors.lightBlueAccent,
    Colors.cyanAccent,
    Colors.tealAccent,
    Colors.greenAccent,
    Colors.lightGreenAccent,
    Colors.limeAccent,
    Colors.yellowAccent,
    Colors.amberAccent,
    Colors.orangeAccent,
    Colors.deepOrangeAccent,
  ];

  /// Name of Material red accent color. Default value is its English name.
  static String redAccentName = 'Red accent';

  /// Name of Material pink accent color. Default value is its English name.
  static String pinkAccentName = 'Pink accent';

  /// Name of Material purple accent color. Default value is its English name.
  static String purpleAccentName = 'Purple accent';

  /// Name of Material deep purple accent color.
  /// Default value is its English name.
  static String deepPurpleAccentName = 'Deep purple accent';

  /// Name of Material indigo accent color. Default value is its English name.
  static String indigoAccentName = 'Indigo accent';

  /// Name of Material blue accent color. Default value is its English name.
  static String blueAccentName = 'Blue accent';

  /// Name of Material light blue accent color.
  /// Default value is its English name.
  static String lightBlueAccentName = 'Light blue accent';

  /// Name of Material cyan accent color. Default value is its English name.
  static String cyanAccentName = 'Cyan accent';

  /// Name of Material teal accent color. Default value is its English name.
  static String tealAccentName = 'Teal accent';

  /// Name of Material green accent color. Default value is its English name.
  static String greenAccentName = 'Green accent';

  /// Name of Material light green accent color.
  /// Default value is its English name.
  static String lightGreenAccentName = 'Light green accent';

  /// Name of Material lime accent color. Default value is its English name.
  static String limeAccentName = 'Lime accent';

  /// Name of Material yellow accent color. Default value is its English name.
  static String yellowAccentName = 'Yellow accent';

  /// Name of Material amber accent color. Default value is its English name.
  static String amberAccentName = 'Amber accent';

  /// Name of Material orange accent color. Default value is its English name.
  static String orangeAccentName = 'Orange accent';

  /// Name of Material deep orange accent color.
  /// Default value is its English name.
  static String deepOrangeAccentName = 'Deep orange accent';

  /// Map of Material accent color swatches and their names.
  ///
  /// Use a primary [ColorSwatch] as key to get its current name string.
  static Map<ColorSwatch<Object>, String> accentColorsNames =
      <ColorSwatch<Object>, String>{
    Colors.redAccent: redAccentName,
    Colors.pinkAccent: pinkAccentName,
    Colors.purpleAccent: purpleAccentName,
    Colors.deepPurpleAccent: deepPurpleAccentName,
    Colors.indigoAccent: indigoAccentName,
    Colors.blueAccent: blueAccentName,
    Colors.lightBlueAccent: lightBlueAccentName,
    Colors.cyanAccent: cyanAccentName,
    Colors.tealAccent: tealAccentName,
    Colors.greenAccent: greenAccentName,
    Colors.lightGreenAccent: lightGreenAccentName,
    Colors.limeAccent: limeAccentName,
    Colors.yellowAccent: yellowAccentName,
    Colors.amberAccent: amberAccentName,
    Colors.orangeAccent: orangeAccentName,
    Colors.deepOrangeAccent: deepOrangeAccentName,
  };

  /// Check if the given color is included in any Material accent color swatch.
  ///
  /// Returns true if the color is a Material accent color, otherwise false.
  static bool isAccentColor(Color color) {
    for (final ColorSwatch<Object> swatch in accentColors) {
      for (final int i in _indexAccent) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return true; // Color found in a swatch, return true.
        }
      }
    }
    // Color was not in any standard Material accent color swatch, return false.
    return false;
  }

  /// Returns a Material accent color swatch for the color given to it.
  ///
  /// If the color is a part of a standard material accent color swatch,
  /// then the standard accent color swatch will be returned.
  ///
  /// If the color is not a Material standard accent color, creates a
  /// material accent swatch for the given color using the given color
  /// as [200] index value and return this created custom color accent swatch.
  /// This color swatch can then be used as a accent Material color swatch.
  static MaterialAccentColor accentSwatch(Color color) {
    for (final ColorSwatch<Object> swatch in accentColors) {
      for (final int i in _indexAccent) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return swatch as MaterialAccentColor; // Found in a swatch, return it.
        }
      }
    }
    // Did not find the given color in a standard Material accent swatch, make
    // a custom material accent swatch based on the given color and return it.
    return createAccentSwatch(color);
  }

  // This is an accentSwatch version made based on this gem:
  // https://medium.com/@filipvk/creating-a-custom-color-swatch-in-flutter-554bcdcb27f3
  // Which was originally made by Filip Velickovic https://filipvk.bitbucket.io/

  /// Create an Accent color swatch from a given single color value.
  ///
  /// The provided color is used as the Accent swatch default color [200] in the
  /// returned swatch with lighter hues for lower index and darker shades
  /// for higher indexes.
  static MaterialAccentColor createAccentSwatch(Color color) {
    final Map<int, Color> swatch = <int, Color>{};
    final int a = color.alpha;
    final int r = color.red;
    final int g = color.green;
    final int b = color.blue;
    for (final int strength in _indexAccent) {
      final double ds = 0.2 - strength / 1000;
      swatch[strength] = Color.fromARGB(
          a,
          r + ((ds < 0 ? r : (255 - r)) * ds).round(),
          g + ((ds < 0 ? g : (255 - g)) * ds).round(),
          b + ((ds < 0 ? b : (255 - b)) * ds).round());
    }
    // The above gives a starting point, this tunes it a bit better, still far
    // from the real algorithm.
    swatch[100] = swatch[100]!.lighten(14);
    swatch[700] = swatch[700]!.lighten(2);
    return MaterialAccentColor(color.value, swatch);
  }

  /// A list with both primary and accent color Material color swatches.
  ///
  /// This list is used to create a color picker that mixes and includes both
  /// the primary material colors and the accent colors in the same picker.
  /// The related colors are grouped after each other so that they come in
  /// related color order, not in primary and accent order.
  static const List<ColorSwatch<Object>> primaryAndAccentColors =
      <ColorSwatch<Object>>[
    Colors.red,
    Colors.redAccent,
    Colors.pink,
    Colors.pinkAccent,
    Colors.purple,
    Colors.purpleAccent,
    Colors.deepPurple,
    Colors.deepPurpleAccent,
    Colors.indigo,
    Colors.indigoAccent,
    Colors.blue,
    Colors.blueAccent,
    Colors.lightBlue,
    Colors.lightBlueAccent,
    Colors.cyan,
    Colors.cyanAccent,
    Colors.teal,
    Colors.tealAccent,
    Colors.green,
    Colors.greenAccent,
    Colors.lightGreen,
    Colors.lightGreenAccent,
    Colors.lime,
    Colors.limeAccent,
    Colors.yellow,
    Colors.yellowAccent,
    Colors.amber,
    Colors.amberAccent,
    Colors.orange,
    Colors.orangeAccent,
    Colors.deepOrange,
    Colors.deepOrangeAccent,
    Colors.brown,
    Colors.blueGrey,
    Colors.grey,
  ];

  /// A color swatch for almost black colors, ending in black.
  ///
  /// These are none transparent shades of close to black values, useful when
  /// you want slightly off black values that are not transparent.
  static const ColorSwatch<Object> blackShade = ColorSwatch<Object>(
    0xFF0A0A0A,
    <int, Color>{
      50: Color(0xFF121212),
      100: Color(0xFF111111),
      200: Color(0xFF101010),
      300: Color(0xFF0E0E0E),
      400: Color(0xFF0C0C0C),
      500: Color(0xFF0A0A0A),
      600: Color(0xFF080808),
      700: Color(0xFF050505),
      800: Color(0xFF030303),
      900: Color(0xFF000000),
    },
  );

  /// A color swatch for almost white colors, starting with white.
  ///
  /// These are none transparent shades of close to white values, useful when
  /// you want slightly off white values that are not transparent.
  static const ColorSwatch<Object> whiteShade = ColorSwatch<Object>(
    0xFFFAFAFB,
    <int, Color>{
      50: Color(0xFFFFFFFF),
      100: Color(0xFFFEFEFE),
      200: Color(0xFFFDFDFD),
      300: Color(0xFFFCFCFC),
      400: Color(0xFFFBFBFB),
      // Last digit B on purpose to make value different from Grey[50].
      500: Color(0xFFFAFAFB),
      600: Color(0xFFF9F9F9),
      700: Color(0xFFF8F8F8),
      800: Color(0xFFF7F7F7),
      900: Color(0xFFF6F6F6),
    },
  );

  /// The [blackShade] and [whiteShade] in a color swatch list.
  static const List<ColorSwatch<Object>> blackAndWhite = <ColorSwatch<Object>>[
    blackShade,
    whiteShade,
  ];

  /// Name of black and near black color swatch.
  /// Default value is its English name.
  static String blackShadeName = 'Black';

  /// Name of white and near white color swatch.
  /// Default value is its English name.
  static String whiteShadeName = 'White';

  /// Map of black and white swatches, with their near black and white colors
  /// to the black and white swatch names.
  ///
  /// Use [blackShade] or [whiteShade] swatch as key to get its current
  /// name string.
  static Map<ColorSwatch<Object>, String> blackAndWhiteNames =
      <ColorSwatch<Object>, String>{
    blackShade: blackShadeName,
    whiteShade: whiteShadeName,
  };

  /// Check if a color is included in the custom black and white swatches.
  ///
  /// Returns true if the color is a black or white swatch, otherwise false.
  static bool isBlackAndWhiteColor(Color color) {
    for (final ColorSwatch<Object> swatch in blackAndWhite) {
      for (final int i in _indexPrimary) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return true; // Color found in a swatch, return true.
        }
      }
    }
    // Did not find the given color in any black or white swatch, return false
    return false;
  }

  /// Returns a black or white color swatch for the color given to it.
  ///
  /// If the color is a part of a black and white color swatch,
  /// then the Black or White color swatch will be returned.
  /// If the color is not in these swatches, it will create a
  /// primary material swatch for the given color using the given color as
  /// the mid 500 value and return this created custom primary color Swatch.
  /// This color swatch can then be used as a primary Material color swatch.
  static ColorSwatch<Object> blackAndWhiteSwatch(Color color) {
    for (final ColorSwatch<Object> swatch in blackAndWhite) {
      for (final int i in _indexPrimary) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          return swatch; // Color found in a swatch, return it.
        }
      }
    }
    // Did not find the given color in a standard Material color swatch,
    // so make a custom material swatch based on the given color and return it.
    return createPrimarySwatch(color);
  }

  /// Check if a color is included in a custom color swatches.
  ///
  /// Returns true if the color is a custom swatch, otherwise false.
  static bool isCustomColor(
      Color color, Map<ColorSwatch<Object>, String>? customSwatch) {
    if (customSwatch != null) {
      for (final ColorSwatch<Object> swatch in customSwatch.keys) {
        for (final int i in _indexPrimary) {
          if (swatch[i] == color || swatch[i]?.value == color.value) {
            return true; // Color found in a swatch, return true.
          }
        }
      }
    }
    // Did not find the given color in a custom swatch, return false.
    return false;
  }

  /// Returns the custom color swatch for the color given to it.
  ///
  /// If the color is a part of a custom color swatch,
  /// then the custom color swatch will be returned.
  /// If the color is not in these swatches, it will create a
  /// primary material swatch for the given color using the given color
  /// as the mid 500 value and return this created custom primary color Swatch.
  /// This color swatch can then be used as a primary Material color swatch.
  static ColorSwatch<Object> customSwatch(
      Color color, Map<ColorSwatch<Object>, String>? customSwatch) {
    if (customSwatch != null) {
      for (final ColorSwatch<Object> swatch in customSwatch.keys) {
        for (final int i in _indexPrimary) {
          if (swatch[i] == color || swatch[i]?.value == color.value) {
            return swatch; // Color found in a swatch so we return it
          }
        }
      }
    }
    // Did not find the given color in the custom color swatch,
    // so make a custom material swatch based on the given color and return it.
    return createPrimarySwatch(color);
  }

  /// Returns the official Material color name for the color passed to it,
  /// including the shade index and Flutter style HexCode.
  ///
  /// If it is not a Material color or one of the Accents colors, only the
  /// Flutter style Hex code is returned.
  static String materialNameAndCode(Color color,
      {Map<ColorSwatch<Object>, String>? colorSwatchNameMap}) {
    final String name =
        materialName(color, colorSwatchNameMap: colorSwatchNameMap);
    if (name == '') {
      // This is not a material color, we just return it's Flutter like HEX code
      return '(0x${colorCode(color)})';
    } else {
      return '$name (0x${colorCode(color)})';
    }
  }

  /// Returns the official Material color name for the color passed to it,
  /// including the shade index and ARGB style HexCode.
  ///
  /// If it is not a Material color or one of the Accents colors, only the
  /// ARGB style Hex code is returned.
  static String materialNameAndARGBCode(Color color,
      {Map<ColorSwatch<Object>, String>? colorSwatchNameMap}) {
    final String name =
        materialName(color, colorSwatchNameMap: colorSwatchNameMap);
    if (name == '') {
      // This is not a material color, we just return it's Flutter like HEX code
      return '(${colorCode(color)})';
    } else {
      return '$name (${colorCode(color)})';
    }
  }

  /// Returns the Material swatch name or custom color swatch name for a
  /// given color.
  ///
  /// The name will include the color index if the flag [withIndex] is true.
  /// If the given color is not a material color or one of the accents colors,
  /// an empty string is returned. The function can also take as input an
  /// optional custom color swatch to name map and return a custom name for any
  /// color found in any of the custom color swatches in the map.
  static String materialName(Color color,
      {Map<ColorSwatch<Object>, String>? colorSwatchNameMap,
      bool withIndex = true}) {
    // If it is a black or white shade, return name, shade and optional index.
    for (final ColorSwatch<Object> swatch in blackAndWhiteNames.keys) {
      for (final int i in _indexPrimary) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          if (withIndex) {
            return '${blackAndWhiteNames[swatch]} [$i]';
          } else {
            return blackAndWhiteNames[swatch]!;
          }
        }
      }
    }
    // If it is a primary color, return name, shade and and optional index.
    for (final ColorSwatch<Object> swatch in primaryColorNames.keys) {
      for (final int i in _indexPrimaryWith850) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          if (withIndex) {
            return '${primaryColorNames[swatch]} [$i]';
          } else {
            return primaryColorNames[swatch]!;
          }
        }
      }
    }
    // If it is an accent color, return name, shade and optional index.
    // index = <int>[100, 200, 400, 700];
    for (final ColorSwatch<Object> swatch in accentColorsNames.keys) {
      for (final int i in _indexAccent) {
        if (swatch[i] == color || swatch[i]?.value == color.value) {
          if (withIndex) {
            return '${accentColorsNames[swatch]} [$i]';
          } else {
            return accentColorsNames[swatch]!;
          }
        }
      }
    }
    // If we have a custom color and name map passed, we will check if the
    // color exists in it as well and what name it has in the map and
    // return name, shade and optional index.
    if (colorSwatchNameMap != null) {
      for (final ColorSwatch<Object> swatch in colorSwatchNameMap.keys) {
        for (final int i in _indexPrimary) {
          if (swatch[i] == color || swatch[i]?.value == color.value) {
            if (withIndex) {
              return '${colorSwatchNameMap[swatch]} [$i]';
            } else {
              return colorSwatchNameMap[swatch]!;
            }
          }
        }
      }
    }
    // If all the above did not yield a name, it has no defined name,
    // return an empty string.
    return '';
  }

  /// Return the color value as a HexCode string in uppercase.
  static String colorCode(Color color) {
    return color.value.toRadixString(16).toUpperCase();
  }

  /// Returns a String name of the color passed to it.
  ///
  /// The `nameThatColor` function uses a list of 1566 color codes and their
  /// names. It finds the color that closest matches the given color in the list
  /// and returns its color name.
  ///
  /// Use this function to name used and selected colors in e.g. a color picker
  /// or name random colors or colors given as input by users.
  ///
  /// The returned color name is based on a Dart port of a JavaScript tool
  /// called 'ntc', short for "Name That Color". The javascript project and info
  /// about it can be found here http://chir.ag/projects/ntc.
  static String nameThatColor(Color color) =>
      _ColorName.fromColor(color).getName;
}

// Private class for storing and getting the name of a given color.
// Use a color or string color code in hex format and get the closest color
// name for it, from a list of colors that contains 1566 color names.
//
// This is a Dart port of http://chir.ag/projects/ntc.
// This is also a modified Dart port of the one found in pub.dev package
// https://pub.dev/packages/random_color by Luka Knezic. Credits and rights
// for the original Dart port belongs to Luka Knezic.
// This version uses const Color values for the named color codes, instead of
// strings parsed to colors. More efficient and we can also see the color of
// each named color code in the IDE, which is a nice feature during development.
// Some color names with obvious spelling mistakes were also corrected.
class _ColorName {
  /// Default const constructor for a ColorName, a color with a Name.
  const _ColorName(this._color, this._name);

  // Error handling show problematic color codes by returning white color.
  factory _ColorName.fromColor(Color color) {
    final String decodeColor = color.value.toRadixString(16);
    final int r = color.red;
    final int g = color.green;
    final int b = color.blue;

    final HSLColor hsl = HSLColor.fromColor(color);
    final int h = hsl.hue.toInt();
    final int s = (hsl.saturation * 100).toInt();
    final int l = (hsl.lightness * 100).toInt();

    int ndf1;
    int ndf2;
    int ndf;
    int df = -1;
    int cl = -1;
    for (int i = 0; i < colorNames.length; i++) {
      if (decodeColor == colorNames[i].getCode) return colorNames[i];

      ndf1 = math.pow(r - colorNames[i].getRed, 2).toInt() +
          math.pow(g - colorNames[i].getGreen, 2).toInt() +
          math.pow(b - colorNames[i].getBlue, 2).toInt();
      ndf2 = math.pow(h - colorNames[i].getHue, 2).toInt() +
          math.pow(s - colorNames[i].getSaturation, 2).toInt() +
          math.pow(l - colorNames[i].getLightness, 2).toInt();
      ndf = ndf1 + ndf2 * 2;
      if (df < 0 || df > ndf) {
        df = ndf;
        cl = i;
      }
    }
    return cl < 0
        ? _ColorName(Colors.white, 'Color [$decodeColor] not found!')
        : colorNames[cl];
  }

  final Color _color;
  final String _name;

  HSLColor get _hslColor => HSLColor.fromColor(_color);
  int get getBlue => _color.blue;
  String get getCode => _color.value.toRadixString(16);
  int get getGreen => _color.green;
  int get getHue => _hslColor.hue.toInt();
  int get getLightness => (_hslColor.lightness * 100).toInt();
  String get getName => _name;
  int get getRed => _color.red;
  int get getSaturation => (_hslColor.saturation * 100).toInt();

  // A const list of 1566 named colors
  static const List<_ColorName> colorNames = <_ColorName>[
    _ColorName(Color(0xFF000000), 'Black'),
    _ColorName(Color(0xFF000080), 'Navy Blue'),
    _ColorName(Color(0xFF0000C8), 'Dark Blue'),
    _ColorName(Color(0xFF0000FF), 'Blue'),
    _ColorName(Color(0xFF000741), 'Stratos'),
    _ColorName(Color(0xFF001B1C), 'Swamp'),
    _ColorName(Color(0xFF002387), 'Resolution Blue'),
    _ColorName(Color(0xFF002900), 'Deep Fir'),
    _ColorName(Color(0xFF002E20), 'Burnham'),
    _ColorName(Color(0xFF002FA7), 'Klein Blue'),
    _ColorName(Color(0xFF003153), 'Prussian Blue'),
    _ColorName(Color(0xFF003366), 'Midnight Blue'),
    _ColorName(Color(0xFF003399), 'Smalt'),
    _ColorName(Color(0xFF003532), 'Deep Teal'),
    _ColorName(Color(0xFF003E40), 'Cyprus'),
    _ColorName(Color(0xFF004620), 'Kaitoke Green'),
    _ColorName(Color(0xFF0047AB), 'Cobalt'),
    _ColorName(Color(0xFF004816), 'Crusoe'),
    _ColorName(Color(0xFF004950), 'Sherpa Blue'),
    _ColorName(Color(0xFF0056A7), 'Endeavour'),
    _ColorName(Color(0xFF00581A), 'Camarone'),
    _ColorName(Color(0xFF0066CC), 'Science Blue'),
    _ColorName(Color(0xFF0066FF), 'Blue Ribbon'),
    _ColorName(Color(0xFF00755E), 'Tropical Rain Forest'),
    _ColorName(Color(0xFF0076A3), 'Allports'),
    _ColorName(Color(0xFF007BA7), 'Deep Cerulean'),
    _ColorName(Color(0xFF007EC7), 'Lochmara'),
    _ColorName(Color(0xFF007FFF), 'Azure Radiance'),
    _ColorName(Color(0xFF008080), 'Teal'),
    _ColorName(Color(0xFF0095B6), 'Bondi Blue'),
    _ColorName(Color(0xFF009DC4), 'Pacific Blue'),
    _ColorName(Color(0xFF00A693), 'Persian Green'),
    _ColorName(Color(0xFF00A86B), 'Jade'),
    _ColorName(Color(0xFF00CC99), 'Caribbean Green'),
    _ColorName(Color(0xFF00CCCC), "Robin's Egg Blue"),
    _ColorName(Color(0xFF00FF00), 'Green'),
    _ColorName(Color(0xFF00FF7F), 'Spring Green'),
    _ColorName(Color(0xFF00FFFF), 'Cyan Aqua'),
    _ColorName(Color(0xFF010D1A), 'Blue Charcoal'),
    _ColorName(Color(0xFF011635), 'Midnight'),
    _ColorName(Color(0xFF011D13), 'Holly'),
    _ColorName(Color(0xFF012731), 'Daintree'),
    _ColorName(Color(0xFF01361C), 'Cardin Green'),
    _ColorName(Color(0xFF01371A), 'County Green'),
    _ColorName(Color(0xFF013E62), 'Astronaut Blue'),
    _ColorName(Color(0xFF013F6A), 'Regal Blue'),
    _ColorName(Color(0xFF014B43), 'Aqua Deep'),
    _ColorName(Color(0xFF015E85), 'Orient'),
    _ColorName(Color(0xFF016162), 'Blue Stone'),
    _ColorName(Color(0xFF016D39), 'Fun Green'),
    _ColorName(Color(0xFF01796F), 'Pine Green'),
    _ColorName(Color(0xFF017987), 'Blue Lagoon'),
    _ColorName(Color(0xFF01826B), 'Deep Sea'),
    _ColorName(Color(0xFF01A368), 'Green Haze'),
    _ColorName(Color(0xFF022D15), 'English Holly'),
    _ColorName(Color(0xFF02402C), 'Sherwood Green'),
    _ColorName(Color(0xFF02478E), 'Congress Blue'),
    _ColorName(Color(0xFF024E46), 'Evening Sea'),
    _ColorName(Color(0xFF026395), 'Bahama Blue'),
    _ColorName(Color(0xFF02866F), 'Observatory'),
    _ColorName(Color(0xFF02A4D3), 'Cerulean'),
    _ColorName(Color(0xFF03163C), 'Tangaroa'),
    _ColorName(Color(0xFF032B52), 'Green Vogue'),
    _ColorName(Color(0xFF036A6E), 'Mosque'),
    _ColorName(Color(0xFF041004), 'Midnight Moss'),
    _ColorName(Color(0xFF041322), 'Black Pearl'),
    _ColorName(Color(0xFF042E4C), 'Blue Whale'),
    _ColorName(Color(0xFF044022), 'Zuccini'),
    _ColorName(Color(0xFF044259), 'Teal Blue'),
    _ColorName(Color(0xFF051040), 'Deep Cove'),
    _ColorName(Color(0xFF051657), 'Gulf Blue'),
    _ColorName(Color(0xFF055989), 'Venice Blue'),
    _ColorName(Color(0xFF056F57), 'Watercourse'),
    _ColorName(Color(0xFF062A78), 'Catalina Blue'),
    _ColorName(Color(0xFF063537), 'Tiber'),
    _ColorName(Color(0xFF069B81), 'Gossamer'),
    _ColorName(Color(0xFF06A189), 'Niagara'),
    _ColorName(Color(0xFF073A50), 'Tarawera'),
    _ColorName(Color(0xFF080110), 'Jaguar'),
    _ColorName(Color(0xFF081910), 'Black Bean'),
    _ColorName(Color(0xFF082567), 'Deep Sapphire'),
    _ColorName(Color(0xFF088370), 'Elf Green'),
    _ColorName(Color(0xFF08E8DE), 'Bright Turquoise'),
    _ColorName(Color(0xFF092256), 'Downriver'),
    _ColorName(Color(0xFF09230F), 'Palm Green'),
    _ColorName(Color(0xFF09255D), 'Madison'),
    _ColorName(Color(0xFF093624), 'Bottle Green'),
    _ColorName(Color(0xFF095859), 'Deep Sea Green'),
    _ColorName(Color(0xFF097F4B), 'Salem'),
    _ColorName(Color(0xFF0A001C), 'Black Russian'),
    _ColorName(Color(0xFF0A480D), 'Dark Fern'),
    _ColorName(Color(0xFF0A6906), 'Japanese Laurel'),
    _ColorName(Color(0xFF0A6F75), 'Atoll'),
    _ColorName(Color(0xFF0B0B0B), 'Cod Gray'),
    _ColorName(Color(0xFF0B0F08), 'Marshland'),
    _ColorName(Color(0xFF0B1107), 'Gordons Green'),
    _ColorName(Color(0xFF0B1304), 'Black Forest'),
    _ColorName(Color(0xFF0B6207), 'San Felix'),
    _ColorName(Color(0xFF0BDA51), 'Malachite'),
    _ColorName(Color(0xFF0C0B1D), 'Ebony'),
    _ColorName(Color(0xFF0C0D0F), 'Woodsmoke'),
    _ColorName(Color(0xFF0C1911), 'Racing Green'),
    _ColorName(Color(0xFF0C7A79), 'Surfie Green'),
    _ColorName(Color(0xFF0C8990), 'Blue Chill'),
    _ColorName(Color(0xFF0D0332), 'Black Rock'),
    _ColorName(Color(0xFF0D1117), 'Bunker'),
    _ColorName(Color(0xFF0D1C19), 'Aztec'),
    _ColorName(Color(0xFF0D2E1C), 'Bush'),
    _ColorName(Color(0xFF0E0E18), 'Cinder'),
    _ColorName(Color(0xFF0E2A30), 'Firefly'),
    _ColorName(Color(0xFF0F2D9E), 'Torea Bay'),
    _ColorName(Color(0xFF10121D), 'Vulcan'),
    _ColorName(Color(0xFF101405), 'Green Waterloo'),
    _ColorName(Color(0xFF105852), 'Eden'),
    _ColorName(Color(0xFF110C6C), 'Arapawa'),
    _ColorName(Color(0xFF120A8F), 'Ultramarine'),
    _ColorName(Color(0xFF123447), 'Elephant'),
    _ColorName(Color(0xFF126B40), 'Jewel'),
    _ColorName(Color(0xFF130000), 'Diesel'),
    _ColorName(Color(0xFF130A06), 'Asphalt'),
    _ColorName(Color(0xFF13264D), 'Blue Zodiac'),
    _ColorName(Color(0xFF134F19), 'Parsley'),
    _ColorName(Color(0xFF140600), 'Nero'),
    _ColorName(Color(0xFF1450AA), 'Tory Blue'),
    _ColorName(Color(0xFF151F4C), 'Bunting'),
    _ColorName(Color(0xFF1560BD), 'Denim'),
    _ColorName(Color(0xFF15736B), 'Genoa'),
    _ColorName(Color(0xFF161928), 'Mirage'),
    _ColorName(Color(0xFF161D10), 'Hunter Green'),
    _ColorName(Color(0xFF162A40), 'Big Stone'),
    _ColorName(Color(0xFF163222), 'Celtic'),
    _ColorName(Color(0xFF16322C), 'Timber Green'),
    _ColorName(Color(0xFF163531), 'Gable Green'),
    _ColorName(Color(0xFF171F04), 'Pine Tree'),
    _ColorName(Color(0xFF175579), 'Chathams Blue'),
    _ColorName(Color(0xFF182D09), 'Deep Forest Green'),
    _ColorName(Color(0xFF18587A), 'Blumine'),
    _ColorName(Color(0xFF19330E), 'Palm Leaf'),
    _ColorName(Color(0xFF193751), 'Nile Blue'),
    _ColorName(Color(0xFF1959A8), 'Fun Blue'),
    _ColorName(Color(0xFF1A1A68), 'Lucky Point'),
    _ColorName(Color(0xFF1AB385), 'Mountain Meadow'),
    _ColorName(Color(0xFF1B0245), 'Tolopea'),
    _ColorName(Color(0xFF1B1035), 'Haiti'),
    _ColorName(Color(0xFF1B127B), 'Deep Koamaru'),
    _ColorName(Color(0xFF1B1404), 'Acadia'),
    _ColorName(Color(0xFF1B2F11), 'Seaweed'),
    _ColorName(Color(0xFF1B3162), 'Biscay'),
    _ColorName(Color(0xFF1B659D), 'Matisse'),
    _ColorName(Color(0xFF1C1208), 'Crowshead'),
    _ColorName(Color(0xFF1C1E13), 'Rangoon Green'),
    _ColorName(Color(0xFF1C39BB), 'Persian Blue'),
    _ColorName(Color(0xFF1C402E), 'Everglade'),
    _ColorName(Color(0xFF1C7C7D), 'Elm'),
    _ColorName(Color(0xFF1D6142), 'Green Pea'),
    _ColorName(Color(0xFF1E0F04), 'Creole'),
    _ColorName(Color(0xFF1E1609), 'Karaka'),
    _ColorName(Color(0xFF1E1708), 'El Paso'),
    _ColorName(Color(0xFF1E385B), 'Cello'),
    _ColorName(Color(0xFF1E433C), 'Te Papa Green'),
    _ColorName(Color(0xFF1E90FF), 'Dodger Blue'),
    _ColorName(Color(0xFF1E9AB0), 'Eastern Blue'),
    _ColorName(Color(0xFF1F120F), 'Night Rider'),
    _ColorName(Color(0xFF1FC2C2), 'Java'),
    _ColorName(Color(0xFF20208D), 'Jacksons Purple'),
    _ColorName(Color(0xFF202E54), 'Cloud Burst'),
    _ColorName(Color(0xFF204852), 'Blue Dianne'),
    _ColorName(Color(0xFF211A0E), 'Eternity'),
    _ColorName(Color(0xFF220878), 'Deep Blue'),
    _ColorName(Color(0xFF228B22), 'Forest Green'),
    _ColorName(Color(0xFF233418), 'Mallard'),
    _ColorName(Color(0xFF240A40), 'Violet'),
    _ColorName(Color(0xFF240C02), 'Kilimanjaro'),
    _ColorName(Color(0xFF242A1D), 'Log Cabin'),
    _ColorName(Color(0xFF242E16), 'Black Olive'),
    _ColorName(Color(0xFF24500F), 'Green House'),
    _ColorName(Color(0xFF251607), 'Graphite'),
    _ColorName(Color(0xFF251706), 'Cannon Black'),
    _ColorName(Color(0xFF251F4F), 'Port Gore'),
    _ColorName(Color(0xFF25272C), 'Shark'),
    _ColorName(Color(0xFF25311C), 'Green Kelp'),
    _ColorName(Color(0xFF2596D1), 'Curious Blue'),
    _ColorName(Color(0xFF260368), 'Paua'),
    _ColorName(Color(0xFF26056A), 'Paris M'),
    _ColorName(Color(0xFF261105), 'Wood Bark'),
    _ColorName(Color(0xFF261414), 'Gondola'),
    _ColorName(Color(0xFF262335), 'Steel Gray'),
    _ColorName(Color(0xFF26283B), 'Ebony Clay'),
    _ColorName(Color(0xFF273A81), 'Bay of Many'),
    _ColorName(Color(0xFF27504B), 'Plantation'),
    _ColorName(Color(0xFF278A5B), 'Eucalyptus'),
    _ColorName(Color(0xFF281E15), 'Oil'),
    _ColorName(Color(0xFF283A77), 'Astronaut'),
    _ColorName(Color(0xFF286ACD), 'Mariner'),
    _ColorName(Color(0xFF290C5E), 'Violent Violet'),
    _ColorName(Color(0xFF292130), 'Bastille'),
    _ColorName(Color(0xFF292319), 'Zeus'),
    _ColorName(Color(0xFF292937), 'Charade'),
    _ColorName(Color(0xFF297B9A), 'Jelly Bean'),
    _ColorName(Color(0xFF29AB87), 'Jungle Green'),
    _ColorName(Color(0xFF2A0359), 'Cherry Pie'),
    _ColorName(Color(0xFF2A140E), 'Coffee Bean'),
    _ColorName(Color(0xFF2A2630), 'Baltic Sea'),
    _ColorName(Color(0xFF2A380B), 'Turtle Green'),
    _ColorName(Color(0xFF2A52BE), 'Cerulean Blue'),
    _ColorName(Color(0xFF2B0202), 'Sepia Black'),
    _ColorName(Color(0xFF2B194F), 'Valhalla'),
    _ColorName(Color(0xFF2B3228), 'Heavy Metal'),
    _ColorName(Color(0xFF2C0E8C), 'Blue Gem'),
    _ColorName(Color(0xFF2C1632), 'Revolver'),
    _ColorName(Color(0xFF2C2133), 'Bleached Cedar'),
    _ColorName(Color(0xFF2C8C84), 'Lochinvar'),
    _ColorName(Color(0xFF2D2510), 'Mikado'),
    _ColorName(Color(0xFF2D383A), 'Outer Space'),
    _ColorName(Color(0xFF2D569B), 'St Tropez'),
    _ColorName(Color(0xFF2E0329), 'Jacaranda'),
    _ColorName(Color(0xFF2E1905), 'Jacko Bean'),
    _ColorName(Color(0xFF2E3222), 'Rangitoto'),
    _ColorName(Color(0xFF2E3F62), 'Rhino'),
    _ColorName(Color(0xFF2E8B57), 'Sea Green'),
    _ColorName(Color(0xFF2EBFD4), 'Scooter'),
    _ColorName(Color(0xFF2F270E), 'Onion'),
    _ColorName(Color(0xFF2F3CB3), 'Governor Bay'),
    _ColorName(Color(0xFF2F519E), 'Sapphire'),
    _ColorName(Color(0xFF2F5A57), 'Spectra'),
    _ColorName(Color(0xFF2F6168), 'Casal'),
    _ColorName(Color(0xFF300529), 'Melanzane'),
    _ColorName(Color(0xFF301F1E), 'Cocoa Brown'),
    _ColorName(Color(0xFF302A0F), 'Woodrush'),
    _ColorName(Color(0xFF304B6A), 'San Juan'),
    _ColorName(Color(0xFF30D5C8), 'Turquoise'),
    _ColorName(Color(0xFF311C17), 'Eclipse'),
    _ColorName(Color(0xFF314459), 'Pickled Bluewood'),
    _ColorName(Color(0xFF315BA1), 'Azure'),
    _ColorName(Color(0xFF31728D), 'Calypso'),
    _ColorName(Color(0xFF317D82), 'Paradiso'),
    _ColorName(Color(0xFF32127A), 'Persian Indigo'),
    _ColorName(Color(0xFF32293A), 'Blackcurrant'),
    _ColorName(Color(0xFF323232), 'Mine Shaft'),
    _ColorName(Color(0xFF325D52), 'Stromboli'),
    _ColorName(Color(0xFF327C14), 'Bilbao'),
    _ColorName(Color(0xFF327DA0), 'Astral'),
    _ColorName(Color(0xFF33036B), 'Christalle'),
    _ColorName(Color(0xFF33292F), 'Thunder'),
    _ColorName(Color(0xFF33CC99), 'Shamrock'),
    _ColorName(Color(0xFF341515), 'Tamarind'),
    _ColorName(Color(0xFF350036), 'Mardi Gras'),
    _ColorName(Color(0xFF350E42), 'Valentino'),
    _ColorName(Color(0xFF350E57), 'Jagger'),
    _ColorName(Color(0xFF353542), 'Tuna'),
    _ColorName(Color(0xFF354E8C), 'Chambray'),
    _ColorName(Color(0xFF363050), 'Martinique'),
    _ColorName(Color(0xFF363534), 'Tuatara'),
    _ColorName(Color(0xFF363C0D), 'Waiouru'),
    _ColorName(Color(0xFF36747D), 'Ming'),
    _ColorName(Color(0xFF368716), 'La Palma'),
    _ColorName(Color(0xFF370202), 'Chocolate'),
    _ColorName(Color(0xFF371D09), 'Clinker'),
    _ColorName(Color(0xFF37290E), 'Brown Tumbleweed'),
    _ColorName(Color(0xFF373021), 'Birch'),
    _ColorName(Color(0xFF377475), 'Oracle'),
    _ColorName(Color(0xFF380474), 'Blue Diamond'),
    _ColorName(Color(0xFF381A51), 'Grape'),
    _ColorName(Color(0xFF383533), 'Dune'),
    _ColorName(Color(0xFF384555), 'Oxford Blue'),
    _ColorName(Color(0xFF384910), 'Clover'),
    _ColorName(Color(0xFF394851), 'Limed Spruce'),
    _ColorName(Color(0xFF396413), 'Dell'),
    _ColorName(Color(0xFF3A0020), 'Toledo'),
    _ColorName(Color(0xFF3A2010), 'Sambuca'),
    _ColorName(Color(0xFF3A2A6A), 'Jacarta'),
    _ColorName(Color(0xFF3A686C), 'William'),
    _ColorName(Color(0xFF3A6A47), 'Killarney'),
    _ColorName(Color(0xFF3AB09E), 'Keppel'),
    _ColorName(Color(0xFF3B000B), 'Temptress'),
    _ColorName(Color(0xFF3B0910), 'Aubergine'),
    _ColorName(Color(0xFF3B1F1F), 'Jon'),
    _ColorName(Color(0xFF3B2820), 'Treehouse'),
    _ColorName(Color(0xFF3B7A57), 'Amazon'),
    _ColorName(Color(0xFF3B91B4), 'Boston Blue'),
    _ColorName(Color(0xFF3C0878), 'Windsor'),
    _ColorName(Color(0xFF3C1206), 'Rebel'),
    _ColorName(Color(0xFF3C1F76), 'Meteorite'),
    _ColorName(Color(0xFF3C2005), 'Dark Ebony'),
    _ColorName(Color(0xFF3C3910), 'Camouflage'),
    _ColorName(Color(0xFF3C4151), 'Bright Gray'),
    _ColorName(Color(0xFF3C4443), 'Cape Cod'),
    _ColorName(Color(0xFF3C493A), 'Lunar Green'),
    _ColorName(Color(0xFF3D0C02), 'Bean  '),
    _ColorName(Color(0xFF3D2B1F), 'Bistre'),
    _ColorName(Color(0xFF3D7D52), 'Goblin'),
    _ColorName(Color(0xFF3E0480), 'Kingfisher Daisy'),
    _ColorName(Color(0xFF3E1C14), 'Cedar'),
    _ColorName(Color(0xFF3E2B23), 'English Walnut'),
    _ColorName(Color(0xFF3E2C1C), 'Black Marlin'),
    _ColorName(Color(0xFF3E3A44), 'Ship Gray'),
    _ColorName(Color(0xFF3EABBF), 'Pelorous'),
    _ColorName(Color(0xFF3F2109), 'Bronze'),
    _ColorName(Color(0xFF3F2500), 'Cola'),
    _ColorName(Color(0xFF3F3002), 'Madras'),
    _ColorName(Color(0xFF3F307F), 'Minsk'),
    _ColorName(Color(0xFF3F4C3A), 'Cabbage Pont'),
    _ColorName(Color(0xFF3F583B), 'Tom Thumb'),
    _ColorName(Color(0xFF3F5D53), 'Mineral Green'),
    _ColorName(Color(0xFF3FC1AA), 'Puerto Rico'),
    _ColorName(Color(0xFF3FFF00), 'Harlequin'),
    _ColorName(Color(0xFF401801), 'Brown Pod'),
    _ColorName(Color(0xFF40291D), 'Cork'),
    _ColorName(Color(0xFF403B38), 'Masala'),
    _ColorName(Color(0xFF403D19), 'Thatch Green'),
    _ColorName(Color(0xFF405169), 'Fjord'),
    _ColorName(Color(0xFF40826D), 'Viridian'),
    _ColorName(Color(0xFF40A860), 'Chateau Green'),
    _ColorName(Color(0xFF410056), 'Ripe Plum'),
    _ColorName(Color(0xFF411E10), 'Paco'),
    _ColorName(Color(0xFF412010), 'Deep Oak'),
    _ColorName(Color(0xFF413C37), 'Merlin'),
    _ColorName(Color(0xFF414257), 'Gun Powder'),
    _ColorName(Color(0xFF414C7D), 'East Bay'),
    _ColorName(Color(0xFF4169E1), 'Royal Blue'),
    _ColorName(Color(0xFF41AA78), 'Ocean Green'),
    _ColorName(Color(0xFF420303), 'Burnt Maroon'),
    _ColorName(Color(0xFF423921), 'Lisbon Brown'),
    _ColorName(Color(0xFF427977), 'Faded Jade'),
    _ColorName(Color(0xFF431560), 'Scarlet Gum'),
    _ColorName(Color(0xFF433120), 'Iroko'),
    _ColorName(Color(0xFF433E37), 'Armadillo'),
    _ColorName(Color(0xFF434C59), 'River Bed'),
    _ColorName(Color(0xFF436A0D), 'Green Leaf'),
    _ColorName(Color(0xFF44012D), 'Barossa'),
    _ColorName(Color(0xFF441D00), 'Morocco Brown'),
    _ColorName(Color(0xFF444954), 'Mako'),
    _ColorName(Color(0xFF454936), 'Kelp'),
    _ColorName(Color(0xFF456CAC), 'San Marino'),
    _ColorName(Color(0xFF45B1E8), 'Picton Blue'),
    _ColorName(Color(0xFF460B41), 'Loulou'),
    _ColorName(Color(0xFF462425), 'Crater Brown'),
    _ColorName(Color(0xFF465945), 'Gray Asparagus'),
    _ColorName(Color(0xFF4682B4), 'Steel Blue'),
    _ColorName(Color(0xFF480404), 'Rustic Red'),
    _ColorName(Color(0xFF480607), 'Bulgarian Rose'),
    _ColorName(Color(0xFF480656), 'Clairvoyant'),
    _ColorName(Color(0xFF481C1C), 'Cocoa Bean'),
    _ColorName(Color(0xFF483131), 'Woody Brown'),
    _ColorName(Color(0xFF483C32), 'Taupe'),
    _ColorName(Color(0xFF49170C), 'Van Cleef'),
    _ColorName(Color(0xFF492615), 'Brown Derby'),
    _ColorName(Color(0xFF49371B), 'Metallic Bronze'),
    _ColorName(Color(0xFF495400), 'Verdun Green'),
    _ColorName(Color(0xFF496679), 'Blue Bayoux'),
    _ColorName(Color(0xFF497183), 'Bismark'),
    _ColorName(Color(0xFF4A2A04), 'Bracken'),
    _ColorName(Color(0xFF4A3004), 'Deep Bronze'),
    _ColorName(Color(0xFF4A3C30), 'Mondo'),
    _ColorName(Color(0xFF4A4244), 'Tundora'),
    _ColorName(Color(0xFF4A444B), 'Gravel'),
    _ColorName(Color(0xFF4A4E5A), 'Trout'),
    _ColorName(Color(0xFF4B0082), 'Pigment Indigo'),
    _ColorName(Color(0xFF4B5D52), 'Nandor'),
    _ColorName(Color(0xFF4C3024), 'Saddle'),
    _ColorName(Color(0xFF4C4F56), 'Abbey'),
    _ColorName(Color(0xFF4D0135), 'Blackberry'),
    _ColorName(Color(0xFF4D0A18), 'Cab Sav'),
    _ColorName(Color(0xFF4D1E01), 'Indian Tan'),
    _ColorName(Color(0xFF4D282D), 'Cowboy'),
    _ColorName(Color(0xFF4D282E), 'Livid Brown'),
    _ColorName(Color(0xFF4D3833), 'Rock'),
    _ColorName(Color(0xFF4D3D14), 'Punga'),
    _ColorName(Color(0xFF4D400F), 'Bronzetone'),
    _ColorName(Color(0xFF4D5328), 'Woodland'),
    _ColorName(Color(0xFF4E0606), 'Mahogany'),
    _ColorName(Color(0xFF4E2A5A), 'Bossanova'),
    _ColorName(Color(0xFF4E3B41), 'Matterhorn'),
    _ColorName(Color(0xFF4E420C), 'Bronze Olive'),
    _ColorName(Color(0xFF4E4562), 'Mulled Wine'),
    _ColorName(Color(0xFF4E6649), 'Axolotl'),
    _ColorName(Color(0xFF4E7F9E), 'Wedgewood'),
    _ColorName(Color(0xFF4EABD1), 'Shakespeare'),
    _ColorName(Color(0xFF4F1C70), 'Honey Flower'),
    _ColorName(Color(0xFF4F2398), 'Daisy Bush'),
    _ColorName(Color(0xFF4F69C6), 'Indigo'),
    _ColorName(Color(0xFF4F7942), 'Fern Green'),
    _ColorName(Color(0xFF4F9D5D), 'Fruit Salad'),
    _ColorName(Color(0xFF4FA83D), 'Apple'),
    _ColorName(Color(0xFF504351), 'Mortar'),
    _ColorName(Color(0xFF507096), 'Kashmir Blue'),
    _ColorName(Color(0xFF507672), 'Cutty Sark'),
    _ColorName(Color(0xFF50C878), 'Emerald'),
    _ColorName(Color(0xFF514649), 'Emperor'),
    _ColorName(Color(0xFF516E3D), 'Chalet Green'),
    _ColorName(Color(0xFF517C66), 'Como'),
    _ColorName(Color(0xFF51808F), 'Smalt Blue'),
    _ColorName(Color(0xFF52001F), 'Castro'),
    _ColorName(Color(0xFF520C17), 'Maroon Oak'),
    _ColorName(Color(0xFF523C94), 'Gigas'),
    _ColorName(Color(0xFF533455), 'Voodoo'),
    _ColorName(Color(0xFF534491), 'Victoria'),
    _ColorName(Color(0xFF53824B), 'Hippie Green'),
    _ColorName(Color(0xFF541012), 'Heath'),
    _ColorName(Color(0xFF544333), 'Judge Gray'),
    _ColorName(Color(0xFF54534D), 'Fuscous Gray'),
    _ColorName(Color(0xFF549019), 'Vida Loca'),
    _ColorName(Color(0xFF55280C), 'Cioccolato'),
    _ColorName(Color(0xFF555B10), 'Saratoga'),
    _ColorName(Color(0xFF556D56), 'Finlandia'),
    _ColorName(Color(0xFF5590D9), 'Havelock Blue'),
    _ColorName(Color(0xFF56B4BE), 'Fountain Blue'),
    _ColorName(Color(0xFF578363), 'Spring Leaves'),
    _ColorName(Color(0xFF583401), 'Saddle Brown'),
    _ColorName(Color(0xFF585562), 'Scarpa Flow'),
    _ColorName(Color(0xFF587156), 'Cactus'),
    _ColorName(Color(0xFF589AAF), 'Hippie Blue'),
    _ColorName(Color(0xFF591D35), 'Wine Berry'),
    _ColorName(Color(0xFF592804), 'Brown Bramble'),
    _ColorName(Color(0xFF593737), 'Congo Brown'),
    _ColorName(Color(0xFF594433), 'Millbrook'),
    _ColorName(Color(0xFF5A6E9C), 'Waikawa Gray'),
    _ColorName(Color(0xFF5A87A0), 'Horizon'),
    _ColorName(Color(0xFF5B3013), 'Jambalaya'),
    _ColorName(Color(0xFF5C0120), 'Bordeaux'),
    _ColorName(Color(0xFF5C0536), 'Mulberry Wood'),
    _ColorName(Color(0xFF5C2E01), 'Carnaby Tan'),
    _ColorName(Color(0xFF5C5D75), 'Comet'),
    _ColorName(Color(0xFF5D1E0F), 'Redwood'),
    _ColorName(Color(0xFF5D4C51), 'Don Juan'),
    _ColorName(Color(0xFF5D5C58), 'Chicago'),
    _ColorName(Color(0xFF5D5E37), 'Verdigris'),
    _ColorName(Color(0xFF5D7747), 'Dingley'),
    _ColorName(Color(0xFF5DA19F), 'Breaker Bay'),
    _ColorName(Color(0xFF5E483E), 'Kabul'),
    _ColorName(Color(0xFF5E5D3B), 'Hemlock'),
    _ColorName(Color(0xFF5F3D26), 'Irish Coffee'),
    _ColorName(Color(0xFF5F5F6E), 'Mid Gray'),
    _ColorName(Color(0xFF5F6672), 'Shuttle Gray'),
    _ColorName(Color(0xFF5FA777), 'Aqua Forest'),
    _ColorName(Color(0xFF5FB3AC), 'Tradewind'),
    _ColorName(Color(0xFF604913), 'Horses Neck'),
    _ColorName(Color(0xFF605B73), 'Smoky'),
    _ColorName(Color(0xFF606E68), 'Corduroy'),
    _ColorName(Color(0xFF6093D1), 'Danube'),
    _ColorName(Color(0xFF612718), 'Espresso'),
    _ColorName(Color(0xFF614051), 'Eggplant'),
    _ColorName(Color(0xFF615D30), 'Costa Del Sol'),
    _ColorName(Color(0xFF61845F), 'Glade Green'),
    _ColorName(Color(0xFF622F30), 'Buccaneer'),
    _ColorName(Color(0xFF623F2D), 'Quincy'),
    _ColorName(Color(0xFF624E9A), 'Butterfly Bush'),
    _ColorName(Color(0xFF625119), 'West Coast'),
    _ColorName(Color(0xFF626649), 'Finch'),
    _ColorName(Color(0xFF639A8F), 'Patina'),
    _ColorName(Color(0xFF63B76C), 'Fern'),
    _ColorName(Color(0xFF6456B7), 'Blue Violet'),
    _ColorName(Color(0xFF646077), 'Dolphin'),
    _ColorName(Color(0xFF646463), 'Storm Dust'),
    _ColorName(Color(0xFF646A54), 'Siam'),
    _ColorName(Color(0xFF646E75), 'Nevada'),
    _ColorName(Color(0xFF6495ED), 'Cornflower Blue'),
    _ColorName(Color(0xFF64CCDB), 'Viking'),
    _ColorName(Color(0xFF65000B), 'Rosewood'),
    _ColorName(Color(0xFF651A14), 'Cherrywood'),
    _ColorName(Color(0xFF652DC1), 'Purple Heart'),
    _ColorName(Color(0xFF657220), 'Fern Frond'),
    _ColorName(Color(0xFF65745D), 'Willow Grove'),
    _ColorName(Color(0xFF65869F), 'Hoki'),
    _ColorName(Color(0xFF660045), 'Pompadour'),
    _ColorName(Color(0xFF660099), 'Purple'),
    _ColorName(Color(0xFF66023C), 'Tyrian Purple'),
    _ColorName(Color(0xFF661010), 'Dark Tan'),
    _ColorName(Color(0xFF66B58F), 'Silver Tree'),
    _ColorName(Color(0xFF66FF00), 'Bright Green'),
    _ColorName(Color(0xFF66FF66), "Screamin' Green"),
    _ColorName(Color(0xFF67032D), 'Black Rose'),
    _ColorName(Color(0xFF675FA6), 'Scampi'),
    _ColorName(Color(0xFF676662), 'Ironside Gray'),
    _ColorName(Color(0xFF678975), 'Viridian Green'),
    _ColorName(Color(0xFF67A712), 'Christi'),
    _ColorName(Color(0xFF683600), 'Nutmeg Wood Finish'),
    _ColorName(Color(0xFF685558), 'Zambezi'),
    _ColorName(Color(0xFF685E6E), 'Salt Box'),
    _ColorName(Color(0xFF692545), 'Tawny Port'),
    _ColorName(Color(0xFF692D54), 'Finn'),
    _ColorName(Color(0xFF695F62), 'Scorpion'),
    _ColorName(Color(0xFF697E9A), 'Lynch'),
    _ColorName(Color(0xFF6A442E), 'Spice'),
    _ColorName(Color(0xFF6A5D1B), 'Himalaya'),
    _ColorName(Color(0xFF6A6051), 'Soya Bean'),
    _ColorName(Color(0xFF6B2A14), 'Hairy Heath'),
    _ColorName(Color(0xFF6B3FA0), 'Royal Purple'),
    _ColorName(Color(0xFF6B4E31), 'Shingle Fawn'),
    _ColorName(Color(0xFF6B5755), 'Dorado'),
    _ColorName(Color(0xFF6B8BA2), 'Bermuda Gray'),
    _ColorName(Color(0xFF6B8E23), 'Olive Drab'),
    _ColorName(Color(0xFF6C3082), 'Eminence'),
    _ColorName(Color(0xFF6CDAE7), 'Turquoise Blue'),
    _ColorName(Color(0xFF6D0101), 'Lonestar'),
    _ColorName(Color(0xFF6D5E54), 'Pine Cone'),
    _ColorName(Color(0xFF6D6C6C), 'Dove Gray'),
    _ColorName(Color(0xFF6D9292), 'Juniper'),
    _ColorName(Color(0xFF6D92A1), 'Gothic'),
    _ColorName(Color(0xFF6E0902), 'Red Oxide'),
    _ColorName(Color(0xFF6E1D14), 'Moccaccino'),
    _ColorName(Color(0xFF6E4826), 'Pickled Bean'),
    _ColorName(Color(0xFF6E4B26), 'Dallas'),
    _ColorName(Color(0xFF6E6D57), 'Kokoda'),
    _ColorName(Color(0xFF6E7783), 'Pale Sky'),
    _ColorName(Color(0xFF6F440C), 'Cafe Royale'),
    _ColorName(Color(0xFF6F6A61), 'Flint'),
    _ColorName(Color(0xFF6F8E63), 'Highland'),
    _ColorName(Color(0xFF6F9D02), 'Limeade'),
    _ColorName(Color(0xFF6FD0C5), 'Downy'),
    _ColorName(Color(0xFF701C1C), 'Persian Plum'),
    _ColorName(Color(0xFF704214), 'Sepia'),
    _ColorName(Color(0xFF704A07), 'Antique Bronze'),
    _ColorName(Color(0xFF704F50), 'Ferra'),
    _ColorName(Color(0xFF706555), 'Coffee'),
    _ColorName(Color(0xFF708090), 'Slate Gray'),
    _ColorName(Color(0xFF711A00), 'Cedar Wood Finish'),
    _ColorName(Color(0xFF71291D), 'Metallic Copper'),
    _ColorName(Color(0xFF714693), 'Affair'),
    _ColorName(Color(0xFF714AB2), 'Studio'),
    _ColorName(Color(0xFF715D47), 'Tobacco Brown'),
    _ColorName(Color(0xFF716338), 'Yellow Metal'),
    _ColorName(Color(0xFF716B56), 'Peat'),
    _ColorName(Color(0xFF716E10), 'Olivetone'),
    _ColorName(Color(0xFF717486), 'Storm Gray'),
    _ColorName(Color(0xFF718080), 'Sirocco'),
    _ColorName(Color(0xFF71D9E2), 'Aquamarine Blue'),
    _ColorName(Color(0xFF72010F), 'Venetian Red'),
    _ColorName(Color(0xFF724A2F), 'Old Copper'),
    _ColorName(Color(0xFF726D4E), 'Go Ben'),
    _ColorName(Color(0xFF727B89), 'Raven'),
    _ColorName(Color(0xFF731E8F), 'Seance'),
    _ColorName(Color(0xFF734A12), 'Raw Umber'),
    _ColorName(Color(0xFF736C9F), 'Kimberly'),
    _ColorName(Color(0xFF736D58), 'Crocodile'),
    _ColorName(Color(0xFF737829), 'Crete'),
    _ColorName(Color(0xFF738678), 'Xanadu'),
    _ColorName(Color(0xFF74640D), 'Spicy Mustard'),
    _ColorName(Color(0xFF747D63), 'Limed Ash'),
    _ColorName(Color(0xFF747D83), 'Rolling Stone'),
    _ColorName(Color(0xFF748881), 'Blue Smoke'),
    _ColorName(Color(0xFF749378), 'Laurel'),
    _ColorName(Color(0xFF74C365), 'Mantis'),
    _ColorName(Color(0xFF755A57), 'Russett'),
    _ColorName(Color(0xFF7563A8), 'Deluge'),
    _ColorName(Color(0xFF76395D), 'Cosmic'),
    _ColorName(Color(0xFF7666C6), 'Blue Marguerite'),
    _ColorName(Color(0xFF76BD17), 'Lima'),
    _ColorName(Color(0xFF76D7EA), 'Sky Blue'),
    _ColorName(Color(0xFF770F05), 'Dark Burgundy'),
    _ColorName(Color(0xFF771F1F), 'Crown of Thorns'),
    _ColorName(Color(0xFF773F1A), 'Walnut'),
    _ColorName(Color(0xFF776F61), 'Pablo'),
    _ColorName(Color(0xFF778120), 'Pacifika'),
    _ColorName(Color(0xFF779E86), 'Oxley'),
    _ColorName(Color(0xFF77DD77), 'Pastel Green'),
    _ColorName(Color(0xFF780109), 'Japanese Maple'),
    _ColorName(Color(0xFF782D19), 'Mocha'),
    _ColorName(Color(0xFF782F16), 'Peanut'),
    _ColorName(Color(0xFF78866B), 'Camouflage Green'),
    _ColorName(Color(0xFF788A25), 'Wasabi'),
    _ColorName(Color(0xFF788BBA), 'Ship Cove'),
    _ColorName(Color(0xFF78A39C), 'Sea Nymph'),
    _ColorName(Color(0xFF795D4C), 'Roman Coffee'),
    _ColorName(Color(0xFF796878), 'Old Lavender'),
    _ColorName(Color(0xFF796989), 'Rum'),
    _ColorName(Color(0xFF796A78), 'Fedora'),
    _ColorName(Color(0xFF796D62), 'Sandstone'),
    _ColorName(Color(0xFF79DEEC), 'Spray'),
    _ColorName(Color(0xFF7A013A), 'Siren'),
    _ColorName(Color(0xFF7A58C1), 'Fuchsia Blue'),
    _ColorName(Color(0xFF7A7A7A), 'Boulder'),
    _ColorName(Color(0xFF7A89B8), 'Wild Blue Yonder'),
    _ColorName(Color(0xFF7AC488), 'De York'),
    _ColorName(Color(0xFF7B3801), 'Red Beech'),
    _ColorName(Color(0xFF7B3F00), 'Cinnamon'),
    _ColorName(Color(0xFF7B6608), 'Yukon Gold'),
    _ColorName(Color(0xFF7B7874), 'Tapa'),
    _ColorName(Color(0xFF7B7C94), 'Waterloo '),
    _ColorName(Color(0xFF7B8265), 'Flax Smoke'),
    _ColorName(Color(0xFF7B9F80), 'Amulet'),
    _ColorName(Color(0xFF7BA05B), 'Asparagus'),
    _ColorName(Color(0xFF7C1C05), 'Kenyan Copper'),
    _ColorName(Color(0xFF7C7631), 'Pesto'),
    _ColorName(Color(0xFF7C778A), 'Topaz'),
    _ColorName(Color(0xFF7C7B7A), 'Concord'),
    _ColorName(Color(0xFF7C7B82), 'Jumbo'),
    _ColorName(Color(0xFF7C881A), 'Trendy Green'),
    _ColorName(Color(0xFF7CA1A6), 'Gumbo'),
    _ColorName(Color(0xFF7CB0A1), 'Acapulco'),
    _ColorName(Color(0xFF7CB7BB), 'Neptune'),
    _ColorName(Color(0xFF7D2C14), 'Pueblo'),
    _ColorName(Color(0xFF7DA98D), 'Bay Leaf'),
    _ColorName(Color(0xFF7DC8F7), 'Malibu'),
    _ColorName(Color(0xFF7DD8C6), 'Bermuda'),
    _ColorName(Color(0xFF7E3A15), 'Copper Canyon'),
    _ColorName(Color(0xFF7F1734), 'Claret'),
    _ColorName(Color(0xFF7F3A02), 'Peru Tan'),
    _ColorName(Color(0xFF7F626D), 'Falcon'),
    _ColorName(Color(0xFF7F7589), 'Mobster'),
    _ColorName(Color(0xFF7F76D3), 'Moody Blue'),
    _ColorName(Color(0xFF7FFF00), 'Chartreuse'),
    _ColorName(Color(0xFF7FFFD4), 'Aquamarine'),
    _ColorName(Color(0xFF800000), 'Maroon'),
    _ColorName(Color(0xFF800B47), 'Rose Bud Cherry'),
    _ColorName(Color(0xFF801818), 'Falu Red'),
    _ColorName(Color(0xFF80341F), 'Red Robin'),
    _ColorName(Color(0xFF803790), 'Vivid Violet'),
    _ColorName(Color(0xFF80461B), 'Russet'),
    _ColorName(Color(0xFF807E79), 'Friar Gray'),
    _ColorName(Color(0xFF808000), 'Olive'),
    _ColorName(Color(0xFF808080), 'Gray'),
    _ColorName(Color(0xFF80B3AE), 'Gulf Stream'),
    _ColorName(Color(0xFF80B3C4), 'Glacier'),
    _ColorName(Color(0xFF80CCEA), 'Seagull'),
    _ColorName(Color(0xFF81422C), 'Nutmeg'),
    _ColorName(Color(0xFF816E71), 'Spicy Pink'),
    _ColorName(Color(0xFF817377), 'Empress'),
    _ColorName(Color(0xFF819885), 'Spanish Green'),
    _ColorName(Color(0xFF826F65), 'Sand Dune'),
    _ColorName(Color(0xFF828685), 'Gunsmoke'),
    _ColorName(Color(0xFF828F72), 'Battleship Gray'),
    _ColorName(Color(0xFF831923), 'Merlot'),
    _ColorName(Color(0xFF837050), 'Shadow'),
    _ColorName(Color(0xFF83AA5D), 'Chelsea Cucumber'),
    _ColorName(Color(0xFF83D0C6), 'Monte Carlo'),
    _ColorName(Color(0xFF843179), 'Plum'),
    _ColorName(Color(0xFF84A0A0), 'Granny Smith'),
    _ColorName(Color(0xFF8581D9), 'Chetwode Blue'),
    _ColorName(Color(0xFF858470), 'Bandicoot'),
    _ColorName(Color(0xFF859FAF), 'Bali Hai'),
    _ColorName(Color(0xFF85C4CC), 'Half Baked'),
    _ColorName(Color(0xFF860111), 'Red Devil'),
    _ColorName(Color(0xFF863C3C), 'Lotus'),
    _ColorName(Color(0xFF86483C), 'Ironstone'),
    _ColorName(Color(0xFF864D1E), 'Bull Shot'),
    _ColorName(Color(0xFF86560A), 'Rusty Nail'),
    _ColorName(Color(0xFF868974), 'Bitter'),
    _ColorName(Color(0xFF86949F), 'Regent Gray'),
    _ColorName(Color(0xFF871550), 'Disco'),
    _ColorName(Color(0xFF87756E), 'Americano'),
    _ColorName(Color(0xFF877C7B), 'Hurricane'),
    _ColorName(Color(0xFF878D91), 'Oslo Gray'),
    _ColorName(Color(0xFF87AB39), 'Sushi'),
    _ColorName(Color(0xFF885342), 'Spicy Mix'),
    _ColorName(Color(0xFF886221), 'Kumera'),
    _ColorName(Color(0xFF888387), 'Suva Gray'),
    _ColorName(Color(0xFF888D65), 'Avocado'),
    _ColorName(Color(0xFF893456), 'Camelot'),
    _ColorName(Color(0xFF893843), 'Solid Pink'),
    _ColorName(Color(0xFF894367), 'Cannon Pink'),
    _ColorName(Color(0xFF897D6D), 'Makara'),
    _ColorName(Color(0xFF8A3324), 'Burnt Umber'),
    _ColorName(Color(0xFF8A73D6), 'True V'),
    _ColorName(Color(0xFF8A8360), 'Clay Creek'),
    _ColorName(Color(0xFF8A8389), 'Monsoon'),
    _ColorName(Color(0xFF8A8F8A), 'Stack'),
    _ColorName(Color(0xFF8AB9F1), 'Jordy Blue'),
    _ColorName(Color(0xFF8B00FF), 'Electric Violet'),
    _ColorName(Color(0xFF8B0723), 'Monarch'),
    _ColorName(Color(0xFF8B6B0B), 'Corn Harvest'),
    _ColorName(Color(0xFF8B8470), 'Olive Haze'),
    _ColorName(Color(0xFF8B847E), 'Schooner'),
    _ColorName(Color(0xFF8B8680), 'Natural Gray'),
    _ColorName(Color(0xFF8B9C90), 'Mantle'),
    _ColorName(Color(0xFF8B9FEE), 'Portage'),
    _ColorName(Color(0xFF8BA690), 'Envy'),
    _ColorName(Color(0xFF8BA9A5), 'Cascade'),
    _ColorName(Color(0xFF8BE6D8), 'Riptide'),
    _ColorName(Color(0xFF8C055E), 'Cardinal Pink'),
    _ColorName(Color(0xFF8C472F), 'Mule Fawn'),
    _ColorName(Color(0xFF8C5738), 'Potters Clay'),
    _ColorName(Color(0xFF8C6495), 'Trendy Pink'),
    _ColorName(Color(0xFF8D0226), 'Paprika'),
    _ColorName(Color(0xFF8D3D38), 'Sanguine Brown'),
    _ColorName(Color(0xFF8D3F3F), 'Tosca'),
    _ColorName(Color(0xFF8D7662), 'Cement'),
    _ColorName(Color(0xFF8D8974), 'Granite Green'),
    _ColorName(Color(0xFF8D90A1), 'Manatee'),
    _ColorName(Color(0xFF8DA8CC), 'Polo Blue'),
    _ColorName(Color(0xFF8E0000), 'Red Berry'),
    _ColorName(Color(0xFF8E4D1E), 'Rope'),
    _ColorName(Color(0xFF8E6F70), 'Opium'),
    _ColorName(Color(0xFF8E775E), 'Domino'),
    _ColorName(Color(0xFF8E8190), 'Mamba'),
    _ColorName(Color(0xFF8EABC1), 'Nepal'),
    _ColorName(Color(0xFF8F021C), 'Pohutukawa'),
    _ColorName(Color(0xFF8F3E33), 'El Salva'),
    _ColorName(Color(0xFF8F4B0E), 'Korma'),
    _ColorName(Color(0xFF8F8176), 'Squirrel'),
    _ColorName(Color(0xFF8FD6B4), 'Vista Blue'),
    _ColorName(Color(0xFF900020), 'Burgundy'),
    _ColorName(Color(0xFF901E1E), 'Old Brick'),
    _ColorName(Color(0xFF907874), 'Hemp'),
    _ColorName(Color(0xFF907B71), 'Almond Frost'),
    _ColorName(Color(0xFF908D39), 'Sycamore'),
    _ColorName(Color(0xFF92000A), 'Sangria'),
    _ColorName(Color(0xFF924321), 'Cumin'),
    _ColorName(Color(0xFF926F5B), 'Beaver'),
    _ColorName(Color(0xFF928573), 'Stonewall'),
    _ColorName(Color(0xFF928590), 'Venus'),
    _ColorName(Color(0xFF9370DB), 'Medium Purple'),
    _ColorName(Color(0xFF93CCEA), 'Cornflower'),
    _ColorName(Color(0xFF93DFB8), 'Algae Green'),
    _ColorName(Color(0xFF944747), 'Copper Rust'),
    _ColorName(Color(0xFF948771), 'Arrowtown'),
    _ColorName(Color(0xFF950015), 'Scarlett'),
    _ColorName(Color(0xFF956387), 'Strikemaster'),
    _ColorName(Color(0xFF959396), 'Mountain Mist'),
    _ColorName(Color(0xFF960018), 'Carmine'),
    _ColorName(Color(0xFF964B00), 'Brown'),
    _ColorName(Color(0xFF967059), 'Leather'),
    _ColorName(Color(0xFF9678B6), 'Purple Mountain'),
    _ColorName(Color(0xFF967BB6), 'Lavender Purple'),
    _ColorName(Color(0xFF96A8A1), 'Pewter'),
    _ColorName(Color(0xFF96BBAB), 'Summer Green'),
    _ColorName(Color(0xFF97605D), 'Au Chico'),
    _ColorName(Color(0xFF9771B5), 'Wisteria'),
    _ColorName(Color(0xFF97CD2D), 'Atlantis'),
    _ColorName(Color(0xFF983D61), 'Vin Rouge'),
    _ColorName(Color(0xFF9874D3), 'Lilac Bush'),
    _ColorName(Color(0xFF98777B), 'Bazaar'),
    _ColorName(Color(0xFF98811B), 'Hacienda'),
    _ColorName(Color(0xFF988D77), 'Pale Oyster'),
    _ColorName(Color(0xFF98FF98), 'Mint Green'),
    _ColorName(Color(0xFF990066), 'Fresh Eggplant'),
    _ColorName(Color(0xFF991199), 'Violet Eggplant'),
    _ColorName(Color(0xFF991613), 'Tamarillo'),
    _ColorName(Color(0xFF991B07), 'Totem Pole'),
    _ColorName(Color(0xFF996666), 'Copper Rose'),
    _ColorName(Color(0xFF9966CC), 'Amethyst'),
    _ColorName(Color(0xFF997A8D), 'Mountbatten Pink'),
    _ColorName(Color(0xFF9999CC), 'Blue Bell'),
    _ColorName(Color(0xFF9A3820), 'Prairie Sand'),
    _ColorName(Color(0xFF9A6E61), 'Toast'),
    _ColorName(Color(0xFF9A9577), 'Gurkha'),
    _ColorName(Color(0xFF9AB973), 'Olivine'),
    _ColorName(Color(0xFF9AC2B8), 'Shadow Green'),
    _ColorName(Color(0xFF9B4703), 'Oregon'),
    _ColorName(Color(0xFF9B9E8F), 'Lemon Grass'),
    _ColorName(Color(0xFF9C3336), 'Stiletto'),
    _ColorName(Color(0xFF9D5616), 'Hawaiian Tan'),
    _ColorName(Color(0xFF9DACB7), 'Gull Gray'),
    _ColorName(Color(0xFF9DC209), 'Pistachio'),
    _ColorName(Color(0xFF9DE093), 'Granny Smith Apple'),
    _ColorName(Color(0xFF9DE5FF), 'Anakiwa'),
    _ColorName(Color(0xFF9E5302), 'Chelsea Gem'),
    _ColorName(Color(0xFF9E5B40), 'Sepia Skin'),
    _ColorName(Color(0xFF9EA587), 'Sage'),
    _ColorName(Color(0xFF9EA91F), 'Citron'),
    _ColorName(Color(0xFF9EB1CD), 'Rock Blue'),
    _ColorName(Color(0xFF9EDEE0), 'Morning Glory'),
    _ColorName(Color(0xFF9F381D), 'Cognac'),
    _ColorName(Color(0xFF9F821C), 'Reef Gold'),
    _ColorName(Color(0xFF9F9F9C), 'Star Dust'),
    _ColorName(Color(0xFF9FA0B1), 'Santas Gray'),
    _ColorName(Color(0xFF9FD7D3), 'Sinbad'),
    _ColorName(Color(0xFF9FDD8C), 'Feijoa'),
    _ColorName(Color(0xFFA02712), 'Tabasco'),
    _ColorName(Color(0xFFA1750D), 'Buttered Rum'),
    _ColorName(Color(0xFFA1ADB5), 'Hit Gray'),
    _ColorName(Color(0xFFA1C50A), 'Citrus'),
    _ColorName(Color(0xFFA1DAD7), 'Aqua Island'),
    _ColorName(Color(0xFFA1E9DE), 'Water Leaf'),
    _ColorName(Color(0xFFA2006D), 'Flirt'),
    _ColorName(Color(0xFFA23B6C), 'Rouge'),
    _ColorName(Color(0xFFA26645), 'Cape Palliser'),
    _ColorName(Color(0xFFA2AAB3), 'Gray Chateau'),
    _ColorName(Color(0xFFA2AEAB), 'Edward'),
    _ColorName(Color(0xFFA3807B), 'Pharlap'),
    _ColorName(Color(0xFFA397B4), 'Amethyst Smoke'),
    _ColorName(Color(0xFFA3E3ED), 'Blizzard Blue'),
    _ColorName(Color(0xFFA4A49D), 'Delta'),
    _ColorName(Color(0xFFA4A6D3), 'Wistful'),
    _ColorName(Color(0xFFA4AF6E), 'Green Smoke'),
    _ColorName(Color(0xFFA50B5E), 'Jazzberry Jam'),
    _ColorName(Color(0xFFA59B91), 'Zorba'),
    _ColorName(Color(0xFFA5CB0C), 'Bahia'),
    _ColorName(Color(0xFFA62F20), 'Roof Terracotta'),
    _ColorName(Color(0xFFA65529), 'Paarl'),
    _ColorName(Color(0xFFA68B5B), 'Barley Corn'),
    _ColorName(Color(0xFFA69279), 'Donkey Brown'),
    _ColorName(Color(0xFFA6A29A), 'Dawn'),
    _ColorName(Color(0xFFA72525), 'Mexican Red'),
    _ColorName(Color(0xFFA7882C), 'Luxor Gold'),
    _ColorName(Color(0xFFA85307), 'Rich Gold'),
    _ColorName(Color(0xFFA86515), 'Reno Sand'),
    _ColorName(Color(0xFFA86B6B), 'Coral Tree'),
    _ColorName(Color(0xFFA8989B), 'Dusty Gray'),
    _ColorName(Color(0xFFA899E6), 'Dull Lavender'),
    _ColorName(Color(0xFFA8A589), 'Tallow'),
    _ColorName(Color(0xFFA8AE9C), 'Bud'),
    _ColorName(Color(0xFFA8AF8E), 'Locust'),
    _ColorName(Color(0xFFA8BD9F), 'Norway'),
    _ColorName(Color(0xFFA8E3BD), 'Chinook'),
    _ColorName(Color(0xFFA9A491), 'Gray Olive'),
    _ColorName(Color(0xFFA9ACB6), 'Aluminium'),
    _ColorName(Color(0xFFA9B2C3), 'Cadet Blue'),
    _ColorName(Color(0xFFA9B497), 'Schist'),
    _ColorName(Color(0xFFA9BDBF), 'Tower Gray'),
    _ColorName(Color(0xFFA9BEF2), 'Perano'),
    _ColorName(Color(0xFFA9C6C2), 'Opal'),
    _ColorName(Color(0xFFAA375A), 'Night Shadz'),
    _ColorName(Color(0xFFAA4203), 'Fire'),
    _ColorName(Color(0xFFAA8B5B), 'Muesli'),
    _ColorName(Color(0xFFAA8D6F), 'Sandal'),
    _ColorName(Color(0xFFAAA5A9), 'Shady Lady'),
    _ColorName(Color(0xFFAAA9CD), 'Logan'),
    _ColorName(Color(0xFFAAABB7), 'Spun Pearl'),
    _ColorName(Color(0xFFAAD6E6), 'Regent St Blue'),
    _ColorName(Color(0xFFAAF0D1), 'Magic Mint'),
    _ColorName(Color(0xFFAB0563), 'Lipstick'),
    _ColorName(Color(0xFFAB3472), 'Royal Heath'),
    _ColorName(Color(0xFFAB917A), 'Sandrift'),
    _ColorName(Color(0xFFABA0D9), 'Cold Purple'),
    _ColorName(Color(0xFFABA196), 'Bronco'),
    _ColorName(Color(0xFFAC8A56), 'Limed Oak'),
    _ColorName(Color(0xFFAC91CE), 'East Side'),
    _ColorName(Color(0xFFAC9E22), 'Lemon Ginger'),
    _ColorName(Color(0xFFACA494), 'Napa'),
    _ColorName(Color(0xFFACA586), 'Hillary'),
    _ColorName(Color(0xFFACA59F), 'Cloudy'),
    _ColorName(Color(0xFFACACAC), 'Silver Chalice'),
    _ColorName(Color(0xFFACB78E), 'Swamp Green'),
    _ColorName(Color(0xFFACCBB1), 'Spring Rain'),
    _ColorName(Color(0xFFACDD4D), 'Conifer'),
    _ColorName(Color(0xFFACE1AF), 'Celadon'),
    _ColorName(Color(0xFFAD781B), 'Mandalay'),
    _ColorName(Color(0xFFADBED1), 'Casper'),
    _ColorName(Color(0xFFADDFAD), 'Moss Green'),
    _ColorName(Color(0xFFADE6C4), 'Padua'),
    _ColorName(Color(0xFFADFF2F), 'Green Yellow'),
    _ColorName(Color(0xFFAE4560), 'Hippie Pink'),
    _ColorName(Color(0xFFAE6020), 'Desert'),
    _ColorName(Color(0xFFAE809E), 'Bouquet'),
    _ColorName(Color(0xFFAF4035), 'Medium Carmine'),
    _ColorName(Color(0xFFAF4D43), 'Apple Blossom'),
    _ColorName(Color(0xFFAF593E), 'Brown Rust'),
    _ColorName(Color(0xFFAF8751), 'Driftwood'),
    _ColorName(Color(0xFFAF8F2C), 'Alpine'),
    _ColorName(Color(0xFFAF9F1C), 'Lucky'),
    _ColorName(Color(0xFFAFA09E), 'Martini'),
    _ColorName(Color(0xFFAFB1B8), 'Bombay'),
    _ColorName(Color(0xFFAFBDD9), 'Pigeon Post'),
    _ColorName(Color(0xFFB04C6A), 'Cadillac'),
    _ColorName(Color(0xFFB05D54), 'Matrix'),
    _ColorName(Color(0xFFB05E81), 'Tapestry'),
    _ColorName(Color(0xFFB06608), 'Mai Tai'),
    _ColorName(Color(0xFFB09A95), 'Del Rio'),
    _ColorName(Color(0xFFB0E0E6), 'Powder Blue'),
    _ColorName(Color(0xFFB0E313), 'Inch Worm'),
    _ColorName(Color(0xFFB10000), 'Bright Red'),
    _ColorName(Color(0xFFB14A0B), 'Vesuvius'),
    _ColorName(Color(0xFFB1610B), 'Pumpkin Skin'),
    _ColorName(Color(0xFFB16D52), 'Santa Fe'),
    _ColorName(Color(0xFFB19461), 'Teak'),
    _ColorName(Color(0xFFB1E2C1), 'Fringy Flower'),
    _ColorName(Color(0xFFB1F4E7), 'Ice Cold'),
    _ColorName(Color(0xFFB20931), 'Shiraz'),
    _ColorName(Color(0xFFB2A1EA), 'Biloba Flower'),
    _ColorName(Color(0xFFB32D29), 'Tall Poppy'),
    _ColorName(Color(0xFFB35213), 'Fiery Orange'),
    _ColorName(Color(0xFFB38007), 'Hot Toddy'),
    _ColorName(Color(0xFFB3AF95), 'Taupe Gray'),
    _ColorName(Color(0xFFB3C110), 'La Rioja'),
    _ColorName(Color(0xFFB43332), 'Well Read'),
    _ColorName(Color(0xFFB44668), 'Blush'),
    _ColorName(Color(0xFFB4CFD3), 'Jungle Mist'),
    _ColorName(Color(0xFFB57281), 'Turkish Rose'),
    _ColorName(Color(0xFFB57EDC), 'Lavender'),
    _ColorName(Color(0xFFB5A27F), 'Mongoose'),
    _ColorName(Color(0xFFB5B35C), 'Olive Green'),
    _ColorName(Color(0xFFB5D2CE), 'Jet Stream'),
    _ColorName(Color(0xFFB5ECDF), 'Cruise'),
    _ColorName(Color(0xFFB6316C), 'Hibiscus'),
    _ColorName(Color(0xFFB69D98), 'Thatch'),
    _ColorName(Color(0xFFB6B095), 'Heathered Gray'),
    _ColorName(Color(0xFFB6BAA4), 'Eagle'),
    _ColorName(Color(0xFFB6D1EA), 'Spindle'),
    _ColorName(Color(0xFFB6D3BF), 'Gum Leaf'),
    _ColorName(Color(0xFFB7410E), 'Rust'),
    _ColorName(Color(0xFFB78E5C), 'Muddy Waters'),
    _ColorName(Color(0xFFB7A214), 'Sahara'),
    _ColorName(Color(0xFFB7A458), 'Husk'),
    _ColorName(Color(0xFFB7B1B1), 'Nobel'),
    _ColorName(Color(0xFFB7C3D0), 'Heather'),
    _ColorName(Color(0xFFB7F0BE), 'Madang'),
    _ColorName(Color(0xFFB81104), 'Milano Red'),
    _ColorName(Color(0xFFB87333), 'Copper'),
    _ColorName(Color(0xFFB8B56A), 'Gimblet'),
    _ColorName(Color(0xFFB8C1B1), 'Green Spring'),
    _ColorName(Color(0xFFB8C25D), 'Celery'),
    _ColorName(Color(0xFFB8E0F9), 'Sail'),
    _ColorName(Color(0xFFB94E48), 'Chestnut'),
    _ColorName(Color(0xFFB95140), 'Crail'),
    _ColorName(Color(0xFFB98D28), 'Marigold'),
    _ColorName(Color(0xFFB9C46A), 'Wild Willow'),
    _ColorName(Color(0xFFB9C8AC), 'Rainee'),
    _ColorName(Color(0xFFBA0101), 'Guardsman Red'),
    _ColorName(Color(0xFFBA450C), 'Rock Spray'),
    _ColorName(Color(0xFFBA6F1E), 'Bourbon'),
    _ColorName(Color(0xFFBA7F03), 'Pirate Gold'),
    _ColorName(Color(0xFFBAB1A2), 'Nomad'),
    _ColorName(Color(0xFFBAC7C9), 'Submarine'),
    _ColorName(Color(0xFFBAEEF9), 'Charlotte'),
    _ColorName(Color(0xFFBB3385), 'Medium Red Violet'),
    _ColorName(Color(0xFFBB8983), 'Brandy Rose'),
    _ColorName(Color(0xFFBBD009), 'Rio Grande'),
    _ColorName(Color(0xFFBBD7C1), 'Surf'),
    _ColorName(Color(0xFFBCC9C2), 'Powder Ash'),
    _ColorName(Color(0xFFBD5E2E), 'Tuscany'),
    _ColorName(Color(0xFFBD978E), 'Quicksand'),
    _ColorName(Color(0xFFBDB1A8), 'Silk'),
    _ColorName(Color(0xFFBDB2A1), 'Malta'),
    _ColorName(Color(0xFFBDB3C7), 'Chatelle'),
    _ColorName(Color(0xFFBDBBD7), 'Lavender Gray'),
    _ColorName(Color(0xFFBDBDC6), 'French Gray'),
    _ColorName(Color(0xFFBDC8B3), 'Clay Ash'),
    _ColorName(Color(0xFFBDC9CE), 'Loblolly'),
    _ColorName(Color(0xFFBDEDFD), 'French Pass'),
    _ColorName(Color(0xFFBEA6C3), 'London Hue'),
    _ColorName(Color(0xFFBEB5B7), 'Pink Swan'),
    _ColorName(Color(0xFFBEDE0D), 'Fuego'),
    _ColorName(Color(0xFFBF5500), 'Rose of Sharon'),
    _ColorName(Color(0xFFBFB8B0), 'Tide'),
    _ColorName(Color(0xFFBFBED8), 'Blue Haze'),
    _ColorName(Color(0xFFBFC1C2), 'Silver Sand'),
    _ColorName(Color(0xFFBFC921), 'Key Lime Pie'),
    _ColorName(Color(0xFFBFDBE2), 'Ziggurat'),
    _ColorName(Color(0xFFBFFF00), 'Lime'),
    _ColorName(Color(0xFFC02B18), 'Thunderbird'),
    _ColorName(Color(0xFFC04737), 'Mojo'),
    _ColorName(Color(0xFFC08081), 'Old Rose'),
    _ColorName(Color(0xFFC0C0C0), 'Silver'),
    _ColorName(Color(0xFFC0D3B9), 'Pale Leaf'),
    _ColorName(Color(0xFFC0D8B6), 'Pixie Green'),
    _ColorName(Color(0xFFC1440E), 'Tia Maria'),
    _ColorName(Color(0xFFC154C1), 'Fuchsia Pink'),
    _ColorName(Color(0xFFC1A004), 'Buddha Gold'),
    _ColorName(Color(0xFFC1B7A4), 'Bison Hide'),
    _ColorName(Color(0xFFC1BAB0), 'Tea'),
    _ColorName(Color(0xFFC1BECD), 'Gray Suit'),
    _ColorName(Color(0xFFC1D7B0), 'Sprout'),
    _ColorName(Color(0xFFC1F07C), 'Sulu'),
    _ColorName(Color(0xFFC26B03), 'Indochine'),
    _ColorName(Color(0xFFC2955D), 'Twine'),
    _ColorName(Color(0xFFC2BDB6), 'Cotton Seed'),
    _ColorName(Color(0xFFC2CAC4), 'Pumice'),
    _ColorName(Color(0xFFC2E8E5), 'Jagged Ice'),
    _ColorName(Color(0xFFC32148), 'Maroon Flush'),
    _ColorName(Color(0xFFC3B091), 'Indian Khaki'),
    _ColorName(Color(0xFFC3BFC1), 'Pale Slate'),
    _ColorName(Color(0xFFC3C3BD), 'Gray Nickel'),
    _ColorName(Color(0xFFC3CDE6), 'Periwinkle Gray'),
    _ColorName(Color(0xFFC3D1D1), 'Tiara'),
    _ColorName(Color(0xFFC3DDF9), 'Tropical Blue'),
    _ColorName(Color(0xFFC41E3A), 'Cardinal'),
    _ColorName(Color(0xFFC45655), 'Fuzzy Wuzzy Brown'),
    _ColorName(Color(0xFFC45719), 'Orange Roughy'),
    _ColorName(Color(0xFFC4C4BC), 'Mist Gray'),
    _ColorName(Color(0xFFC4D0B0), 'Coriander'),
    _ColorName(Color(0xFFC4F4EB), 'Mint Tulip'),
    _ColorName(Color(0xFFC54B8C), 'Mulberry'),
    _ColorName(Color(0xFFC59922), 'Nugget'),
    _ColorName(Color(0xFFC5994B), 'Tussock'),
    _ColorName(Color(0xFFC5DBCA), 'Sea Mist'),
    _ColorName(Color(0xFFC5E17A), 'Yellow Green'),
    _ColorName(Color(0xFFC62D42), 'Brick Red'),
    _ColorName(Color(0xFFC6726B), 'Contessa'),
    _ColorName(Color(0xFFC69191), 'Oriental Pink'),
    _ColorName(Color(0xFFC6A84B), 'Roti'),
    _ColorName(Color(0xFFC6C3B5), 'Ash'),
    _ColorName(Color(0xFFC6C8BD), 'Kangaroo'),
    _ColorName(Color(0xFFC6E610), 'Las Palmas'),
    _ColorName(Color(0xFFC7031E), 'Monza'),
    _ColorName(Color(0xFFC71585), 'Red Violet'),
    _ColorName(Color(0xFFC7BCA2), 'Coral Reef'),
    _ColorName(Color(0xFFC7C1FF), 'Melrose'),
    _ColorName(Color(0xFFC7C4BF), 'Cloud'),
    _ColorName(Color(0xFFC7C9D5), 'Ghost'),
    _ColorName(Color(0xFFC7CD90), 'Pine Glade'),
    _ColorName(Color(0xFFC7DDE5), 'Botticelli'),
    _ColorName(Color(0xFFC88A65), 'Antique Brass'),
    _ColorName(Color(0xFFC8A2C8), 'Lilac'),
    _ColorName(Color(0xFFC8A528), 'Hokey Pokey'),
    _ColorName(Color(0xFFC8AABF), 'Lily'),
    _ColorName(Color(0xFFC8B568), 'Laser'),
    _ColorName(Color(0xFFC8E3D7), 'Edgewater'),
    _ColorName(Color(0xFFC96323), 'Piper'),
    _ColorName(Color(0xFFC99415), 'Pizza'),
    _ColorName(Color(0xFFC9A0DC), 'Light Wisteria'),
    _ColorName(Color(0xFFC9B29B), 'Rodeo Dust'),
    _ColorName(Color(0xFFC9B35B), 'Sundance'),
    _ColorName(Color(0xFFC9B93B), 'Earls Green'),
    _ColorName(Color(0xFFC9C0BB), 'Silver Rust'),
    _ColorName(Color(0xFFC9D9D2), 'Conch'),
    _ColorName(Color(0xFFC9FFA2), 'Reef'),
    _ColorName(Color(0xFFC9FFE5), 'Aero Blue'),
    _ColorName(Color(0xFFCA3435), 'Flush Mahogany'),
    _ColorName(Color(0xFFCABB48), 'Turmeric'),
    _ColorName(Color(0xFFCADCD4), 'Paris White'),
    _ColorName(Color(0xFFCAE00D), 'Bitter Lemon'),
    _ColorName(Color(0xFFCAE6DA), 'Skeptic'),
    _ColorName(Color(0xFFCB8FA9), 'Viola'),
    _ColorName(Color(0xFFCBCAB6), 'Foggy Gray'),
    _ColorName(Color(0xFFCBD3B0), 'Green Mist'),
    _ColorName(Color(0xFFCBDBD6), 'Nebula'),
    _ColorName(Color(0xFFCC3333), 'Persian Red'),
    _ColorName(Color(0xFFCC5501), 'Burnt Orange'),
    _ColorName(Color(0xFFCC7722), 'Ochre'),
    _ColorName(Color(0xFFCC8899), 'Puce'),
    _ColorName(Color(0xFFCCCAA8), 'Thistle Green'),
    _ColorName(Color(0xFFCCCCFF), 'Periwinkle'),
    _ColorName(Color(0xFFCCFF00), 'Electric Lime'),
    _ColorName(Color(0xFFCD5700), 'Tenn'),
    _ColorName(Color(0xFFCD5C5C), 'Chestnut Rose'),
    _ColorName(Color(0xFFCD8429), 'Brandy Punch'),
    _ColorName(Color(0xFFCDF4FF), 'Onahau'),
    _ColorName(Color(0xFFCEB98F), 'Sorrell Brown'),
    _ColorName(Color(0xFFCEBABA), 'Cold Turkey'),
    _ColorName(Color(0xFFCEC291), 'Yuma'),
    _ColorName(Color(0xFFCEC7A7), 'Chino'),
    _ColorName(Color(0xFFCFA39D), 'Eunry'),
    _ColorName(Color(0xFFCFB53B), 'Old Gold'),
    _ColorName(Color(0xFFCFDCCF), 'Tasman'),
    _ColorName(Color(0xFFCFE5D2), 'Surf Crest'),
    _ColorName(Color(0xFFCFF9F3), 'Humming Bird'),
    _ColorName(Color(0xFFCFFAF4), 'Scandal'),
    _ColorName(Color(0xFFD05F04), 'Red Stage'),
    _ColorName(Color(0xFFD06DA1), 'Hopbush'),
    _ColorName(Color(0xFFD07D12), 'Meteor'),
    _ColorName(Color(0xFFD0BEF8), 'Perfume'),
    _ColorName(Color(0xFFD0C0E5), 'Prelude'),
    _ColorName(Color(0xFFD0F0C0), 'Tea Green'),
    _ColorName(Color(0xFFD18F1B), 'Geebung'),
    _ColorName(Color(0xFFD1BEA8), 'Vanilla'),
    _ColorName(Color(0xFFD1C6B4), 'Soft Amber'),
    _ColorName(Color(0xFFD1D2CA), 'Celeste'),
    _ColorName(Color(0xFFD1D2DD), 'Mischka'),
    _ColorName(Color(0xFFD1E231), 'Pear'),
    _ColorName(Color(0xFFD2691E), 'Hot Cinnamon'),
    _ColorName(Color(0xFFD27D46), 'Raw Sienna'),
    _ColorName(Color(0xFFD29EAA), 'Careys Pink'),
    _ColorName(Color(0xFFD2B48C), 'Tan'),
    _ColorName(Color(0xFFD2DA97), 'Deco'),
    _ColorName(Color(0xFFD2F6DE), 'Blue Romance'),
    _ColorName(Color(0xFFD2F8B0), 'Gossip'),
    _ColorName(Color(0xFFD3CBBA), 'Sisal'),
    _ColorName(Color(0xFFD3CDC5), 'Swirl'),
    _ColorName(Color(0xFFD47494), 'Charm'),
    _ColorName(Color(0xFFD4B6AF), 'Clam Shell'),
    _ColorName(Color(0xFFD4BF8D), 'Straw'),
    _ColorName(Color(0xFFD4C4A8), 'Akaroa'),
    _ColorName(Color(0xFFD4CD16), 'Bird Flower'),
    _ColorName(Color(0xFFD4D7D9), 'Iron'),
    _ColorName(Color(0xFFD4DFE2), 'Geyser'),
    _ColorName(Color(0xFFD4E2FC), 'Hawkes Blue'),
    _ColorName(Color(0xFFD54600), 'Grenadier'),
    _ColorName(Color(0xFFD591A4), 'Can Can'),
    _ColorName(Color(0xFFD59A6F), 'Whiskey'),
    _ColorName(Color(0xFFD5D195), 'Winter Hazel'),
    _ColorName(Color(0xFFD5F6E3), 'Granny Apple'),
    _ColorName(Color(0xFFD69188), 'My Pink'),
    _ColorName(Color(0xFFD6C562), 'Tacha'),
    _ColorName(Color(0xFFD6CEF6), 'Moon Raker'),
    _ColorName(Color(0xFFD6D6D1), 'Quill Gray'),
    _ColorName(Color(0xFFD6FFDB), 'Snowy Mint'),
    _ColorName(Color(0xFFD7837F), 'New York Pink'),
    _ColorName(Color(0xFFD7C498), 'Pavlova'),
    _ColorName(Color(0xFFD7D0FF), 'Fog'),
    _ColorName(Color(0xFFD84437), 'Valencia'),
    _ColorName(Color(0xFFD87C63), 'Japonica'),
    _ColorName(Color(0xFFD8BFD8), 'Thistle'),
    _ColorName(Color(0xFFD8C2D5), 'Maverick'),
    _ColorName(Color(0xFFD8FCFA), 'Foam'),
    _ColorName(Color(0xFFD94972), 'Cabaret'),
    _ColorName(Color(0xFFD99376), 'Burning Sand'),
    _ColorName(Color(0xFFD9B99B), 'Cameo'),
    _ColorName(Color(0xFFD9D6CF), 'Timberwolf'),
    _ColorName(Color(0xFFD9DCC1), 'Tana'),
    _ColorName(Color(0xFFD9E4F5), 'Link Water'),
    _ColorName(Color(0xFFD9F7FF), 'Mabel'),
    _ColorName(Color(0xFFDA3287), 'Cerise'),
    _ColorName(Color(0xFFDA5B38), 'Flame Pea'),
    _ColorName(Color(0xFFDA6304), 'Bamboo'),
    _ColorName(Color(0xFFDA6A41), 'Red Damask'),
    _ColorName(Color(0xFFDA70D6), 'Orchid'),
    _ColorName(Color(0xFFDA8A67), 'Copperfield'),
    _ColorName(Color(0xFFDAA520), 'Golden Grass'),
    _ColorName(Color(0xFFDAECD6), 'Zanah'),
    _ColorName(Color(0xFFDAF4F0), 'Iceberg'),
    _ColorName(Color(0xFFDAFAFF), 'Oyster Bay'),
    _ColorName(Color(0xFFDB5079), 'Cranberry'),
    _ColorName(Color(0xFFDB9690), 'Petite Orchid'),
    _ColorName(Color(0xFFDB995E), 'Di Serria'),
    _ColorName(Color(0xFFDBDBDB), 'Alto'),
    _ColorName(Color(0xFFDBFFF8), 'Frosted Mint'),
    _ColorName(Color(0xFFDC143C), 'Crimson'),
    _ColorName(Color(0xFFDC4333), 'Punch'),
    _ColorName(Color(0xFFDCB20C), 'Galliano'),
    _ColorName(Color(0xFFDCB4BC), 'Blossom'),
    _ColorName(Color(0xFFDCD747), 'Wattle'),
    _ColorName(Color(0xFFDCD9D2), 'Westar'),
    _ColorName(Color(0xFFDCDDCC), 'Moon Mist'),
    _ColorName(Color(0xFFDCEDB4), 'Caper'),
    _ColorName(Color(0xFFDCF0EA), 'Swans Down'),
    _ColorName(Color(0xFFDDD6D5), 'Swiss Coffee'),
    _ColorName(Color(0xFFDDF9F1), 'White Ice'),
    _ColorName(Color(0xFFDE3163), 'Cerise Red'),
    _ColorName(Color(0xFFDE6360), 'Roman'),
    _ColorName(Color(0xFFDEA681), 'Tumbleweed'),
    _ColorName(Color(0xFFDEBA13), 'Gold Tips'),
    _ColorName(Color(0xFFDEC196), 'Brandy'),
    _ColorName(Color(0xFFDECBC6), 'Wafer'),
    _ColorName(Color(0xFFDED4A4), 'Sapling'),
    _ColorName(Color(0xFFDED717), 'Barberry'),
    _ColorName(Color(0xFFDEE5C0), 'Beryl Green'),
    _ColorName(Color(0xFFDEF5FF), 'Pattens Blue'),
    _ColorName(Color(0xFFDF73FF), 'Heliotrope'),
    _ColorName(Color(0xFFDFBE6F), 'Apache'),
    _ColorName(Color(0xFFDFCD6F), 'Chenin'),
    _ColorName(Color(0xFFDFCFDB), 'Lola'),
    _ColorName(Color(0xFFDFECDA), 'Willow Brook'),
    _ColorName(Color(0xFFDFFF00), 'Chartreuse Yellow'),
    _ColorName(Color(0xFFE0B0FF), 'Mauve'),
    _ColorName(Color(0xFFE0B646), 'Anzac'),
    _ColorName(Color(0xFFE0B974), 'Harvest Gold'),
    _ColorName(Color(0xFFE0C095), 'Calico'),
    _ColorName(Color(0xFFE0FFFF), 'Baby Blue'),
    _ColorName(Color(0xFFE16865), 'Sunglo'),
    _ColorName(Color(0xFFE1BC64), 'Equator'),
    _ColorName(Color(0xFFE1C0C8), 'Pink Flare'),
    _ColorName(Color(0xFFE1E6D6), 'Periglacial Blue'),
    _ColorName(Color(0xFFE1EAD4), 'Kidnapper'),
    _ColorName(Color(0xFFE1F6E8), 'Tara'),
    _ColorName(Color(0xFFE25465), 'Mandy'),
    _ColorName(Color(0xFFE2725B), 'Terracotta'),
    _ColorName(Color(0xFFE28913), 'Golden Bell'),
    _ColorName(Color(0xFFE292C0), 'Shocking'),
    _ColorName(Color(0xFFE29418), 'Dixie'),
    _ColorName(Color(0xFFE29CD2), 'Light Orchid'),
    _ColorName(Color(0xFFE2D8ED), 'Snuff'),
    _ColorName(Color(0xFFE2EBED), 'Mystic'),
    _ColorName(Color(0xFFE2F3EC), 'Apple Green'),
    _ColorName(Color(0xFFE30B5C), 'Razzmatazz'),
    _ColorName(Color(0xFFE32636), 'Alizarin Crimson'),
    _ColorName(Color(0xFFE34234), 'Cinnabar'),
    _ColorName(Color(0xFFE3BEBE), 'Cavern Pink'),
    _ColorName(Color(0xFFE3F5E1), 'Peppermint'),
    _ColorName(Color(0xFFE3F988), 'Mindaro'),
    _ColorName(Color(0xFFE47698), 'Deep Blush'),
    _ColorName(Color(0xFFE49B0F), 'Gamboge'),
    _ColorName(Color(0xFFE4C2D5), 'Melanie'),
    _ColorName(Color(0xFFE4CFDE), 'Twilight'),
    _ColorName(Color(0xFFE4D1C0), 'Bone'),
    _ColorName(Color(0xFFE4D422), 'Sunflower'),
    _ColorName(Color(0xFFE4D5B7), 'Grain Brown'),
    _ColorName(Color(0xFFE4D69B), 'Zombie'),
    _ColorName(Color(0xFFE4F6E7), 'Frostee'),
    _ColorName(Color(0xFFE4FFD1), 'Snow Flurry'),
    _ColorName(Color(0xFFE52B50), 'Amaranth'),
    _ColorName(Color(0xFFE5841B), 'Zest'),
    _ColorName(Color(0xFFE5CCC9), 'Dust Storm'),
    _ColorName(Color(0xFFE5D7BD), 'Stark White'),
    _ColorName(Color(0xFFE5D8AF), 'Hampton'),
    _ColorName(Color(0xFFE5E0E1), 'Bon Jour'),
    _ColorName(Color(0xFFE5E5E5), 'Mercury'),
    _ColorName(Color(0xFFE5F9F6), 'Polar'),
    _ColorName(Color(0xFFE64E03), 'Trinidad'),
    _ColorName(Color(0xFFE6BE8A), 'Gold Sand'),
    _ColorName(Color(0xFFE6BEA5), 'Cashmere'),
    _ColorName(Color(0xFFE6D7B9), 'Double Spanish White'),
    _ColorName(Color(0xFFE6E4D4), 'Satin Linen'),
    _ColorName(Color(0xFFE6F2EA), 'Harp'),
    _ColorName(Color(0xFFE6F8F3), 'Off Green'),
    _ColorName(Color(0xFFE6FFE9), 'Hint of Green'),
    _ColorName(Color(0xFFE6FFFF), 'Tranquil'),
    _ColorName(Color(0xFFE77200), 'Mango Tango'),
    _ColorName(Color(0xFFE7730A), 'Christine'),
    _ColorName(Color(0xFFE79F8C), "Tony's Pink"),
    _ColorName(Color(0xFFE79FC4), 'Kobi'),
    _ColorName(Color(0xFFE7BCB4), 'Rose Fog'),
    _ColorName(Color(0xFFE7BF05), 'Corn'),
    _ColorName(Color(0xFFE7CD8C), 'Putty'),
    _ColorName(Color(0xFFE7ECE6), 'Gray Nurse'),
    _ColorName(Color(0xFFE7F8FF), 'Lily White'),
    _ColorName(Color(0xFFE7FEFF), 'Bubbles'),
    _ColorName(Color(0xFFE89928), 'Fire Bush'),
    _ColorName(Color(0xFFE8B9B3), 'Shilo'),
    _ColorName(Color(0xFFE8E0D5), 'Pearl Bush'),
    _ColorName(Color(0xFFE8EBE0), 'Green White'),
    _ColorName(Color(0xFFE8F1D4), 'Chrome White'),
    _ColorName(Color(0xFFE8F2EB), 'Gin'),
    _ColorName(Color(0xFFE8F5F2), 'Aqua Squeeze'),
    _ColorName(Color(0xFFE96E00), 'Clementine'),
    _ColorName(Color(0xFFE97451), 'Burnt Sienna'),
    _ColorName(Color(0xFFE97C07), 'Tahiti Gold'),
    _ColorName(Color(0xFFE9CECD), 'Oyster Pink'),
    _ColorName(Color(0xFFE9D75A), 'Confetti'),
    _ColorName(Color(0xFFE9E3E3), 'Ebb'),
    _ColorName(Color(0xFFE9F8ED), 'Ottoman'),
    _ColorName(Color(0xFFE9FFFD), 'Clear Day'),
    _ColorName(Color(0xFFEA88A8), 'Carissma'),
    _ColorName(Color(0xFFEAAE69), 'Porsche'),
    _ColorName(Color(0xFFEAB33B), 'Tulip Tree'),
    _ColorName(Color(0xFFEAC674), 'Rob Roy'),
    _ColorName(Color(0xFFEADAB8), 'Raffia'),
    _ColorName(Color(0xFFEAE8D4), 'White Rock'),
    _ColorName(Color(0xFFEAF6EE), 'Panache'),
    _ColorName(Color(0xFFEAF6FF), 'Solitude'),
    _ColorName(Color(0xFFEAF9F5), 'Aqua Spring'),
    _ColorName(Color(0xFFEAFFFE), 'Dew'),
    _ColorName(Color(0xFFEB9373), 'Apricot'),
    _ColorName(Color(0xFFEBC2AF), 'Zinnwaldite'),
    _ColorName(Color(0xFFECA927), 'Fuel Yellow'),
    _ColorName(Color(0xFFECC54E), 'Ronchi'),
    _ColorName(Color(0xFFECC7EE), 'French Lilac'),
    _ColorName(Color(0xFFECCDB9), 'Just Right'),
    _ColorName(Color(0xFFECE090), 'Wild Rice'),
    _ColorName(Color(0xFFECEBBD), 'Fall Green'),
    _ColorName(Color(0xFFECEBCE), 'Aths Special'),
    _ColorName(Color(0xFFECF245), 'Starship'),
    _ColorName(Color(0xFFED0A3F), 'Red Ribbon'),
    _ColorName(Color(0xFFED7A1C), 'Tango'),
    _ColorName(Color(0xFFED9121), 'Carrot Orange'),
    _ColorName(Color(0xFFED989E), 'Sea Pink'),
    _ColorName(Color(0xFFEDB381), 'Tacao'),
    _ColorName(Color(0xFFEDC9AF), 'Desert Sand'),
    _ColorName(Color(0xFFEDCDAB), 'Pancho'),
    _ColorName(Color(0xFFEDDCB1), 'Chamois'),
    _ColorName(Color(0xFFEDEA99), 'Primrose'),
    _ColorName(Color(0xFFEDF5DD), 'Frost'),
    _ColorName(Color(0xFFEDF5F5), 'Aqua Haze'),
    _ColorName(Color(0xFFEDF6FF), 'Zumthor'),
    _ColorName(Color(0xFFEDF9F1), 'Narvik'),
    _ColorName(Color(0xFFEDFC84), 'Honeysuckle'),
    _ColorName(Color(0xFFEE82EE), 'Lavender Magenta'),
    _ColorName(Color(0xFFEEC1BE), 'Beauty Bush'),
    _ColorName(Color(0xFFEED794), 'Chalky'),
    _ColorName(Color(0xFFEED9C4), 'Almond'),
    _ColorName(Color(0xFFEEDC82), 'Flax'),
    _ColorName(Color(0xFFEEDEDA), 'Bizarre'),
    _ColorName(Color(0xFFEEE3AD), 'Double Colonial White'),
    _ColorName(Color(0xFFEEEEE8), 'Cararra'),
    _ColorName(Color(0xFFEEEF78), 'Manz'),
    _ColorName(Color(0xFFEEF0C8), 'Tahuna Sands'),
    _ColorName(Color(0xFFEEF0F3), 'Athens Gray'),
    _ColorName(Color(0xFFEEF3C3), 'Tusk'),
    _ColorName(Color(0xFFEEF4DE), 'Loafer'),
    _ColorName(Color(0xFFEEF6F7), 'Catskill White'),
    _ColorName(Color(0xFFEEFDFF), 'Twilight Blue'),
    _ColorName(Color(0xFFEEFF9A), 'Jonquil'),
    _ColorName(Color(0xFFEEFFE2), 'Rice Flower'),
    _ColorName(Color(0xFFEF863F), 'Jaffa'),
    _ColorName(Color(0xFFEFEFEF), 'Gallery'),
    _ColorName(Color(0xFFEFF2F3), 'Porcelain'),
    _ColorName(Color(0xFFF091A9), 'Mauvelous'),
    _ColorName(Color(0xFFF0D52D), 'Golden Dream'),
    _ColorName(Color(0xFFF0DB7D), 'Golden Sand'),
    _ColorName(Color(0xFFF0DC82), 'Buff'),
    _ColorName(Color(0xFFF0E2EC), 'Prim'),
    _ColorName(Color(0xFFF0E68C), 'Khaki'),
    _ColorName(Color(0xFFF0EEFD), 'Selago'),
    _ColorName(Color(0xFFF0EEFF), 'Titan White'),
    _ColorName(Color(0xFFF0F8FF), 'Alice Blue'),
    _ColorName(Color(0xFFF0FCEA), 'Feta'),
    _ColorName(Color(0xFFF18200), 'Gold Drop'),
    _ColorName(Color(0xFFF19BAB), 'Wewak'),
    _ColorName(Color(0xFFF1E788), 'Sahara Sand'),
    _ColorName(Color(0xFFF1E9D2), 'Parchment'),
    _ColorName(Color(0xFFF1E9FF), 'Blue Chalk'),
    _ColorName(Color(0xFFF1EEC1), 'Mint Julep'),
    _ColorName(Color(0xFFF1F1F1), 'Seashell'),
    _ColorName(Color(0xFFF1F7F2), 'Saltpan'),
    _ColorName(Color(0xFFF1FFAD), 'Tidal'),
    _ColorName(Color(0xFFF1FFC8), 'Chiffon'),
    _ColorName(Color(0xFFF2552A), 'Flamingo'),
    _ColorName(Color(0xFFF28500), 'Tangerine'),
    _ColorName(Color(0xFFF2C3B2), "Mandy's Pink"),
    _ColorName(Color(0xFFF2F2F2), 'Concrete'),
    _ColorName(Color(0xFFF2FAFA), 'Black Squeeze'),
    _ColorName(Color(0xFFF34723), 'Pomegranate'),
    _ColorName(Color(0xFFF3AD16), 'Buttercup'),
    _ColorName(Color(0xFFF3D69D), 'New Orleans'),
    _ColorName(Color(0xFFF3D9DF), 'Vanilla Ice'),
    _ColorName(Color(0xFFF3E7BB), 'Sidecar'),
    _ColorName(Color(0xFFF3E9E5), 'Dawn Pink'),
    _ColorName(Color(0xFFF3EDCF), 'Wheatfield'),
    _ColorName(Color(0xFFF3FB62), 'Canary'),
    _ColorName(Color(0xFFF3FBD4), 'Orinoco'),
    _ColorName(Color(0xFFF3FFD8), 'Carla'),
    _ColorName(Color(0xFFF400A1), 'Hollywood Cerise'),
    _ColorName(Color(0xFFF4A460), 'Sandy brown'),
    _ColorName(Color(0xFFF4C430), 'Saffron'),
    _ColorName(Color(0xFFF4D81C), 'Ripe Lemon'),
    _ColorName(Color(0xFFF4EBD3), 'Janna'),
    _ColorName(Color(0xFFF4F2EE), 'Pampas'),
    _ColorName(Color(0xFFF4F4F4), 'Wild Sand'),
    _ColorName(Color(0xFFF4F8FF), 'Zircon'),
    _ColorName(Color(0xFFF57584), 'Froly'),
    _ColorName(Color(0xFFF5C85C), 'Cream Can'),
    _ColorName(Color(0xFFF5C999), 'Manhattan'),
    _ColorName(Color(0xFFF5D5A0), 'Maize'),
    _ColorName(Color(0xFFF5DEB3), 'Wheat'),
    _ColorName(Color(0xFFF5E7A2), 'Sandwisp'),
    _ColorName(Color(0xFFF5E7E2), 'Pot Pourri'),
    _ColorName(Color(0xFFF5E9D3), 'Albescent White'),
    _ColorName(Color(0xFFF5EDEF), 'Soft Peach'),
    _ColorName(Color(0xFFF5F3E5), 'Ecru White'),
    _ColorName(Color(0xFFF5F5DC), 'Beige'),
    _ColorName(Color(0xFFF5FB3D), 'Golden Fizz'),
    _ColorName(Color(0xFFF5FFBE), 'Australian Mint'),
    _ColorName(Color(0xFFF64A8A), 'French Rose'),
    _ColorName(Color(0xFFF653A6), 'Brilliant Rose'),
    _ColorName(Color(0xFFF6A4C9), 'Illusion'),
    _ColorName(Color(0xFFF6F0E6), 'Merino'),
    _ColorName(Color(0xFFF6F7F7), 'Black Haze'),
    _ColorName(Color(0xFFF6FFDC), 'Spring Sun'),
    _ColorName(Color(0xFFF7468A), 'Violet Red'),
    _ColorName(Color(0xFFF77703), 'Chilean Fire'),
    _ColorName(Color(0xFFF77FBE), 'Persian Pink'),
    _ColorName(Color(0xFFF7B668), 'Rajah'),
    _ColorName(Color(0xFFF7C8DA), 'Azalea'),
    _ColorName(Color(0xFFF7DBE6), 'We Peep'),
    _ColorName(Color(0xFFF7F2E1), 'Quarter Spanish White'),
    _ColorName(Color(0xFFF7F5FA), 'Whisper'),
    _ColorName(Color(0xFFF7FAF7), 'Snow Drift'),
    _ColorName(Color(0xFFF8B853), 'Casablanca'),
    _ColorName(Color(0xFFF8C3DF), 'Chantilly'),
    _ColorName(Color(0xFFF8D9E9), 'Cherub'),
    _ColorName(Color(0xFFF8DB9D), 'Marzipan'),
    _ColorName(Color(0xFFF8DD5C), 'Energy Yellow'),
    _ColorName(Color(0xFFF8E4BF), 'Givry'),
    _ColorName(Color(0xFFF8F0E8), 'White Linen'),
    _ColorName(Color(0xFFF8F4FF), 'Magnolia'),
    _ColorName(Color(0xFFF8F6F1), 'Spring Wood'),
    _ColorName(Color(0xFFF8F7DC), 'Coconut Cream'),
    _ColorName(Color(0xFFF8F7FC), 'White Lilac'),
    _ColorName(Color(0xFFF8F8F7), 'Desert Storm'),
    _ColorName(Color(0xFFF8F99C), 'Texas'),
    _ColorName(Color(0xFFF8FACD), 'Corn Field'),
    _ColorName(Color(0xFFF8FDD3), 'Mimosa'),
    _ColorName(Color(0xFFF95A61), 'Carnation'),
    _ColorName(Color(0xFFF9BF58), 'Saffron Mango'),
    _ColorName(Color(0xFFF9E0ED), 'Carousel Pink'),
    _ColorName(Color(0xFFF9E4BC), 'Dairy Cream'),
    _ColorName(Color(0xFFF9E663), 'Portica'),
    _ColorName(Color(0xFFF9EAF3), 'Amour'),
    _ColorName(Color(0xFFF9F8E4), 'Rum Swizzle'),
    _ColorName(Color(0xFFF9FF8B), 'Dolly'),
    _ColorName(Color(0xFFF9FFF6), 'Sugar Cane'),
    _ColorName(Color(0xFFFA7814), 'Ecstasy'),
    _ColorName(Color(0xFFFA9D5A), 'Tan Hide'),
    _ColorName(Color(0xFFFAD3A2), 'Corvette'),
    _ColorName(Color(0xFFFADFAD), 'Peach Yellow'),
    _ColorName(Color(0xFFFAE600), 'Turbo'),
    _ColorName(Color(0xFFFAEAB9), 'Astra'),
    _ColorName(Color(0xFFFAECCC), 'Champagne'),
    _ColorName(Color(0xFFFAF0E6), 'Linen'),
    _ColorName(Color(0xFFFAF3F0), 'Fantasy'),
    _ColorName(Color(0xFFFAF7D6), 'Citrine White'),
    _ColorName(Color(0xFFFAFAFA), 'Alabaster'),
    _ColorName(Color(0xFFFAFDE4), 'Hint of Yellow'),
    _ColorName(Color(0xFFFAFFA4), 'Milan'),
    _ColorName(Color(0xFFFB607F), 'Brink Pink'),
    _ColorName(Color(0xFFFB8989), 'Geraldine'),
    _ColorName(Color(0xFFFBA0E3), 'Lavender Rose'),
    _ColorName(Color(0xFFFBA129), 'Sea Buckthorn'),
    _ColorName(Color(0xFFFBAC13), 'Sun'),
    _ColorName(Color(0xFFFBAED2), 'Lavender Pink'),
    _ColorName(Color(0xFFFBB2A3), 'Rose Bud'),
    _ColorName(Color(0xFFFBBEDA), 'Cupid'),
    _ColorName(Color(0xFFFBCCE7), 'Classic Rose'),
    _ColorName(Color(0xFFFBCEB1), 'Apricot Peach'),
    _ColorName(Color(0xFFFBE7B2), 'Banana Mania'),
    _ColorName(Color(0xFFFBE870), 'Marigold Yellow'),
    _ColorName(Color(0xFFFBE96C), 'Festival'),
    _ColorName(Color(0xFFFBEA8C), 'Sweet Corn'),
    _ColorName(Color(0xFFFBEC5D), 'Candy Corn'),
    _ColorName(Color(0xFFFBF9F9), 'Hint of Red'),
    _ColorName(Color(0xFFFBFFBA), 'Shalimar'),
    _ColorName(Color(0xFFFC0FC0), 'Shocking Pink'),
    _ColorName(Color(0xFFFC80A5), 'Tickle Me Pink'),
    _ColorName(Color(0xFFFC9C1D), 'Tree Poppy'),
    _ColorName(Color(0xFFFCC01E), 'Lightning Yellow'),
    _ColorName(Color(0xFFFCD667), 'Goldenrod'),
    _ColorName(Color(0xFFFCD917), 'Candlelight'),
    _ColorName(Color(0xFFFCDA98), 'Cherokee'),
    _ColorName(Color(0xFFFCF4D0), 'Double Pearl Lusta'),
    _ColorName(Color(0xFFFCF4DC), 'Pearl Lusta'),
    _ColorName(Color(0xFFFCF8F7), 'Vista White'),
    _ColorName(Color(0xFFFCFBF3), 'Bianca'),
    _ColorName(Color(0xFFFCFEDA), 'Moon Glow'),
    _ColorName(Color(0xFFFCFFE7), 'China Ivory'),
    _ColorName(Color(0xFFFCFFF9), 'Ceramic'),
    _ColorName(Color(0xFFFD0E35), 'Torch Red'),
    _ColorName(Color(0xFFFD5B78), 'Wild Watermelon'),
    _ColorName(Color(0xFFFD7B33), 'Crusta'),
    _ColorName(Color(0xFFFD7C07), 'Sorbus'),
    _ColorName(Color(0xFFFD9FA2), 'Sweet Pink'),
    _ColorName(Color(0xFFFDD5B1), 'Light Apricot'),
    _ColorName(Color(0xFFFDD7E4), 'Pig Pink'),
    _ColorName(Color(0xFFFDE1DC), 'Cinderella'),
    _ColorName(Color(0xFFFDE295), 'Golden Glow'),
    _ColorName(Color(0xFFFDE910), 'Lemon'),
    _ColorName(Color(0xFFFDF5E6), 'Old Lace'),
    _ColorName(Color(0xFFFDF6D3), 'Half Colonial White'),
    _ColorName(Color(0xFFFDF7AD), 'Drover'),
    _ColorName(Color(0xFFFDFEB8), 'Pale Prim'),
    _ColorName(Color(0xFFFDFFD5), 'Cumulus'),
    _ColorName(Color(0xFFFE28A2), 'Persian Rose'),
    _ColorName(Color(0xFFFE4C40), 'Sunset Orange'),
    _ColorName(Color(0xFFFE6F5E), 'Bittersweet'),
    _ColorName(Color(0xFFFE9D04), 'California'),
    _ColorName(Color(0xFFFEA904), 'Yellow Sea'),
    _ColorName(Color(0xFFFEBAAD), 'Melon'),
    _ColorName(Color(0xFFFED33C), 'Bright Sun'),
    _ColorName(Color(0xFFFED85D), 'Dandelion'),
    _ColorName(Color(0xFFFEDB8D), 'Salomie'),
    _ColorName(Color(0xFFFEE5AC), 'Cape Honey'),
    _ColorName(Color(0xFFFEEBF3), 'Remy'),
    _ColorName(Color(0xFFFEEFCE), 'Oasis'),
    _ColorName(Color(0xFFFEF0EC), 'Bridesmaid'),
    _ColorName(Color(0xFFFEF2C7), 'Beeswax'),
    _ColorName(Color(0xFFFEF3D8), 'Bleach White'),
    _ColorName(Color(0xFFFEF4CC), 'Pipi'),
    _ColorName(Color(0xFFFEF4DB), 'Half Spanish White'),
    _ColorName(Color(0xFFFEF4F8), 'Wisp Pink'),
    _ColorName(Color(0xFFFEF5F1), 'Provincial Pink'),
    _ColorName(Color(0xFFFEF7DE), 'Half Dutch White'),
    _ColorName(Color(0xFFFEF8E2), 'Solitaire'),
    _ColorName(Color(0xFFFEF8FF), 'White Pointer'),
    _ColorName(Color(0xFFFEF9E3), 'Off Yellow'),
    _ColorName(Color(0xFFFEFCED), 'Orange White'),
    _ColorName(Color(0xFFFF0000), 'Red'),
    _ColorName(Color(0xFFFF007F), 'Rose'),
    _ColorName(Color(0xFFFF00CC), 'Purple Pizzazz'),
    _ColorName(Color(0xFFFF00FF), 'Magenta / Fuchsia'),
    _ColorName(Color(0xFFFF2400), 'Scarlet'),
    _ColorName(Color(0xFFFF3399), 'Wild Strawberry'),
    _ColorName(Color(0xFFFF33CC), 'Razzle Dazzle Rose'),
    _ColorName(Color(0xFFFF355E), 'Radical Red'),
    _ColorName(Color(0xFFFF3F34), 'Red Orange'),
    _ColorName(Color(0xFFFF4040), 'Coral Red'),
    _ColorName(Color(0xFFFF4D00), 'Vermilion'),
    _ColorName(Color(0xFFFF4F00), 'International Orange'),
    _ColorName(Color(0xFFFF6037), 'Outrageous Orange'),
    _ColorName(Color(0xFFFF6600), 'Blaze Orange'),
    _ColorName(Color(0xFFFF66FF), 'Pink Flamingo'),
    _ColorName(Color(0xFFFF681F), 'Orange'),
    _ColorName(Color(0xFFFF69B4), 'Hot Pink'),
    _ColorName(Color(0xFFFF6B53), 'Persimmon'),
    _ColorName(Color(0xFFFF6FFF), 'Blush Pink'),
    _ColorName(Color(0xFFFF7034), 'Burning Orange'),
    _ColorName(Color(0xFFFF7518), 'Pumpkin'),
    _ColorName(Color(0xFFFF7D07), 'Flamenco'),
    _ColorName(Color(0xFFFF7F00), 'Flush Orange'),
    _ColorName(Color(0xFFFF7F50), 'Coral'),
    _ColorName(Color(0xFFFF8C69), 'Salmon'),
    _ColorName(Color(0xFFFF9000), 'Pizazz'),
    _ColorName(Color(0xFFFF910F), 'West Side'),
    _ColorName(Color(0xFFFF91A4), 'Pink Salmon'),
    _ColorName(Color(0xFFFF9933), 'Neon Carrot'),
    _ColorName(Color(0xFFFF9966), 'Atomic Tangerine'),
    _ColorName(Color(0xFFFF9980), 'Vivid Tangerine'),
    _ColorName(Color(0xFFFF9E2C), 'Sunshade'),
    _ColorName(Color(0xFFFFA000), 'Orange Peel'),
    _ColorName(Color(0xFFFFA194), 'Mona Lisa'),
    _ColorName(Color(0xFFFFA500), 'Web Orange'),
    _ColorName(Color(0xFFFFA6C9), 'Carnation Pink'),
    _ColorName(Color(0xFFFFAB81), 'Hit Pink'),
    _ColorName(Color(0xFFFFAE42), 'Yellow Orange'),
    _ColorName(Color(0xFFFFB0AC), 'Cornflower Lilac'),
    _ColorName(Color(0xFFFFB1B3), 'Sundown'),
    _ColorName(Color(0xFFFFB31F), 'My Sin'),
    _ColorName(Color(0xFFFFB555), 'Texas Rose'),
    _ColorName(Color(0xFFFFB7D5), 'Cotton Candy'),
    _ColorName(Color(0xFFFFB97B), 'Macaroni and Cheese'),
    _ColorName(Color(0xFFFFBA00), 'Selective Yellow'),
    _ColorName(Color(0xFFFFBD5F), 'Koromiko'),
    _ColorName(Color(0xFFFFBF00), 'Amber'),
    _ColorName(Color(0xFFFFC0A8), 'Wax Flower'),
    _ColorName(Color(0xFFFFC0CB), 'Pink'),
    _ColorName(Color(0xFFFFC3C0), 'Your Pink'),
    _ColorName(Color(0xFFFFC901), 'Supernova'),
    _ColorName(Color(0xFFFFCBA4), 'Flesh'),
    _ColorName(Color(0xFFFFCC33), 'Sunglow'),
    _ColorName(Color(0xFFFFCC5C), 'Golden Tainoi'),
    _ColorName(Color(0xFFFFCC99), 'Peach Orange'),
    _ColorName(Color(0xFFFFCD8C), 'Chardonnay'),
    _ColorName(Color(0xFFFFD1DC), 'Pastel Pink'),
    _ColorName(Color(0xFFFFD2B7), 'Romantic'),
    _ColorName(Color(0xFFFFD38C), 'Grandis'),
    _ColorName(Color(0xFFFFD700), 'Gold'),
    _ColorName(Color(0xFFFFD801), 'School bus Yellow'),
    _ColorName(Color(0xFFFFD8D9), 'Cosmos'),
    _ColorName(Color(0xFFFFDB58), 'Mustard'),
    _ColorName(Color(0xFFFFDCD6), 'Peach Schnapps'),
    _ColorName(Color(0xFFFFDDAF), 'Caramel'),
    _ColorName(Color(0xFFFFDDCD), 'Tuft Bush'),
    _ColorName(Color(0xFFFFDDCF), 'Watusi'),
    _ColorName(Color(0xFFFFDDF4), 'Pink Lace'),
    _ColorName(Color(0xFFFFDEAD), 'Navajo White'),
    _ColorName(Color(0xFFFFDEB3), 'Frangipani'),
    _ColorName(Color(0xFFFFE1DF), 'Pippin'),
    _ColorName(Color(0xFFFFE1F2), 'Pale Rose'),
    _ColorName(Color(0xFFFFE2C5), 'Negroni'),
    _ColorName(Color(0xFFFFE5A0), 'Cream Brulee'),
    _ColorName(Color(0xFFFFE5B4), 'Peach'),
    _ColorName(Color(0xFFFFE6C7), 'Tequila'),
    _ColorName(Color(0xFFFFE772), 'Kournikova'),
    _ColorName(Color(0xFFFFEAC8), 'Sandy Beach'),
    _ColorName(Color(0xFFFFEAD4), 'Karry'),
    _ColorName(Color(0xFFFFEC13), 'Broom'),
    _ColorName(Color(0xFFFFEDBC), 'Colonial White'),
    _ColorName(Color(0xFFFFEED8), 'Derby'),
    _ColorName(Color(0xFFFFEFA1), 'Vis Vis'),
    _ColorName(Color(0xFFFFEFC1), 'Egg White'),
    _ColorName(Color(0xFFFFEFD5), 'Papaya Whip'),
    _ColorName(Color(0xFFFFEFEC), 'Fair Pink'),
    _ColorName(Color(0xFFFFF0DB), 'Peach Cream'),
    _ColorName(Color(0xFFFFF0F5), 'Lavender blush'),
    _ColorName(Color(0xFFFFF14F), 'Gorse'),
    _ColorName(Color(0xFFFFF1B5), 'Buttermilk'),
    _ColorName(Color(0xFFFFF1D8), 'Pink Lady'),
    _ColorName(Color(0xFFFFF1EE), 'Forget Me Not'),
    _ColorName(Color(0xFFFFF1F9), 'Tutu'),
    _ColorName(Color(0xFFFFF39D), 'Picasso'),
    _ColorName(Color(0xFFFFF3F1), 'Chardon'),
    _ColorName(Color(0xFFFFF46E), 'Paris Daisy'),
    _ColorName(Color(0xFFFFF4CE), 'Barley White'),
    _ColorName(Color(0xFFFFF4DD), 'Egg Sour'),
    _ColorName(Color(0xFFFFF4E0), 'Sazerac'),
    _ColorName(Color(0xFFFFF4E8), 'Serenade'),
    _ColorName(Color(0xFFFFF4F3), 'Chablis'),
    _ColorName(Color(0xFFFFF5EE), 'Seashell Peach'),
    _ColorName(Color(0xFFFFF5F3), 'Sauvignon'),
    _ColorName(Color(0xFFFFF6D4), 'Milk Punch'),
    _ColorName(Color(0xFFFFF6DF), 'Varden'),
    _ColorName(Color(0xFFFFF6F5), 'Rose White'),
    _ColorName(Color(0xFFFFF8D1), 'Baja White'),
    _ColorName(Color(0xFFFFF9E2), 'Gin Fizz'),
    _ColorName(Color(0xFFFFF9E6), 'Early Dawn'),
    _ColorName(Color(0xFFFFFACD), 'Lemon Chiffon'),
    _ColorName(Color(0xFFFFFAF4), 'Bridal Heath'),
    _ColorName(Color(0xFFFFFBDC), 'Scotch Mist'),
    _ColorName(Color(0xFFFFFBF9), 'Soapstone'),
    _ColorName(Color(0xFFFFFC99), 'Witch Haze'),
    _ColorName(Color(0xFFFFFCEA), 'Buttery White'),
    _ColorName(Color(0xFFFFFCEE), 'Island Spice'),
    _ColorName(Color(0xFFFFFDD0), 'Cream'),
    _ColorName(Color(0xFFFFFDE6), 'Chilean Heath'),
    _ColorName(Color(0xFFFFFDE8), 'Travertine'),
    _ColorName(Color(0xFFFFFDF3), 'Orchid White'),
    _ColorName(Color(0xFFFFFDF4), 'Quarter Pearl Lusta'),
    _ColorName(Color(0xFFFFFEE1), 'Half and Half'),
    _ColorName(Color(0xFFFFFEEC), 'Apricot White'),
    _ColorName(Color(0xFFFFFEF0), 'Rice Cake'),
    _ColorName(Color(0xFFFFFEF6), 'Black White'),
    _ColorName(Color(0xFFFFFEFD), 'Romance'),
    _ColorName(Color(0xFFFFFF00), 'Yellow'),
    _ColorName(Color(0xFFFFFF66), 'Laser Lemon'),
    _ColorName(Color(0xFFFFFF99), 'Pale Canary'),
    _ColorName(Color(0xFFFFFFB4), 'Portafino'),
    _ColorName(Color(0xFFFFFFF0), 'Ivory'),
    _ColorName(Color(0xFFFFFFFF), 'White'),
  ];
}

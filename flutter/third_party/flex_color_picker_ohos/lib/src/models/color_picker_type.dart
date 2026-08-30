/// Enum that represents the different offered color picker types.
enum ColorPickerType {
  /// A color picker that contains both primary and accent Material colors.
  both,

  /// A color picker that contain the primary Material color swatches.
  primary,

  /// A color picker that contain the accent Material color swatches.
  accent,

  /// A color picker that offers black and white and their very near shades
  /// as color swatches.
  bw,

  /// A color picker that shows custom provided colors and their material like
  /// swatches and a custom name for each color swatch.
  custom,

  /// A secondary color picker that shows custom provided colors and their
  /// material like swatches and a custom name for each color swatch.
  customSecondary,

  /// A HSV color wheel picker that can select any color.
  wheel,
}

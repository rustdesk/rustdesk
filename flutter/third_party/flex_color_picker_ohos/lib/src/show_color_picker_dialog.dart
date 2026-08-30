// ignore_for_file: comment_references

part of 'color_picker.dart';

/// Define a color picker, show its dialog and wait for it to return a color.
///
Future<Color> showColorPickerDialog(
  /// Required build context for the dialog
  BuildContext context,

  /// The active color selection when the color picker dialog is created.
  Color color, {
  /// A [ColorPickerType] to bool map. Defines which pickers are enabled in the
  /// color picker's sliding selector and thus available as color pickers.
  ///
  /// Available options are based on the [ColorPickerType] enum that
  /// includes values `both`, `primary`, `accent`, `bw`, `custom` and `wheel`.
  ///
  /// By default, a map that sets primary and accent pickers to true, and
  /// other pickers to false, is used.
  ///
  /// To modify key-value enable/disable pairs, you only have to provide values
  /// for the pairs you want to change from their default value. Any missing
  /// key-value pair in the provided map will keep their default value.
  Map<ColorPickerType, bool> pickersEnabled = const <ColorPickerType, bool>{
    ColorPickerType.both: false,
    ColorPickerType.primary: true,
    ColorPickerType.accent: true,
    ColorPickerType.bw: false,
    ColorPickerType.custom: false,
    ColorPickerType.wheel: false,
  },

  /// Set to true to allow selection of color swatch shades.
  ///
  /// If false, only the main color from a swatch is shown and can be selected.
  /// This is index [500] for Material primary colors and index [200] for accent
  /// colors. On the Wheel, only the selected color is shown there is no
  /// color related color swatch of the selected color shown.
  ///
  /// Defaults to true.
  bool enableShadesSelection = true,

  /// There is an extra index [850] used only by grey Material color in Flutter.
  /// If you want to include it in the grey color shades selection, then set
  /// this property to true.
  ///
  /// Defaults to false.
  bool includeIndex850 = false,

  /// Set to true to allow selection of color tone from a tonal palette.
  ///
  /// When set to true, the ColorPicker will use Material 3 color utilities
  /// to compute a tonal palette for the selected color, allowing you to
  /// select a color tone from the Tonal Palette for the selected color.
  ///
  /// For more info on Material 3 Color system, see:
  /// https://m3.material.io/styles/color/the-color-system/key-colors-tones
  ///
  /// The picker item size for tonal palette color indicator items is
  /// 10/13 the size of defined width and height. This is done in order to
  /// as far as possible try to match the width of the Primary Material Swatch
  /// items total width, it has 10 colors, the M3 tonal palette has 13 colors.
  /// The idea is try to match their width when they are both shown.
  ///
  /// Defaults to false.
  final bool enableTonalPalette = false,

  /// Whether the tonal palette uses a fixed minimum chroma value for all
  /// tones or if it uses the chroma value of the selected color.
  ///
  /// Prior to version 3.6.0 the tonal palette used minimum chroma value of 48
  /// or chroma of the selected color. This was the default primary tonal
  /// palette behavior in Flutter's ColorScheme.fromSeed method before
  /// Flutter version 3.22.0.
  ///
  /// Starting from version 3.6.0 the FlexColorPicker creates a HCT color space
  /// tonal palette using whatever hue and chroma is in the selected color.
  ///
  /// If you for some reason want to use the old behavior, set this property to
  /// true. This will make the tonal palette use the fixed minimum chroma value
  /// of 48 for all tones.
  ///
  /// Defaults to false.
  final bool tonalPaletteFixedMinChroma = false,

  /// How much space should be occupied in the color picker's vertical axis.
  ///
  /// After allocating space to children, in the ColorPicker column there
  /// might be some remaining free space. This value controls whether to
  /// maximize or minimize the amount of
  /// free space, subject to the incoming layout constraints.
  ///
  /// If some children have a non-zero flex factors (and none have a fit of
  /// [FlexFit.loose]), they will expand to consume all the available space and
  /// there will be no remaining free space to maximize or minimize, making this
  /// value irrelevant to the final layout.
  ///
  /// Defaults to `MainAxisSize.max` like Column does as well.
  final MainAxisSize mainAxisSize = MainAxisSize.max,

  /// Cross axis alignment used to layout the main content of the
  /// color picker in its column layout.
  ///
  /// Defaults to CrossAxisAlignment.center.
  CrossAxisAlignment crossAxisAlignment = CrossAxisAlignment.center,

  /// Padding around the entire color picker content.
  ///
  /// Defaults to const EdgeInsets.all(16).
  EdgeInsetsGeometry padding = const EdgeInsets.all(16),

  /// Vertical spacing between items in the color picker column.
  ///
  /// Defaults to 8 dp. Must be from 0 to 300 dp.
  double columnSpacing = 8,

  /// Vertical spacing below the top toolbar header and action buttons.
  ///
  /// If not defined, defaults to [columnSpacing].
  /// Must be null or from 0 to 300 dp.
  final double? toolbarSpacing,

  /// Vertical spacing below the material 2 based color shades palette.
  ///
  /// If not defined, defaults to [columnSpacing].
  /// Must be null or from 0 to 300 dp.
  final double? shadesSpacing,

  /// Enable the opacity control for the color value.
  ///
  /// Set to true to allow users to control the opacity value of the
  /// selected color. The displayed Opacity value on the slider goes from 0%,
  /// which is totally transparent, to 100%, which if fully opaque.
  ///
  /// When enabled, the opacity value is not returned as a separate value,
  /// it is returned in the alpha channel of the returned ARGB color value, in
  /// the onColor callbacks.
  ///
  /// Defaults to false.
  bool enableOpacity = false,

  /// The height of the opacity slider track.
  ///
  /// Defaults to 36 dp
  double opacityTrackHeight = 36,

  /// The width of the opacity slider track.
  ///
  /// If null, the slider fills to expand available width of the picker.
  /// If not null, it must be >= 150 dp.
  double? opacityTrackWidth,

  /// The radius of the thumb on the opacity slider.
  ///
  /// Defaults to 16 dp.
  double opacityThumbRadius = 16,

  /// Used to configure action buttons for the color picker dialog.
  ///
  /// Defaults to [ColorPickerActionButtons] ().
  ColorPickerActionButtons actionButtons = const ColorPickerActionButtons(),

  /// Used to configure the copy paste behavior of the color picker.
  ///
  /// Defaults to [ColorPickerCopyPasteBehavior] ().
  ColorPickerCopyPasteBehavior copyPasteBehavior =
      const ColorPickerCopyPasteBehavior(),

  /// Icon data for the icon used to indicate the selected color.
  ///
  /// The size of the `selectedColorIcon` is 60% of the smaller of color
  /// indicator `width` and `height`. The color of indicator icon is
  /// black or white, based on what contrast best with the selected color.
  ///
  /// Defaults to a check mark [Icons.check].
  IconData selectedColorIcon = Icons.check,

  /// Width of the color indicator items.
  ///
  /// Defaults to 40 dp. Must be from 15 to 150 dp.
  double width = 40.0,

  /// Height of the color indicator items.
  ///
  /// Defaults to 40 dp. Must be from 15 to 150 dp.
  double height = 40.0,

  /// Set to true to make tonal color items same size as the size defined
  /// for main and swatch shades indicator items.
  ///
  /// If false, the tonal color items will be smaller and auto sized for the
  /// palette to be same width as the Material-2 Color palette.
  ///
  /// Defaults to false. The color boxes are smaller, but length of their
  /// items is the same as MaterialColor swatch. You may prefer true to get
  /// them to be same size, especially if you only use tonal palette.
  ///
  /// For legacy compatibility reasons, this property is false by default.
  bool tonalColorSameSize = false,

  /// The horizontal spacing between the color picker indicator items.
  ///
  /// Defaults to 4 dp. Must be from 0 to 50 dp.
  double spacing = 4,

  /// The space between the color picker color item rows, when they need to
  /// be wrapped to multiple rows.
  ///
  /// Defaults to 4 dp. Must be from 0 to 50 dp.
  double runSpacing = 4,

  /// The Material elevation of the color indicator items.
  ///
  /// Defaults to 0 dp. Must be >= 0.
  double elevation = 0,

  /// Set to true, to show a 1 dp border around the color indicator items.
  ///
  /// This property is useful if the white/near white and black/near black
  /// shades color picker is enabled.
  ///
  /// Defaults to false.
  bool hasBorder = false,

  /// Border radius of the color indicator items.
  ///
  /// If null, it defaults to `width`/4. Must be from 0 to 50 dp, if not null.
  double? borderRadius,

  /// The color of the 1 dp optional border used on [ColorIndicator] and on
  /// [ColorWheelPicker], when each have their border toggle set to true.
  ///
  /// If no color is given, the border color defaults to
  /// Theme.of(context).dividerColor.
  Color? borderColor,

  /// Diameter of the HSV based color wheel picker.
  ///
  /// Defaults to 190 dp. Must be from 100 to maximum 500 dp.
  double wheelDiameter = 190,

  /// The stroke width of the color wheel circle.
  ///
  /// Defaults to 16 dp. Must be from 4 to maximum 50 dp.
  double wheelWidth = 16,

  /// Padding between shade square inside the hue wheel and inner
  /// side of the wheel.
  ///
  /// Keep it reasonable in relation to wheelDiameter and wheelWidth, values
  /// from 0 to 20 are recommended.
  ///
  /// Defaults to 0 dp.
  double wheelSquarePadding = 0,

  /// Border radius of the shade square inside the hue wheel.
  ///
  /// Keep it reasonable, the thumb center always goes out to the square box
  /// corner, regardless of this border radius. It is only for visual design,
  /// the edge color shades are in the sharp corner, even if not shown.
  ///
  /// Recommended values 0 to 16.
  ///
  /// Defaults to 4 dp.
  double wheelSquareBorderRadius = 4,

  /// Set to true to show a 1 dp border around the color wheel.
  ///
  /// Defaults to false.
  bool wheelHasBorder = false,

  /// Title widget for the color picker.
  ///
  /// Typically a Text widget, e.g. `Text('ColorPicker')`. If not provided or
  /// null, there is no title on the toolbar of the color picker.
  ///
  /// This widget can be used instead of `heading` or with it, depends on design
  /// need.
  ///
  /// The title widget is like an app bar title in the sense that at
  /// the end of it, 1 to 4 actions buttons may also be present for copy, paste,
  /// select-close and cancel-close. The select-close and cancel-close actions
  /// should only be enabled when the picker is used in a dialog. The copy and
  /// paste actions can be enabled also when the picker is not in a dialog.
  Widget? title,

  /// Heading widget for the color picker.
  ///
  /// Typically a Text widget, e.g. `Text('Select color')`.
  /// If not provided or null, there is no heading for the color picker.
  Widget? heading,

  /// Subheading widget for the color shades selection.
  ///
  /// Typically a Text widget, e.g. `Text('Select color shade')`.
  /// If not provided or null, there is no subheading for the color shades.
  Widget? subheading,

  /// Subheading widget for the color tone selection.
  ///
  /// Typically a Text widget, e.g. `Text('Select color shade')`.
  /// If not provided or null, there is no subheading for the color shades.
  Widget? tonalSubheading,

  /// Subheading widget for the HSV color wheel picker.
  ///
  /// Typically a Text widget, e.g.
  /// `Text('Selected color and its material like shades')`.
  ///
  /// The color wheel uses a separate subheading widget so that it may have
  /// another explanation, since its use case differs from the other subheading
  /// cases. If not provided, there is no subheading for the color wheel picker.
  Widget? wheelSubheading,

  /// Subheading widget for the recently used colors.
  ///
  /// Typically a Text widget, e.g. `Text('Recent colors')`.
  /// If not provided or null, there is no subheading for the recent color.
  /// The recently used colors subheading is not shown even if provided, when
  /// `showRecentColors` is false.
  Widget? recentColorsSubheading,

  /// Subheading widget for the opacity slider.
  ///
  /// Typically a Text widget, e.g. `Text('Opacity')`.
  /// If not provided or null, there is no subheading for the opacity slider.
  /// The opacity subheading is not shown even if provided, when
  /// `enableOpacity` is false.
  Widget? opacitySubheading,

  /// Set to true to show the Material name and index of the selected `color`.
  ///
  /// Defaults to false.
  bool showMaterialName = false,

  /// Text style for the displayed material color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  TextStyle? materialNameTextStyle,

  /// Set to true to show an English color name of the selected `color`.
  ///
  /// Uses the `ColorTools.nameThatColor` function to give an English name to
  /// any selected color. The function has a list of 1566 color codes and
  /// their names, it finds the color that closest matches the given color in
  /// the list and returns its color name.
  ///
  /// Defaults to false.
  bool showColorName = false,

  /// Text style for the displayed color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  TextStyle? colorNameTextStyle,

  /// Set to true to show the RGB Hex color code of the selected `color`.
  ///
  /// The color code can be copied with copy icon button or other enabled copy
  /// actions in the color picker. On the wheel picker the color code can be
  /// edited to enter and select a color of a known RGB hex value. If the
  /// property `colorCodeReadOnly` has been set to false the color code field
  /// can never be edited directly, it is then only used to display the code
  /// of currently selected color.
  ///
  /// Defaults to false.
  bool showColorCode = false,

  /// When true, the color code entry field uses the currently selected
  /// color as its background color.
  ///
  /// This makes the color code entry field a large current color indicator
  /// area, that changes color as the color value is changed.
  /// The text color of the field, will automatically adjust for best contrast,
  /// as will the opacity indicator text. Enabling this feature will override
  /// any color specified in `colorCodeTextStyle` and `colorCodePrefixStyle`,
  /// but their styles will otherwise be kept as specified.
  ///
  /// Defaults to false.
  bool colorCodeHasColor = false,

  /// Whether to show an edit icon button before the color code field.
  ///
  /// The edit icon button can be used to give users a visual que that the
  /// color code field can be edited.
  ///
  /// When set to true, the icon button is only shown when the wheel picker is
  /// active and [colorCodeReadOnly] is false.
  ///
  /// Tapping the icon button will focus the color code entry field.
  ///
  /// Defaults to false.
  bool showEditIconButton = false,

  /// The icon to use on the edit icon button.
  ///
  /// Defaults to [Icons.edit].
  IconData editIcon = Icons.edit,

  /// Whether the color code entry field should have no color when focused.
  ///
  /// If the option to make the color code field have the same color as the
  /// selected color is enabled via [colorCodeHasColor], it makes it look
  /// and double like a big color indicator that shows the selected color.
  ///
  /// It can also make the edit of the color code confusing, as its color on
  /// purpose also changes as you edit and enter a new color value. If you
  /// find this behavior confusing and want to make the color code field
  /// always have no color during value entry, regardless of the selected color,
  /// then set this option to true.
  ///
  /// Defaults to false.
  bool focusedEditHasNoColor = false,

  /// Text style for the displayed generic color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  TextStyle? colorCodeTextStyle,

  /// The TextStyle of the prefix of the color code.
  ///
  /// The prefix always include the alpha value and may also include a num char
  /// '#' or '0x' based on the `ColorPickerCopyPasteBehavior.copyFormat`
  /// setting.
  ///
  /// Defaults to `colorCodeTextStyle`, if not defined.
  TextStyle? colorCodePrefixStyle,

  /// When true, the color code field is always read only.
  ///
  /// If set to true, the color code field cannot be edited. Normally it can
  /// be edited when used in a picker that can select and show any color.
  /// Setting this to false makes it read only also on such pickers. This
  /// currently only applies to the wheel picker, but will also apply to
  /// future full color range pickers.
  ///
  /// Pickers that only offer a fixed palette, that you can just offered colors
  /// from always have the color code field in read only mode, this setting
  /// does not affect them.
  ///
  /// Regardless of the picker and `colorCodeReadOnly` value, you can change
  /// color value by pasting in a new value, if your copy paste configuration
  /// allows it.
  ///
  /// Defaults to false.
  bool colorCodeReadOnly = false,

  /// Set to true to show the int [Color.value] of the selected `color`.
  ///
  /// This is a developer feature, showing the int color value can be
  /// useful during software development. If enabled the value is shown after
  /// the color code. For text style it also uses the `colorCodeTextStyle`.
  /// There is no copy button for the shown int value, but the value is
  /// displayed with a [SelectableText] widget, so it can be select painted
  /// and copied if so required.
  ///
  /// Defaults to false.
  bool showColorValue = false,

  /// Set to true to a list of recently selected colors selection at the bottom
  /// of the picker.
  ///
  /// When `showRecentColors` is enabled, the color picker shows recently
  /// selected colors in a list at the bottom of the color picker. The list
  /// uses first-in, first-out to keep min 2 to max 20 colors (defaults to 5)
  /// on the recently used colors list, the desired max value can be modified
  /// with `maxRecentColors`.
  ///
  /// Defaults to false.
  bool showRecentColors = false,

  /// The maximum numbers of recent colors to show in the list of recent colors.
  ///
  /// The max recent colors must be from 2 to 20. Defaults to 5.
  int maxRecentColors = 5,

  /// A list with the recently select colors.
  ///
  /// Defaults to an empty list of colors. You can provide a starting
  /// set from some stored state if so desired.
  List<Color> recentColors = const <Color>[],

  /// Set to true to enable all tooltips in this widget.
  ///
  /// When true, it enables all tooltips that are available in the color picker.
  /// If the tooltips get in the way you can disable them all by setting this
  /// property to `false`. Why not consider providing a setting in your app that
  /// allows users to turn ON and OFF the tooltips in the app? FlexColorPicker
  /// includes this toggle to make that easy to implement when it comes to its
  /// tooltip behavior.
  ///
  /// Defaults to true.
  bool enableTooltips = true,

  /// The color on the thumb of the slider that shows the selected picker type.
  ///
  /// If not defined, it defaults to `Color(0xFFFFFFFF)` (white) in light
  /// theme and to `Color(0xFF636366)` in dark theme, which are the defaults
  /// for the used [CupertinoSlidingSegmentedControl].
  ///
  /// If you give it a custom color, the color picker will automatically adjust
  /// the text color on the selected thumb for best legible text contrast.
  Color? selectedPickerTypeColor,

  /// The TextStyle of the labels in segmented color picker type selector.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodySmall`, if not defined.
  TextStyle? pickerTypeTextStyle,

  /// A [ColorPickerType] to String map that contains labels for the picker
  /// type selector.
  ///
  /// If not defined, or omitted in provided mpa, then the following default
  /// English labels are used:
  ///  * [ColorPickerType.both] : 'Both'
  ///  * [ColorPickerType.primary] : 'Primary & Accent'
  ///  * [ColorPickerType.accent] : 'Primary'
  ///  * [ColorPickerType.bw] : 'Black & White'
  ///  * [ColorPickerType.custom] : 'Custom'
  ///  * [ColorPickerType.wheel] : 'Wheel'
  Map<ColorPickerType, String> pickerTypeLabels =
      const <ColorPickerType, String>{
    ColorPickerType.primary: ColorPicker._selectPrimaryLabel,
    ColorPickerType.accent: ColorPicker._selectAccentLabel,
    ColorPickerType.bw: ColorPicker._selectBlackWhiteLabel,
    ColorPickerType.both: ColorPicker._selectBothLabel,
    ColorPickerType.custom: ColorPicker._selectCustomLabel,
    ColorPickerType.wheel: ColorPicker._selectWheelAnyLabel,
  },

  /// Color swatch to name map, with custom swatches and their names.
  ///
  /// Used to provide custom [ColorSwatch] objects to the custom color picker,
  /// including their custom name label. These colors, their swatch shades
  /// and names, are shown and used when the picker type
  /// [ColorPickerType.custom] option is enabled in the color picker.
  ///
  /// Defaults to an empty map. If the map is empty, the custom colors picker
  /// will not be shown even if it is enabled in `pickersEnabled`.
  Map<ColorSwatch<Object>, String> customColorSwatchesAndNames =
      const <ColorSwatch<Object>, String>{},

  /// Color swatch to name map, with custom swatches and their names.
  ///
  /// Used to provide custom [ColorSwatch] objects to the custom color picker,
  /// including their custom name label. These colors, their swatch shades
  /// and names, are shown and used when the picker type
  /// [ColorPickerType.customSecondary] option is enabled in the color picker.
  ///
  /// Defaults to an empty map. If the map is empty, the custom colors picker
  /// will not be shown even if it is enabled in [pickersEnabled].
  Map<ColorSwatch<Object>, String> customSecondaryColorSwatchesAndNames =
      const <ColorSwatch<Object>, String>{},

  // ***************************************************************************
  // Below properties that refer to the dialog
  // ***************************************************************************

  /// Title of the color picker dialog, often omitted in favor of using a
  /// `title` and/or `heading` already defined in the [ColorPicker].
  Widget? dialogTitle,

  /// Padding around the dialog title, if a title is used.
  /// Defaults to `EdgeInsets.zero`, since the title is normally omitted
  /// and provided via the `heading` property of the `ColorPicker` instead.
  final EdgeInsetsGeometry titlePadding = EdgeInsets.zero,

  /// Style for the text in the dialog `title` of this `AlertDialog`.
  ///
  /// If null, `DialogTheme.titleTextStyle` is used. If that's null,
  /// defaults to `TextTheme.titleLarge` of `ThemeData.textTheme`.
  final TextStyle? titleTextStyle,

  /// Padding around the content in the dialog.
  ///
  /// Defaults to `EdgeInsets.zero`, as the content padding is expected to
  /// be a part of the `ColorPicker`.
  final EdgeInsetsGeometry contentPadding = EdgeInsets.zero,

  /// Padding around the Cancel and OK action buttons at the bottom of
  /// the dialog.
  ///
  /// Typically used to provide padding to the button bar between the button
  /// bar and the edges of the dialog.
  ///
  /// Defaults to null and follows ambient [AlertDialog] themed actions padding
  /// or [AlertDialog] default if not defined.
  ///
  /// Versions before FlexColorPicker 3.0.0 defaulted to
  /// `EdgeInsets.symmetric(horizontal: 16) use it for same padding as in
  /// previous versions.
  final EdgeInsetsGeometry? actionsPadding,

  /// The padding that surrounds each bottom action button.
  ///
  /// This is different from [actionsPadding], which defines the padding
  /// between the entire button bar and the edges of the dialog.
  ///
  /// Defaults to null and follows ambient [AlertDialog] themed button padding
  /// or [AlertDialog] default if not defined.
  ///
  /// Versions before FlexColorPicker 3.0.0 defaulted to `EdgeInsets.all(16),
  /// use it for same button padding as in previous versions.
  final EdgeInsetsGeometry? buttonPadding,

  /// The background color of the surface of this Dialog.
  ///
  /// This sets the Material.color on this Dialog's Material.
  /// If null, ThemeData.dialogBackgroundColor is used.
  ///
  /// NOTE: The ColorPicker is designed to fit on background color with
  /// brightness that follow active theme mode. Putting e.g. white as
  /// background in dark theme mode, will not produce usable results.
  final Color? backgroundColor,

  /// The z-coordinate of this Dialog.
  ///
  /// If null then DialogTheme.elevation is used, and if that's null then the
  /// dialog's elevation is 24.0. The z-coordinate at which to place this
  /// material relative to its parent.
  ///
  /// This controls the size of the shadow below the material and the opacity
  /// of the elevation overlay color if it is applied. If this is non-zero,
  /// the contents of the material are clipped, because the widget
  /// conceptually defines an independent printed piece of material.
  /// Changing this value will cause the shadow and the elevation
  /// overlay to animate over Material.animationDuration.
  ///
  /// Defaults to 0.
  final double? dialogElevation,

  /// The color used to paint a drop shadow under the dialog's Material,
  /// which reflects the dialog's elevation.
  final Color? shadowColor,

  /// The color used as a surface tint overlay on the dialog's background
  /// color, which reflects the dialog's elevation.
  final Color? surfaceTintColor,

  /// The semantic label of the dialog used by accessibility frameworks to
  /// announce screen transitions when the dialog is opened and closed.
  ///
  /// In iOS, if this label is not provided, a semantic label will be inferred
  /// from the [title] if it is not null.
  ///
  /// In Android, if this label is not provided, the dialog will use the
  /// [MaterialLocalizations.alertDialogLabel] as its label.
  ///
  /// See also:
  ///
  ///  * [SemanticsConfiguration.namesRoute], for a description of how this
  ///    value is used.
  final String? semanticLabel,

  /// The amount of padding added to `MediaQueryData.viewInsets` on the
  /// outside of the `ColorPicker` dialog.
  ///
  /// Defines the minimum space between the screen's edges and the dialog.
  /// Defaults to `EdgeInsets.symmetric(horizontal: 40, vertical: 24)`.
  final EdgeInsets insetPadding =
      const EdgeInsets.symmetric(horizontal: 40, vertical: 24),

  /// Controls how the contents of the dialog are clipped (or not) to the
  /// given shape.
  ///
  /// See the enum `Clip` for details of all possible options and their
  /// common use cases.
  ///
  /// Defaults to Clip.none, and must not be null.
  final Clip clipBehavior = Clip.none,

  /// The shape of this dialog's border.
  ///
  /// Defines the dialog's Material.shape.
  ///
  /// The default shape is a RoundedRectangleBorder with a radius of 4.0.
  final ShapeBorder? shape,

  /// If true, the dialog can be closed by clicking outside it.
  ///
  /// Defaults to true.
  bool barrierDismissible = true,

  /// The background transparency color of the dialog barrier.
  ///
  /// Defaults to [Colors.black12] which is considerably lighter than the
  /// standard [Colors.black54] and allows us to see the impact of selected
  /// color on app behind the dialog. If this is not desired, set it back to
  /// [Colors.black54] when you call `showColorPickerDialog`, or make it
  /// even more transparent.
  ///
  /// You can also make the barrier completely transparent.
  Color barrierColor = Colors.black12,

  /// The `barrierLabel` argument is the semantic label used for a dismissible
  /// barrier. This argument defaults to `null`.
  String? barrierLabel,

  /// The `useSafeArea` argument is used to indicate if the dialog should only
  /// display in 'safe' areas of the screen not used by the operating system
  /// (see [SafeArea] for more details).
  ///
  /// Default to `true` by default, which means the dialog will not overlap
  /// operating system areas. If it is set to `false` the dialog will only
  /// be constrained by the screen size.
  bool useSafeArea = true,

  /// The `routeSettings` argument is passed to [showGeneralDialog],
  /// see [RouteSettings] for details.
  RouteSettings? routeSettings,

  /// Offset anchorPoint for the dialog.
  Offset? anchorPoint,

  /// The [transitionBuilder] argument is used to define how the route
  /// arrives on and leaves off the screen.
  ///
  /// If this transition is not specified, the default Material platform
  /// transition builder for [showDialog] is used.
  RouteTransitionsBuilder? transitionBuilder,

  /// The [transitionDuration] argument is used to determine how long it takes
  /// for the route to arrive on or leave off the screen.
  ///
  /// It only has any effect when a custom `transitionBuilder`is used.
  ///
  /// This argument defaults to 200 milliseconds.
  Duration transitionDuration = const Duration(milliseconds: 200),

  /// You can provide BoxConstraints to constrain the size of the dialog.
  ///
  /// You might want to do this at least for the height, otherwise
  /// the dialog height might jump up and down jarringly if its size changes
  /// when user changes the picker type with the selector.¨
  ///
  /// Normally you would not change the picker's content element sizes after
  /// you have determined what works in your implementation. You can usually
  /// figure out a good dialog box size that works well for your use case,
  /// for all active pickers, instead of allowing the color picker dialog
  /// to auto size itself, which it will do if no constraints are defined.
  BoxConstraints? constraints,
}

    /// Picker properties
    ) async {
  Color selectedColor = color;
  // ignore: use_build_context_synchronously
  if (!(await ColorPicker(
    color: color,
    onColorChanged: (Color newColor) {
      selectedColor = newColor;
    },
    pickersEnabled: pickersEnabled,
    enableShadesSelection: enableShadesSelection,
    includeIndex850: includeIndex850,
    enableTonalPalette: enableTonalPalette,
    tonalPaletteFixedMinChroma: tonalPaletteFixedMinChroma,
    crossAxisAlignment: crossAxisAlignment,
    mainAxisSize: mainAxisSize,
    padding: padding,
    columnSpacing: columnSpacing,
    toolbarSpacing: toolbarSpacing,
    shadesSpacing: shadesSpacing,
    enableOpacity: enableOpacity,
    opacityTrackHeight: opacityTrackHeight,
    opacityTrackWidth: opacityTrackWidth,
    opacityThumbRadius: opacityThumbRadius,
    actionButtons: actionButtons,
    copyPasteBehavior: copyPasteBehavior,
    selectedColorIcon: selectedColorIcon,
    width: width,
    height: height,
    spacing: spacing,
    tonalColorSameSize: tonalColorSameSize,
    runSpacing: runSpacing,
    elevation: elevation,
    hasBorder: hasBorder,
    borderRadius: borderRadius,
    borderColor: borderColor,
    wheelDiameter: wheelDiameter,
    wheelWidth: wheelWidth,
    wheelSquarePadding: wheelSquarePadding,
    wheelSquareBorderRadius: wheelSquareBorderRadius,
    wheelHasBorder: wheelHasBorder,
    title: title,
    heading: heading,
    subheading: subheading,
    tonalSubheading: tonalSubheading,
    wheelSubheading: wheelSubheading,
    recentColorsSubheading: recentColorsSubheading,
    opacitySubheading: opacitySubheading,
    showMaterialName: showMaterialName,
    materialNameTextStyle: materialNameTextStyle,
    showColorName: showColorName,
    colorNameTextStyle: colorNameTextStyle,
    showColorCode: showColorCode,
    colorCodeHasColor: colorCodeHasColor,
    showEditIconButton: showEditIconButton,
    editIcon: editIcon,
    focusedEditHasNoColor: focusedEditHasNoColor,
    colorCodeTextStyle: colorCodeTextStyle,
    colorCodePrefixStyle: colorCodePrefixStyle,
    colorCodeReadOnly: colorCodeReadOnly,
    showColorValue: showColorValue,
    showRecentColors: showRecentColors,
    maxRecentColors: maxRecentColors,
    recentColors: recentColors,
    enableTooltips: enableTooltips,
    selectedPickerTypeColor: selectedPickerTypeColor,
    pickerTypeTextStyle: pickerTypeTextStyle,
    pickerTypeLabels: pickerTypeLabels,
    customColorSwatchesAndNames: customColorSwatchesAndNames,
    customSecondaryColorSwatchesAndNames: customSecondaryColorSwatchesAndNames,
  ).showPickerDialog(
    context,
    title: dialogTitle,
    titlePadding: titlePadding,
    titleTextStyle: titleTextStyle,
    contentPadding: contentPadding,
    actionsPadding: actionsPadding,
    buttonPadding: buttonPadding,
    backgroundColor: backgroundColor,
    elevation: dialogElevation,
    shadowColor: shadowColor,
    surfaceTintColor: surfaceTintColor,
    semanticLabel: semanticLabel,
    insetPadding: insetPadding,
    clipBehavior: clipBehavior,
    shape: shape,
    barrierColor: barrierColor,
    barrierDismissible: barrierDismissible,
    barrierLabel: barrierLabel,
    useSafeArea: useSafeArea,
    routeSettings: routeSettings,
    anchorPoint: anchorPoint,
    transitionBuilder: transitionBuilder,
    transitionDuration: transitionDuration,
    constraints: constraints,
  ))) {
    selectedColor = color;
  }
  return selectedColor;
}

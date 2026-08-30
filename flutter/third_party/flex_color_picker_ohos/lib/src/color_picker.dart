import 'dart:async';

import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'color_indicator.dart';
import 'color_picker_extensions.dart';
import 'color_tools.dart';
import 'color_wheel_picker.dart';
import 'functions/picker_functions.dart';
import 'models/color_picker_action_buttons.dart';
import 'models/color_picker_copy_paste_behavior.dart';
import 'models/color_picker_type.dart';
import 'widgets/color_code_field.dart';
import 'widgets/color_picker_toolbar.dart';
import 'widgets/copy_paste_handler.dart';
import 'widgets/main_colors.dart';
import 'widgets/opacity/opacity_slider.dart';
import 'widgets/picker_selector.dart';
import 'widgets/recent_colors.dart';
import 'widgets/shade_colors.dart';
import 'widgets/tonal_palette_colors.dart';

part 'show_color_picker_dialog.dart';

// ignore_for_file: comment_references

// Set the bool flag to true to show debug prints. Even if you forgot
// to set it to false, debug prints will not show in release builds.
// The handy part is that if it gets in the way in debugging, it is an easy
// toggle to turn it off here for just this feature. You can leave it true
// below to see this feature's logs in debug mode.
const bool _debug = !kReleaseMode && false;

const int _minRecentColors = 2;
const int _maxRecentColors = 20;

/// A customizable Material primary color, accent color and custom colors,
/// color picker.
///
/// You can configure which Material color swatches can be used for color
/// selection, any combination of both primary/accent in same picker or in
/// separate groups. There is an almost black and white shades picker and
/// it is possible to include a page with custom material and accent swatches
/// using custom names for the custom swatches.
/// It is possible to specify if only the main color in a swatch should be
/// selectable or its shades as well.
///
/// There is also a color wheel picker that allows you to select any
/// color and automatically generate a Material primary swatch for the
/// selected color.
///
/// If a selected color in the wheel picker belongs to any standard Material
/// color, primary or accent, and any of its shades or any of the provided
/// custom color swatches, then the wheel picker will not calculate swatch
/// colors for such a color. It will instead show all the shades from the
/// selected color's swatch. Selecting the shades on the wheel picker will then
/// select the shade color and show where the color shade is on the HSV wheel.
///
/// If a selected on the color wheel, is not any color or shade of the
/// pre-defined ones, then the wheel picker will always generate a
/// new swatch from the selected color, using the selected color as the new
/// primary swatch 500 index midpoint.
@immutable
class ColorPicker extends StatefulWidget {
  /// Default constructor for the color picker.
  const ColorPicker({
    super.key,
    // Core properties, set color and change callbacks.
    this.color = Colors.blue,
    required this.onColorChanged,
    this.onColorChangeStart,
    this.onColorChangeEnd,
    // Color picker types shown and used by the color picker.
    this.pickersEnabled = const <ColorPickerType, bool>{
      ColorPickerType.both: false,
      ColorPickerType.primary: true,
      ColorPickerType.accent: true,
      ColorPickerType.bw: false,
      ColorPickerType.custom: false,
      ColorPickerType.customSecondary: false,
      ColorPickerType.wheel: false,
    },
    this.enableShadesSelection = true,
    this.includeIndex850 = false,
    this.enableTonalPalette = false,
    this.tonalPaletteFixedMinChroma = false,
    // Layout
    this.mainAxisSize = MainAxisSize.max,
    this.crossAxisAlignment = CrossAxisAlignment.center,
    this.padding = const EdgeInsets.all(16),
    this.columnSpacing = 8,
    this.toolbarSpacing,
    this.shadesSpacing,
    // Opacity slider
    this.enableOpacity = false,
    this.opacityTrackHeight = 36,
    this.opacityTrackWidth,
    this.opacityThumbRadius = 16,
    // Picker action buttons and copy paste behavior.
    this.actionButtons = const ColorPickerActionButtons(),
    this.copyPasteBehavior = const ColorPickerCopyPasteBehavior(),
    this.selectedColorIcon = Icons.check,
    // Picker item and wheel picker properties.
    this.width = 40,
    this.height = 40,
    this.tonalColorSameSize = false,
    this.spacing = 4,
    this.runSpacing = 4,
    this.elevation = 0,
    this.hasBorder = false,
    this.borderRadius,
    this.borderColor,
    this.wheelDiameter = 190,
    this.wheelWidth = 16,
    this.wheelSquarePadding = 0,
    this.wheelSquareBorderRadius = 4,
    this.wheelHasBorder = false,
    // Title, headings and sub headings used by the color picker.
    this.title,
    this.heading,
    this.subheading,
    this.tonalSubheading,
    this.wheelSubheading,
    this.recentColorsSubheading,
    this.opacitySubheading,
    // Toggles to show color names and codes and their text styles.
    this.showMaterialName = false,
    this.materialNameTextStyle,
    this.showColorName = false,
    this.colorNameTextStyle,
    this.showColorCode = false,
    this.colorCodeHasColor = false,
    this.showEditIconButton = false,
    this.editIcon = Icons.edit,
    this.focusedEditHasNoColor = false,
    this.colorCodeTextStyle,
    @Deprecated('This property is deprecated and no longer has any function. '
        'It was removed in v2.0.0. To modify the copy icon on the color code '
        'entry field, define the `ColorPickerCopyPasteBehavior(copyIcon: '
        'myIcon)` and provide it via the `copyPasteBehavior` property.')
    this.colorCodeIcon,
    this.colorCodePrefixStyle,
    this.colorCodeReadOnly = false,
    this.showColorValue = false,
    // Toggles showing the recent colors selection.
    this.showRecentColors = false,
    this.maxRecentColors = 5,
    this.recentColors = const <Color>[],
    this.onRecentColorsChanged,
    // Enable tooltips
    this.enableTooltips = true,
    // Segmented color picker selector control properties.
    this.selectedPickerTypeColor,
    this.pickerTypeTextStyle,
    this.pickerTypeLabels = const <ColorPickerType, String>{
      ColorPickerType.primary: _selectPrimaryLabel,
      ColorPickerType.accent: _selectAccentLabel,
      ColorPickerType.bw: _selectBlackWhiteLabel,
      ColorPickerType.both: _selectBothLabel,
      ColorPickerType.custom: _selectCustomLabel,
      ColorPickerType.customSecondary: _selectCustomSecondaryLabel,
      ColorPickerType.wheel: _selectWheelAnyLabel,
    },
    // Custom color, swatches and name map for the custom color swatches.
    this.customColorSwatchesAndNames = const <ColorSwatch<Object>, String>{},
    this.customSecondaryColorSwatchesAndNames =
        const <ColorSwatch<Object>, String>{},
    //
  })  : assert(columnSpacing >= 0 && columnSpacing <= 300,
            'The picker item column spacing must be from 0 to max 300 dp.'),
        assert(
            toolbarSpacing == null ||
                (toolbarSpacing >= 0 && toolbarSpacing <= 300),
            'The spacing must be null or from 0 to max 300 dp.'),
        assert(
            shadesSpacing == null ||
                (shadesSpacing >= 0 && shadesSpacing <= 300),
            'The spacing must be null or from 0 to max 300 dp.'),
        assert(spacing >= 0 && spacing <= 50,
            'The picker item spacing must be from 0 to max 50 dp.'),
        assert(runSpacing >= 0 && runSpacing <= 50,
            'The picker item runSpacing must be from 0 to max 50 dp.'),
        assert(elevation >= 0, 'The picker item elevation must be >= 0 dp.'),
        assert(width >= 15 && width <= 150,
            'The pick item width must be from 15 to max 150 dp.'),
        assert(height >= 15 && height <= 150,
            'The pick item height must be from 15 to max 150 dp.'),
        assert(
            borderRadius == null || (borderRadius >= 0 && borderRadius <= 50),
            'The pick item borderRadius must be null or from 0 to max 50 dp.'),
        assert(opacityTrackWidth == null || opacityTrackWidth >= 150,
            'The opacity slider track width must be null or >= 150.'),
        assert(opacityTrackHeight >= 8 && opacityTrackHeight <= 50,
            'The opacity slider track height must be from 8 to max 50 dp.'),
        assert(opacityThumbRadius >= 12 && opacityThumbRadius <= 30,
            'The opacity slider thumb radius must be from 12 to max 30 dp.'),
        assert(wheelDiameter >= 100 && wheelDiameter <= 500,
            'The wheel diameter must be from 100 to max 500 dp.'),
        assert(wheelWidth >= 4 && wheelWidth <= 50,
            'The color wheel width must be from 4 to max 50 dp.'),
        assert(
            maxRecentColors >= _minRecentColors &&
                maxRecentColors <= _maxRecentColors,
            'The maxRecentColors must be >= $_minRecentColors '
            'and <= $_maxRecentColors.');

  /// The active color selection in the color picker.
  ///
  /// Also used as initial value when the color picker is created.
  ///
  /// Use the callback [onColorChanged] to get the new color values from
  /// the picker and update the [color] in the parent widget.
  final Color color;

  /// Required [ValueChanged] callback, called when user selects
  /// a new color with new color value.
  ///
  ///
  /// Called every time the color value changes when operating thumbs on the
  /// color wheel or color or transparency sliders
  ///
  /// Changing which picker type is viewed does not trigger this callback, it
  /// is not triggered until a color in the viewed picker is selected.
  ///
  /// The picker passes the new value to the callback but does not actually
  /// change state until the parent widget rebuilds the color picker with
  /// the new value.
  ///
  /// The callback provided to [onColorChanged] should update the state of the
  /// parent [StatefulWidget] using the [State.setState] method, so that the
  /// parent gets rebuilt; for example:
  ///
  /// ```dart
  /// ColorPicker(
  ///   color: _pickerColor,
  ///   onChanged: (Color color) {
  ///     setState(() {
  ///       _pickerColor = color;
  ///     });
  ///   },
  /// )
  /// ```
  final ValueChanged<Color> onColorChanged;

  /// Optional [ValueChanged] callback. Called when user starts color selection
  /// with current color value.
  ///
  /// When clicking a new color in color items, the color value before the
  /// selected new value was clicked is returned. It is also called
  /// with the current start color when user starts the interaction on the
  /// color wheel or on a color or transparency slider.
  final ValueChanged<Color>? onColorChangeStart;

  /// Optional [ValueChanged] callback. Called when user ends color selection
  /// with the new color value.
  ///
  /// When clicking a new color on color items, the clicked color is returned.
  /// It is also called with the resulting color value when user ends the
  /// interaction on the color wheel or on a color or transparency slider.
  final ValueChanged<Color>? onColorChangeEnd;

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
  final Map<ColorPickerType, bool> pickersEnabled;

  /// Set to true to allow selection of color swatch shades.
  ///
  /// If false, only the main color from a swatch is shown and can be selected.
  /// This is index [500] for Material primary colors and index [200] for accent
  /// colors. On the Wheel, only the selected color is shown there is no
  /// color related color swatch of the selected color shown.
  ///
  /// Defaults to true.
  final bool enableShadesSelection;

  /// There is an extra index [850] used only by grey Material color in Flutter.
  /// If you want to include it in the grey color shades selection, then set
  /// this property to true.
  ///
  /// Defaults to false.
  final bool includeIndex850;

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
  final bool enableTonalPalette;

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
  final bool tonalPaletteFixedMinChroma;

  /// Cross axis alignment used to layout the main content of the
  /// color picker in its column layout.
  ///
  /// Defaults to CrossAxisAlignment.center.
  final CrossAxisAlignment crossAxisAlignment;

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
  /// Defaults to [MainAxisSize.max] like [Column] does as well.
  final MainAxisSize mainAxisSize;

  /// Padding around the entire color picker content.
  ///
  /// Defaults to const EdgeInsets.all(16).
  final EdgeInsetsGeometry padding;

  /// Vertical spacing between items in the color picker column.
  ///
  /// Defaults to 8 dp. Must be from 0 to 300 dp.
  final double columnSpacing;

  /// Vertical spacing below the top toolbar header and action buttons.
  ///
  /// If not defined, defaults to [columnSpacing].
  /// Must be null or from 0 to 300 dp.
  final double? toolbarSpacing;

  /// Vertical spacing below the Material-2 based color shades palette.
  ///
  /// If not defined, defaults to [columnSpacing].
  /// Must be null or from 0 to 300 dp.
  final double? shadesSpacing;

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
  /// When false, colors that has any other alpha value than 0xFF are changed
  /// to 0xFF. To allow custom colors and pasted in color values without
  /// setting [enableOpacity] to true and showing the opacity slider, set
  /// [enableTransparentCustomColors] to true.
  ///
  /// Defaults to false.
  final bool enableOpacity;

  /// The height of the opacity slider track.
  ///
  /// Defaults to 36 dp
  final double opacityTrackHeight;

  /// The width of the opacity slider track.
  ///
  /// If null, the slider fills to expand available width of the picker.
  /// If not null, it must be >= 150 dp.
  final double? opacityTrackWidth;

  /// The radius of the thumb on the opacity slider.
  ///
  /// Defaults to 16 dp.
  final double opacityThumbRadius;

  /// Used to configure action buttons for the color picker dialog.
  ///
  /// Defaults to [ColorPickerActionButtons] ().
  final ColorPickerActionButtons actionButtons;

  /// Used to configure the copy paste behavior of the color picker.
  ///
  /// Defaults to [ColorPickerCopyPasteBehavior] ().
  final ColorPickerCopyPasteBehavior copyPasteBehavior;

  /// Icon data for the icon used to indicate the selected color.
  ///
  /// The size of the [selectedColorIcon] is 60% of the smaller of color
  /// indicator [width] and [height]. The color of indicator icon is
  /// black or white, based on what contrast best with the selected color.
  ///
  /// Defaults to a check mark [Icons.check].
  final IconData selectedColorIcon;

  /// Width of the color indicator items.
  ///
  /// Defaults to 40 dp. Must be from 15 to 150 dp.
  final double width;

  /// Height of the color indicator items.
  ///
  /// Defaults to 40 dp. Must be from 15 to 150 dp.
  final double height;

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
  final bool tonalColorSameSize;

  /// The horizontal spacing between the color picker indicator items.
  ///
  /// Defaults to 4 dp. Must be from 0 to 50 dp.
  final double spacing;

  /// The space between the color picker color item rows, when they need to
  /// be wrapped to multiple rows.
  ///
  /// Defaults to 4 dp. Must be from 0 to 50 dp.
  final double runSpacing;

  /// The Material elevation of the color indicator items.
  ///
  /// Defaults to 0 dp. Must be >= 0.
  final double elevation;

  /// Set to true, to show a 1 dp border around the color indicator items.
  ///
  /// This property is useful if the white/near white and black/near black
  /// shades color picker is enabled.
  ///
  /// Defaults to false.
  final bool hasBorder;

  /// Border radius of the color indicator items.
  ///
  /// If null, it defaults to [width]/4. Must be from 0 to 50 dp, if not null.
  final double? borderRadius;

  /// The color of the 1 dp optional border used on [ColorIndicator] and on
  /// [ColorWheelPicker], when each have their border toggle set to true.
  ///
  /// If no color is given, the border color defaults to
  /// Theme.of(context).dividerColor.
  final Color? borderColor;

  /// Diameter of the HSV based color wheel picker.
  ///
  /// Defaults to 190 dp. Must be from 100 to maximum 500 dp.
  final double wheelDiameter;

  /// The stroke width of the color wheel circle.
  ///
  /// Defaults to 16 dp. Must be from 4 to maximum 50 dp.
  final double wheelWidth;

  /// Padding between shade square inside the hue wheel and inner
  /// side of the wheel.
  ///
  /// Keep it reasonable in relation to wheelDiameter and wheelWidth, values
  /// from 0 to 20 are recommended.
  ///
  /// Defaults to 0 dp.
  final double wheelSquarePadding;

  /// Border radius of the shade square inside the hue wheel.
  ///
  /// Keep it reasonable, the thumb center always goes out to the square box
  /// corner, regardless of this border radius. It is only for visual design,
  /// the edge color shades are in the sharp corner, even if not shown.
  ///
  /// Recommended values 0 to 16.
  ///
  /// Defaults to 4 dp.
  final double wheelSquareBorderRadius;

  /// Set to true to show a 1 dp border around the color wheel.
  ///
  /// Defaults to false.
  final bool wheelHasBorder;

  /// Title widget for the color picker.
  ///
  /// Typically a Text widget, e.g. `Text('ColorPicker')`. If not provided or
  /// null, there is no title on the toolbar of the color picker.
  ///
  /// This widget can be used instead of [heading] or with it, depends on design
  /// need.
  ///
  /// The title widget is like an app bar title in the sense that at
  /// the end of it, 1 to 4 actions buttons may also be present for copy, paste,
  /// select-close and cancel-close. The select-close and cancel-close actions
  /// should only be enabled when the picker is used in a dialog. The copy and
  /// paste actions can be enabled also when the picker is not in a dialog.
  final Widget? title;

  /// Heading widget for the color picker.
  ///
  /// Typically a Text widget, e.g. `Text('Select color')`.
  /// If not provided or null, there is no heading for the color picker.
  final Widget? heading;

  /// Subheading widget for the color shades selection.
  ///
  /// Typically a Text widget, e.g. `Text('Select color shade')`.
  /// If not provided or null, there is no subheading for the color shades.
  final Widget? subheading;

  /// Subheading widget for the color tone selection.
  ///
  /// Typically a Text widget, e.g. `Text('Select Material 3 color tone')`.
  /// If not provided or null, there is no subheading for the color shades.
  final Widget? tonalSubheading;

  /// Subheading widget for the HSV color wheel picker.
  ///
  /// Typically a Text widget, e.g.
  /// `Text('Selected color and its material like shades')`.
  ///
  /// The color wheel uses a separate subheading widget so that it may have
  /// another explanation, since its use case differs from the other subheading
  /// cases. If not provided, there is no subheading for the color wheel picker.
  final Widget? wheelSubheading;

  /// Subheading widget for the recently used colors.
  ///
  /// Typically a Text widget, e.g. `Text('Recent colors')`.
  /// If not provided or null, there is no subheading for the recent color.
  /// The recently used colors subheading is not shown even if provided, when
  /// [showRecentColors] is false.
  final Widget? recentColorsSubheading;

  /// Subheading widget for the opacity slider.
  ///
  /// Typically a Text widget, e.g. `Text('Opacity')`.
  /// If not provided or null, there is no subheading for the opacity slider.
  /// The opacity subheading is not shown even if provided, when
  /// [enableOpacity] is false.
  final Widget? opacitySubheading;

  /// Set to true to show the Material name and index of the selected [color].
  ///
  /// Defaults to false.
  final bool showMaterialName;

  /// Text style for the displayed material color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  final TextStyle? materialNameTextStyle;

  /// Set to true to show an English color name of the selected [color].
  ///
  /// Uses the [ColorTools.nameThatColor] function to give an English name to
  /// any selected color. The function has a list of 1566 color codes and
  /// their names, it finds the color that closest matches the given color in
  /// the list and returns its color name.
  ///
  /// Defaults to false.
  final bool showColorName;

  /// Text style for the displayed color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  final TextStyle? colorNameTextStyle;

  /// Set to true to show the RGB Hex color code of the selected [color].
  ///
  /// The color code can be copied with copy icon button or other enabled copy
  /// actions in the color picker. On the wheel picker the color code can be
  /// edited to enter and select a color of a known RGB hex value. If the
  /// property [colorCodeReadOnly] has been set to false the color code field
  /// can never be edited directly, it is then only used to display the code
  /// of currently selected color.
  ///
  /// Defaults to false.
  final bool showColorCode;

  /// When true, the color code entry field uses the currently selected
  /// color as its background color.
  ///
  /// This makes the color code entry field a large current color indicator
  /// area, that changes color as the color value is changed.
  /// The text color of the field, will automatically adjust for best contrast,
  /// as will the opacity indicator text. Enabling this feature will override
  /// any color specified in [colorCodeTextStyle] and [colorCodePrefixStyle],
  /// but their styles will otherwise be kept as specified.
  ///
  /// Defaults to false.
  final bool colorCodeHasColor;

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
  final bool showEditIconButton;

  /// The icon to use on the edit icon button.
  ///
  /// Defaults to [Icons.edit].
  final IconData editIcon;

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
  final bool focusedEditHasNoColor;

  /// Text style for the displayed generic color name in the picker.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodyMedium`, if not defined.
  final TextStyle? colorCodeTextStyle;

  /// Old property, no longer in use. This property is now set via
  /// property [copyPasteBehavior] and [ColorPickerCopyPasteBehavior.copyIcon]
  @Deprecated('This property is deprecated and no longer has any function. '
      'It was removed in v2.0.0. To modify the copy icon on the color code '
      'entry field, define the `ColorPickerCopyPasteBehavior(copyIcon: '
      'myIcon)` and provide it via the `copyPasteBehavior` property.')
  final IconData? colorCodeIcon;

  /// The TextStyle of the prefix of the color code.
  ///
  /// The prefix always include the alpha value and may also include a num char
  /// '#' or '0x' based on the `ColorPickerCopyPasteBehavior.copyFormat`
  /// setting.
  ///
  /// Defaults to [colorCodeTextStyle], if not defined.
  final TextStyle? colorCodePrefixStyle;

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
  /// Regardless of the picker and [colorCodeReadOnly] value, you can change
  /// color value by pasting in a new value, if your copy paste configuration
  /// allows it.
  ///
  /// Defaults to false.
  final bool colorCodeReadOnly;

  /// Set to true to show the int [Color.value] of the selected [color].
  ///
  /// This is a developer feature, showing the int color value can be
  /// useful during software development. If enabled the value is shown after
  /// the color code. For text style it also uses the [colorCodeTextStyle].
  /// There is no copy button for the shown int value, but the value is
  /// displayed with a [SelectableText] widget, so it can be select painted
  /// and copied if so required.
  ///
  /// Defaults to false.
  final bool showColorValue;

  /// Set to true to a list of recently selected colors selection at the bottom
  /// of the picker.
  ///
  /// When `showRecentColors` is enabled, the color picker shows recently
  /// selected colors in a list at the bottom of the color picker. The list
  /// uses first-in, first-out to keep min 2 to max 20 colors (defaults to 5)
  /// on the recently used colors list, the desired max value can be modified
  /// with [maxRecentColors].
  ///
  /// Defaults to false.
  final bool showRecentColors;

  /// The maximum numbers of recent colors to show in the list of recent colors.
  ///
  /// The max recent colors must be from 2 to 20. Defaults to 5.
  final int maxRecentColors;

  /// A list with the recently select colors.
  ///
  /// Defaults to an empty list of colors. You can provide a starting
  /// set from some stored state if so desired.
  final List<Color> recentColors;

  /// Optional callback that returns the current list of recently selected
  /// colors.
  ///
  /// This optional callback is called every time a new color is added to the
  /// recent colors list with the complete current list of recently used colors.
  ///
  /// If the optional callback is not provided, then it is not called. You can
  /// use this callback to save and restore the recently used colors. To
  /// initialize the list when the color picker is created give it a starting
  /// via [recentColors]. This could be a list kept just in state during
  /// the current app session, or it could have been persisted and restored
  /// from a previous session.
  final ValueChanged<List<Color>>? onRecentColorsChanged;

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
  final bool enableTooltips;

  /// The color on the thumb of the slider that shows the selected picker type.
  ///
  /// If not defined, it defaults to `Color(0xFFFFFFFF)` (white) in light
  /// theme and to `Color(0xFF636366)` in dark theme, which are the defaults
  /// for the used [CupertinoSlidingSegmentedControl].
  ///
  /// If you give it a custom color, the color picker will automatically adjust
  /// the text color on the selected thumb for best legible text contrast.
  final Color? selectedPickerTypeColor;

  /// The TextStyle of the labels in segmented color picker type selector.
  ///
  /// Defaults to `Theme.of(context).textTheme.bodySmall`, if not defined.
  final TextStyle? pickerTypeTextStyle;

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
  ///  * [ColorPickerType.customSecondary] : 'Option'
  ///  * [ColorPickerType.wheel] : 'Wheel'
  final Map<ColorPickerType, String> pickerTypeLabels;

  /// Color swatch to name map, with custom swatches and their names.
  ///
  /// Used to provide custom [ColorSwatch] objects to the custom color picker,
  /// including their custom name label. These colors, their swatch shades
  /// and names, are shown and used when the picker type
  /// [ColorPickerType.custom] option is enabled in the color picker.
  ///
  /// Defaults to an empty map. If the map is empty, the custom colors picker
  /// will not be shown even if it is enabled in [pickersEnabled].
  final Map<ColorSwatch<Object>, String> customColorSwatchesAndNames;

  /// Color swatch to name map, with custom swatches and their names.
  ///
  /// Used to provide custom [ColorSwatch] objects to the custom color picker,
  /// including their custom name label. These colors, their swatch shades
  /// and names, are shown and used when the picker type
  /// [ColorPickerType.customSecondary] option is enabled in the color picker.
  ///
  /// Defaults to an empty map. If the map is empty, the custom colors picker
  /// will not be shown even if it is enabled in [pickersEnabled].
  final Map<ColorSwatch<Object>, String> customSecondaryColorSwatchesAndNames;

  /// English default label for picker with both primary and accent colors.
  static const String _selectBothLabel = 'Primary & Accent';

  /// English default label for picker with primary colors.
  static const String _selectPrimaryLabel = 'Primary';

  /// English default label for picker with accent colors.
  static const String _selectAccentLabel = 'Accent';

  /// English default label for picker with black and white shades.
  static const String _selectBlackWhiteLabel = 'Black & White';

  /// English default label for picker with custom defined colors.
  static const String _selectCustomLabel = 'Custom';

  /// English default label for picker with custom secondary defined colors.
  static const String _selectCustomSecondaryLabel = 'Option';

  /// English default label for the HSV wheel picker that can select any color.
  static const String _selectWheelAnyLabel = 'Wheel';

  @override
  State<ColorPicker> createState() => _ColorPickerState();

  /// Show the defined [ColorPicker] in a custom alert dialog.
  ///
  /// The [showPickerDialog] method is a convenience function to show the
  /// [ColorPicker] widget in a modal dialog. It re-implements the standard
  /// [showDialog] function with opinionated Cancel and OK buttons.
  ///
  /// If a [transitionBuilder] is provided the [showPickerDialog] instead uses
  /// a [showGeneralDialog] implementation to show the [ColorPicker], this
  /// allows for customization of the show animation.
  ///
  /// It also by default uses a lighter barrier color. This is useful if the
  /// color picker is used to dynamically change color of a widget or entire
  /// application theme, since we can then better see the impact of the color
  /// choice behind the modal dialog when the barrier is made almost fully
  /// transparent.
  ///
  /// Returns a Future bool, that resolves to true if the dialog is closed with
  /// OK action button, and to false if the cancel action was selected.
  /// Clicking outside the dialog also closes it and returns false.
  ///
  /// The actual color selected in the dialog is handled via the `onChange`
  /// callbacks of the [ColorPicker] instance.
  Future<bool> showPickerDialog(
    /// The dialog requires a BuildContext.
    BuildContext context, {
    /// Title of the color picker dialog, often omitted in favor of using a
    /// [title] and/or [heading] already defined in the [ColorPicker].
    Widget? title,

    /// Padding around the dialog title, if a title is used.
    /// Defaults to `EdgeInsets.zero`, since the title is normally omitted
    /// and provided via the `heading` property of the `ColorPicker` instead.
    final EdgeInsetsGeometry titlePadding = EdgeInsets.zero,

    /// Style for the text in the [title] of this [AlertDialog].
    ///
    /// If null, [DialogTheme.titleTextStyle] is used. If that's null,
    /// defaults to [TextTheme.titleLarge] of [ThemeData.textTheme].
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
    /// Defaults to null and follows ambient [AlertDialog] themed actions
    /// padding or [AlertDialog] default if not defined.
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
    /// Defaults to null and follows ambient [AlertDialog] themed buttons
    /// padding or [AlertDialog] default if not defined.
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
    final double? elevation,

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

    /// The background transparency color of the dialog barrier.
    ///
    /// Defaults to [Colors.black12] which is considerably lighter than the
    /// standard [Colors.black54] and allows us to see the impact of selected
    /// color on app behind the dialog. If this is not desired, set it back to
    /// [Colors.black54] when you call [showPickerDialog], or make it even more
    /// transparent.
    ///
    /// You can also make the barrier completely transparent.
    Color barrierColor = Colors.black12,

    /// If true, the dialog can be closed by clicking outside it.
    ///
    /// Defaults to true.
    bool barrierDismissible = true,

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
  }) async {
    assert(debugCheckHasMaterialLocalizations(context),
        'A context with Material localizations is required');
    // Get the Material localizations.
    final MaterialLocalizations translate = MaterialLocalizations.of(context);

    // Make the dialog OK button.
    final String okButtonLabel =
        actionButtons.dialogOkButtonLabel ?? translate.okButtonLabel;
    final Text okButtonContent = Text(okButtonLabel);
    Widget okButton;
    switch (actionButtons.dialogOkButtonType) {
      case ColorPickerActionButtonType.text:
        okButton = actionButtons.dialogActionIcons
            ? TextButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                icon: Icon(actionButtons.okIcon),
                label: okButtonContent,
              )
            : TextButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                child: okButtonContent,
              );
      case ColorPickerActionButtonType.outlined:
        okButton = actionButtons.dialogActionIcons
            ? OutlinedButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                icon: Icon(actionButtons.okIcon),
                label: okButtonContent,
              )
            : OutlinedButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                child: okButtonContent,
              );
      case ColorPickerActionButtonType.elevated:
        okButton = actionButtons.dialogActionIcons
            ? ElevatedButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                icon: Icon(actionButtons.okIcon),
                label: okButtonContent,
              )
            : ElevatedButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                child: okButtonContent,
              );
      case ColorPickerActionButtonType.filled:
        okButton = actionButtons.dialogActionIcons
            ? FilledButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                icon: Icon(actionButtons.okIcon),
                label: okButtonContent,
              )
            : FilledButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                child: okButtonContent,
              );
      case ColorPickerActionButtonType.filledTonal:
        okButton = actionButtons.dialogActionIcons
            ? FilledButton.tonalIcon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                icon: Icon(actionButtons.okIcon),
                label: okButtonContent,
              )
            : FilledButton.tonal(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(true);
                },
                child: okButtonContent,
              );
    }

    // Make the dialog OK button.
    final String cancelButtonLabel =
        actionButtons.dialogCancelButtonLabel ?? translate.cancelButtonLabel;
    final Widget cancelButtonContent = Text(cancelButtonLabel);
    Widget cancelButton;
    switch (actionButtons.dialogCancelButtonType) {
      case ColorPickerActionButtonType.text:
        cancelButton = actionButtons.dialogActionIcons
            ? TextButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                icon: Icon(actionButtons.closeIcon),
                label: cancelButtonContent,
              )
            : TextButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                child: cancelButtonContent,
              );
      case ColorPickerActionButtonType.outlined:
        cancelButton = actionButtons.dialogActionIcons
            ? OutlinedButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                icon: Icon(actionButtons.closeIcon),
                label: cancelButtonContent,
              )
            : OutlinedButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                child: cancelButtonContent,
              );
      case ColorPickerActionButtonType.elevated:
        cancelButton = actionButtons.dialogActionIcons
            ? ElevatedButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                icon: Icon(actionButtons.closeIcon),
                label: cancelButtonContent,
              )
            : ElevatedButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                child: cancelButtonContent,
              );
      case ColorPickerActionButtonType.filled:
        cancelButton = actionButtons.dialogActionIcons
            ? FilledButton.icon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                icon: Icon(actionButtons.closeIcon),
                label: cancelButtonContent,
              )
            : FilledButton(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                child: cancelButtonContent,
              );
      case ColorPickerActionButtonType.filledTonal:
        cancelButton = actionButtons.dialogActionIcons
            ? FilledButton.tonalIcon(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                icon: Icon(actionButtons.closeIcon),
                label: cancelButtonContent,
              )
            : FilledButton.tonal(
                onPressed: () {
                  Navigator.of(context,
                          rootNavigator: actionButtons.useRootNavigator)
                      .pop(false);
                },
                child: cancelButtonContent,
              );
    }

    // False if dialog cancelled, true if color selected
    bool colorWasSelected = false;

    // Determine OK-Cancel button order.
    final TargetPlatform platform = Theme.of(context).platform;
    final bool okIsLeft = (platform == TargetPlatform.windows &&
            actionButtons.dialogActionOrder ==
                ColorPickerActionButtonOrder.adaptive) ||
        actionButtons.dialogActionOrder ==
            ColorPickerActionButtonOrder.okIsLeft;

    // Put or [ColorPicker] instance `this` in an `AlertDialog` using all
    // to it assigned and above defined properties.
    Widget dialog = AlertDialog(
      title: title,
      titlePadding: titlePadding,
      titleTextStyle: titleTextStyle,
      content: constraints == null
          ? this
          : ConstrainedBox(
              constraints: constraints,
              child: this,
            ),
      contentPadding: contentPadding,
      actions: actionButtons.dialogActionButtons
          ? <Widget>[
              if (okIsLeft) ...<Widget>[
                okButton,
                if (!actionButtons.dialogActionOnlyOkButton) cancelButton,
              ] else ...<Widget>[
                if (!actionButtons.dialogActionOnlyOkButton) cancelButton,
                okButton,
              ]
            ]
          : null,
      actionsPadding: actionsPadding,
      buttonPadding: buttonPadding,
      backgroundColor: backgroundColor,
      elevation: elevation,
      shadowColor: shadowColor,
      surfaceTintColor: surfaceTintColor,
      semanticLabel: semanticLabel,
      insetPadding: insetPadding,
      clipBehavior: clipBehavior,
      shape: shape,
      scrollable: true,
    );

    // No `transitionBuilder` give, then use
    // the platform and Material2/3 dependent default MaterialPage route
    // transition via `showDialog`, as in all versions before 3.0.0.
    if (transitionBuilder == null) {
      await showDialog<bool>(
          context: context,
          barrierDismissible: barrierDismissible,
          barrierColor: barrierColor,
          barrierLabel: barrierLabel,
          useSafeArea: useSafeArea,
          useRootNavigator: actionButtons.useRootNavigator,
          routeSettings: routeSettings,
          anchorPoint: anchorPoint,
          builder: (BuildContext context) {
            return dialog;
          }).then((bool? value) {
        // If the dialog return value was null, then we got here by a
        // barrier dismiss, then we set the return value to false.
        colorWasSelected = value ?? false;
      });
    }
    // If a `transitionBuilder` is given, we use `showGeneralDialog` using the
    // given `transitionBuilder` and a custom `pageBuilder`, conditionally
    // wrapping `SafeArea` around or AlertDialog widget and capturing the
    // current theme that we and wrapping it around the page builder.
    else {
      final CapturedThemes themes = InheritedTheme.capture(
        from: context,
        to: Navigator.of(
          context,
          rootNavigator: actionButtons.useRootNavigator,
        ).context,
      );
      if (useSafeArea) {
        dialog = SafeArea(child: dialog);
      }
      await showGeneralDialog<bool>(
        context: context,
        barrierDismissible: barrierDismissible,
        barrierColor: barrierColor,
        barrierLabel: barrierLabel ??
            MaterialLocalizations.of(context).modalBarrierDismissLabel,
        useRootNavigator: actionButtons.useRootNavigator,
        routeSettings: routeSettings,
        anchorPoint: anchorPoint,
        transitionBuilder: transitionBuilder,
        transitionDuration: transitionDuration,
        pageBuilder: (BuildContext context, Animation<double> animation1,
            Animation<double> animation2) {
          return themes.wrap(Builder(
            builder: (BuildContext context) => dialog,
          ));
        },
      ).then((bool? value) {
        // If the dialog return value was null, then we got here by a
        // barrier dismiss, then we set the return value to false.
        colorWasSelected = value ?? false;
      });
    }
    return colorWasSelected;
  }
}

class _ColorPickerState extends State<ColorPicker> {
  // Focus node the entire color picker.
  late FocusNode _focusNode;
  // Focus node of the picker selector.
  late FocusNode _pickerFocusNode;
  // Focus node of the opacity slider.
  late FocusNode _opacityFocusNode;

  // The currently active used list of color swatches we select
  // the active color from
  late List<ColorSwatch<Object>> _activeColorSwatchList;

  // The active Swatch in the active Color swatch List. Can be null temporarily
  // when we searched for a color in a swatch but did not find it anywhere.
  ColorSwatch<Object>? _activeSwatch;

  // Which picker are we using now.
  late ColorPickerType _activePicker;

  // Current selected color, it never has opacity. The slider sets opacity to
  // in the _opacity property.
  late Color _selectedColor;

  // A color that is tapped on in indicators. This color may have opacity if
  // the source color has opacity.
  late Color _tappedColor;

  // Current color opacity value, this is the opacity set by the slider.
  late double _opacity;

  // Color picker indicator selected item should request focus.
  bool _selectedShouldFocus = true;

  // The edit code field has focus. When it does, paste entries
  // use the parse and paste logic if `codeEntryParsedPaste` is false.
  // This ColorCodeField focus status is used together with the
  // `codeEntryParsedPaste` to determine if we should paste with parser or not.
  bool _editCodeFocused = false;

  // Edit color code field should update? Whenever the edit code field is
  // updated outside the edit field, we need to send a signal back that it
  // should update.
  bool _editShouldUpdate = true;

  // Set to true if wheel picker should update. Whenever the wheel picker is
  // updated outside the wheel picker, we need to send a signal back that it
  // should update.
  bool _wheelShouldUpdate = true;

  // The tonal picker should only update its tonal palette whe we click on
  // colors in other color picker, not when we select a color in the
  // tonal palette. This local state is used to send the update signal.
  bool _tonalShouldUpdate = true;

  // This is set to true when a tonal palette color is selected, ie operated on.
  // If one is selected, we do not get main color selection indicator
  // unless it also matches main color [500], if it is found in the
  // sub palette, it is is not selected.
  bool _tonalOperated = false;

  // Set to true when widget update triggered via internal state change.
  bool _fromInternal = false;

  // Color wheel picker should request focus.
  bool _wheelShouldFocus = false;

  // Set to true when we are drag and operating the wheel picker.
  bool _onWheel = false;

  // Set to true when edit icon is taped and edit field shuld focus
  bool _requestEditFocus = false;

  // Becomes true when we have more than one ColorPickerType available in
  // the `widget.pickersEnabled` property. If there is just one picker enabled
  // then that picker will be used, but we will not use the picker type
  // selector at all. If no picker is enabled, the material picker will be
  // used anyway, but there is no picker selector in that case either.
  bool _usePickerSelector = false;

  // We need a map we can guarantee has no gaps, so we make a local
  // version of it that is always complete.
  late Map<ColorPickerType, bool> _pickers;

  // The current state of the list with the recent color selections.
  late List<Color> _recentColors;

  // Map of swatch names in corresponding list of color swatches.
  late Map<ColorPickerType, List<ColorSwatch<Object>>> _typeToSwatchMap;

  // A ColorPickerType to String map that contains labels for picker types,
  // with the default label strings in English applied as fallback.
  late Map<ColorPickerType, String> _pickerLabels;

  @override
  void initState() {
    super.initState();
    // We always remove alpha from incoming color.
    _selectedColor = widget.color.withAlpha(0xFF);
    _tappedColor = widget.color;
    // Opacity is captured in _opacity if enabled.
    _opacity = widget.enableOpacity ? widget.color.opacity : 1;
    // Picker labels, use english fallbacks if none provided.
    _pickerLabels = <ColorPickerType, String>{
      ColorPickerType.both: widget.pickerTypeLabels[ColorPickerType.both] ??
          ColorPicker._selectBothLabel,
      ColorPickerType.primary:
          widget.pickerTypeLabels[ColorPickerType.primary] ??
              ColorPicker._selectPrimaryLabel,
      ColorPickerType.accent: widget.pickerTypeLabels[ColorPickerType.accent] ??
          ColorPicker._selectAccentLabel,
      ColorPickerType.bw: widget.pickerTypeLabels[ColorPickerType.bw] ??
          ColorPicker._selectBlackWhiteLabel,
      ColorPickerType.custom: widget.pickerTypeLabels[ColorPickerType.custom] ??
          ColorPicker._selectCustomLabel,
      ColorPickerType.customSecondary:
          widget.pickerTypeLabels[ColorPickerType.customSecondary] ??
              ColorPicker._selectCustomSecondaryLabel,
      ColorPickerType.wheel: widget.pickerTypeLabels[ColorPickerType.wheel] ??
          ColorPicker._selectWheelAnyLabel,
    };
    // A map with the picker type enum as key to a color swatch list.
    _typeToSwatchMap = <ColorPickerType, List<ColorSwatch<Object>>>{
      ColorPickerType.both: ColorTools.primaryAndAccentColors,
      ColorPickerType.primary: ColorTools.primaryColors,
      ColorPickerType.accent: ColorTools.accentColors,
      ColorPickerType.bw: ColorTools.blackAndWhite,
      ColorPickerType.custom: widget.customColorSwatchesAndNames.keys.toList(),
      ColorPickerType.customSecondary:
          widget.customSecondaryColorSwatchesAndNames.keys.toList(),
      ColorPickerType.wheel: <ColorSwatch<Object>>[
        // Make a swatch of the selected color in the wheel.
        ColorTools.primarySwatch(_selectedColor)
      ],
    };
    // Enabled color pickers, with defaults if not specified.
    _pickers = <ColorPickerType, bool>{
      ColorPickerType.both:
          widget.pickersEnabled[ColorPickerType.both] ?? false,
      ColorPickerType.primary:
          widget.pickersEnabled[ColorPickerType.primary] ?? true,
      ColorPickerType.accent:
          widget.pickersEnabled[ColorPickerType.accent] ?? true,
      ColorPickerType.bw: widget.pickersEnabled[ColorPickerType.bw] ?? false,
      ColorPickerType.custom:
          // Custom picker is always disabled if no custom swatches are given.
          (widget.pickersEnabled[ColorPickerType.custom] ?? false) &&
              widget.customColorSwatchesAndNames.isNotEmpty,
      ColorPickerType.customSecondary:
          // Custom secondary is always disabled if no custom swatches are given
          (widget.pickersEnabled[ColorPickerType.customSecondary] ?? false) &&
              widget.customSecondaryColorSwatchesAndNames.isNotEmpty,
      ColorPickerType.wheel:
          widget.pickersEnabled[ColorPickerType.wheel] ?? false,
    };
    // Define used focus nodes.
    _focusNode = FocusNode();
    _pickerFocusNode = FocusNode();
    _opacityFocusNode = FocusNode();
    // Always update the wheel and edit field when ColorPicker is initialized.
    _wheelShouldUpdate = true;
    _editShouldUpdate = true;
    // Always update tonal when ColorPicker is initialized.
    _tonalShouldUpdate = true;
    _tonalOperated = false;
    _fromInternal = false;
    // If there are no shade or tonal colors displayed, the wheel must
    // focus on init.
    _wheelShouldFocus =
        !widget.enableShadesSelection && !widget.enableTonalPalette;
    // Init the list of the recently used colors to their initial value.
    _recentColors = <Color>[...widget.recentColors];
    // Find the best color picker to show the current selectedColor value.
    _findPicker();
    // And update to the active swatch related to the selected color.
    _activeColorSwatchList = _typeToSwatchMap[_activePicker]!;
    _updateActiveSwatch();
  }

  @override
  void didUpdateWidget(ColorPicker oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_debug) {
      debugPrint('didUpdateWidget called: fromInternal: $_fromInternal'
          ' ******************************');
    }
    // Set to true if a change was done where we need to find the picker again.
    bool shouldFindPickerAndSwatch = false;
    // Opacity enable/disable changed, update selected color and opacity.
    if (widget.enableOpacity != oldWidget.enableOpacity) {
      _opacity = widget.enableOpacity ? widget.color.opacity : 1;
      if (_debug) {
        debugPrint('didUpdateWidget changed: enableOpacity = '
            '${widget.enableOpacity == oldWidget.enableOpacity}'
            ' opacity=$_opacity');
      }
    }
    // The color was updated externally, update to new color and find picker.
    if (widget.color != _selectedColor || widget.color != _tappedColor) {
      if (_debug) {
        debugPrint('didUpdateWidget changed color: '
            'color=${widget.color} selectedColor=$_selectedColor');
      }
      _selectedColor = widget.color.withAlpha(0xFF);
      _opacity = widget.enableOpacity ? widget.color.opacity : 1;
      // Make a swatch too be to find it on wheel, if color is there.
      _typeToSwatchMap[ColorPickerType.wheel] = <ColorSwatch<Object>>[
        ColorTools.createPrimarySwatch(_selectedColor),
      ];
      if (!_fromInternal) {
        _tappedColor = _selectedColor;
        // Wheel and edit needs to update.
        _wheelShouldUpdate = true;
        _editShouldUpdate = true;
        // Tonal picker should update from external change.
        _tonalShouldUpdate = true;
        _tonalOperated = false;
        // We need to find the right picker again.
        shouldFindPickerAndSwatch = true;
      }
    }
    // Picker labels map changed, update used one, with its default fallbacks.
    if (!mapEquals(widget.pickerTypeLabels, oldWidget.pickerTypeLabels)) {
      if (_debug) {
        debugPrint(
          'didUpdateWidget pickerTypeLabels mapEquals: '
          '${mapEquals(widget.pickerTypeLabels, oldWidget.pickerTypeLabels)}',
        );
      }
      _pickerLabels = <ColorPickerType, String>{
        ColorPickerType.both: widget.pickerTypeLabels[ColorPickerType.both] ??
            ColorPicker._selectBothLabel,
        ColorPickerType.primary:
            widget.pickerTypeLabels[ColorPickerType.primary] ??
                ColorPicker._selectPrimaryLabel,
        ColorPickerType.accent:
            widget.pickerTypeLabels[ColorPickerType.accent] ??
                ColorPicker._selectAccentLabel,
        ColorPickerType.bw: widget.pickerTypeLabels[ColorPickerType.bw] ??
            ColorPicker._selectBlackWhiteLabel,
        ColorPickerType.custom:
            widget.pickerTypeLabels[ColorPickerType.custom] ??
                ColorPicker._selectCustomLabel,
        ColorPickerType.customSecondary:
            widget.pickerTypeLabels[ColorPickerType.customSecondary] ??
                ColorPicker._selectCustomSecondaryLabel,
        ColorPickerType.wheel: widget.pickerTypeLabels[ColorPickerType.wheel] ??
            ColorPicker._selectWheelAnyLabel,
      };
    }
    // Pickers customColorSwatchesAndNames map changed, or pickersEnabled map
    // changed, they depend on each other, so we always update state of both.
    if (widget.customColorSwatchesAndNames.toString() !=
            oldWidget.customColorSwatchesAndNames.toString() ||
        !mapEquals(widget.pickersEnabled, oldWidget.pickersEnabled)) {
      // In above un-equality check, the mapEquals, or with map != operator,
      // does not work if you provide a map made with createPrimarySwatch or
      // createAccentSwatch. The Wheel Picker does not work entirely as
      // intended if the above IF is evaluated incorrectly. The wrong result
      // will be noticed seen as that Wheel picker will not be kept active
      // when selecting known swatch color in it. It will instead move to
      // the picker to Swatch where the color exists. This is not desired.
      // We want to stay on the Wheel in this case. If you see the
      // wrong behavior it is due to the above IF being evaluated incorrectly.
      // When comparing the `customColorSwatchesAndNames` new and old Widget
      // maps toString results, it was observed that they were equal for the
      // problem use cases, while the [mapEquals] or == operator was not.
      // Therefore using `toString` comparisons for now to get around the issue,
      // not ideal, but it works OK.
      if (_debug) {
        debugPrint('didUpdateWidget pickersEnabled or custom swatch updated!');
      }
      // Update _typeToSwatchMap, because custom color swatches were updated.
      _typeToSwatchMap = <ColorPickerType, List<ColorSwatch<Object>>>{
        ColorPickerType.both: ColorTools.primaryAndAccentColors,
        ColorPickerType.primary: ColorTools.primaryColors,
        ColorPickerType.accent: ColorTools.accentColors,
        ColorPickerType.bw: ColorTools.blackAndWhite,
        ColorPickerType.custom: widget.customColorSwatchesAndNames.keys
            .toList(), // Use empty map if no custom swatch given.
        ColorPickerType.customSecondary: widget
            .customSecondaryColorSwatchesAndNames.keys
            .toList(), // Use empty map if no custom swatch given.
        ColorPickerType.wheel: <ColorSwatch<Object>>[
          // Make a swatch of the selected color in the wheel.
          // If color has opacity, it will make a swatch of the color with
          // same opacity in it as well.
          ColorTools.primarySwatch(_selectedColor)
        ],
      };
      // Update enabled color pickers, with defaults if none given, depends
      // on customColorSwatchesAndNames, so we need to update this also when
      // it changes, not just the enabled pickers.
      _pickers = <ColorPickerType, bool>{
        ColorPickerType.both:
            widget.pickersEnabled[ColorPickerType.both] ?? false,
        ColorPickerType.primary:
            widget.pickersEnabled[ColorPickerType.primary] ?? true,
        ColorPickerType.accent:
            widget.pickersEnabled[ColorPickerType.accent] ?? true,
        ColorPickerType.bw: widget.pickersEnabled[ColorPickerType.bw] ?? false,
        ColorPickerType.custom:
            // Custom picker is always disabled if no custom swatches are given.
            (widget.pickersEnabled[ColorPickerType.custom] ?? false) &&
                widget.customColorSwatchesAndNames.isNotEmpty,
        ColorPickerType.customSecondary:
            // Custom second is always disabled if no custom swatches are given.
            (widget.pickersEnabled[ColorPickerType.customSecondary] ?? false) &&
                widget.customSecondaryColorSwatchesAndNames.isNotEmpty,
        ColorPickerType.wheel:
            widget.pickersEnabled[ColorPickerType.wheel] ?? false,
      };
      if (_debug) {
        debugPrint('${widget.customColorSwatchesAndNames}');
        debugPrint('${oldWidget.customColorSwatchesAndNames}');
      }
      // We should find picker and swatch after above updates.
      shouldFindPickerAndSwatch = true;
    }
    // If the recent colors that is stored in local state version is
    // not equal to the one passed in, then the picker got a new externally
    // provided recent colors list, and it should replace the current local
    // state stored list with the new one. This is probably a fairly rare
    // use-case, but let's  support it anyway.
    if (!listEquals(widget.recentColors, _recentColors)) {
      _recentColors = <Color>[...widget.recentColors];
    }
    // Last find picker and swatch, if the flag to do so is set.
    if (shouldFindPickerAndSwatch) {
      if (_debug) debugPrint('didUpdateWidget shouldFindPickerAndSwatch');
      _findPicker();
      _updateActiveSwatch();
    }
    _fromInternal = false;
  }

  // Find the best matching picker of available ones to show selected color.
  void _findPicker() {
    // The selected color indicator in the picker should request focus.
    _selectedShouldFocus = true;
    // We use the picker selector segment control only if more than one picker
    // is enabled in the color picker. If anybody ever reads this comment,
    // I admit, this kind of logic is a bit tricky. Imo looping over the items
    // and counting the ones that are true and returning true if count is > 1,
    // is imo more understandable, but this was interesting to try, and it does
    // the same thing! :)
    _usePickerSelector =
        _pickers.values.fold<int>(0, (int t, bool e) => t + (e ? 1 : 0)) > 1;
    // If we have a picker selector, we get the best one of the enabled ones,
    // to show the current selectedColor.
    if (_usePickerSelector) {
      _activePicker = findColorInSelector(
        color: _tappedColor,
        typeToSwatchMap: _typeToSwatchMap,
        pickersEnabled: _pickers,
        lookInShades: widget.enableShadesSelection,
        include850: widget.includeIndex850,
      );
    }
    // If we don't have segment control selector, we use the only swatch
    // selection that is still true and will not show a picker selector at all.
    else {
      if (_pickers[ColorPickerType.both]!) {
        _activePicker = ColorPickerType.both;
      } else if (_pickers[ColorPickerType.primary]!) {
        _activePicker = ColorPickerType.primary;
      } else if (_pickers[ColorPickerType.accent]!) {
        _activePicker = ColorPickerType.accent;
      } else if (_pickers[ColorPickerType.bw]!) {
        _activePicker = ColorPickerType.bw;
      } else if (_pickers[ColorPickerType.custom]!) {
        _activePicker = ColorPickerType.custom;
      } else if (_pickers[ColorPickerType.customSecondary]!) {
        _activePicker = ColorPickerType.customSecondary;
      } else if (_pickers[ColorPickerType.wheel]!) {
        _activePicker = ColorPickerType.wheel;
      }
      // If they were all false, we show the Material primary swatches picker.
      else {
        _activePicker = ColorPickerType.primary;
      }
    }
    // We need to set focus on the wheel when it is the selected picker
    // and the shades that would normally grab focus are not shown. If we do
    // no do this, focus may be on Edit input field, and if it is focused
    // the virtual keyboard will appear when wheel is displayed. We do not
    // want that to happen until the user clicks on the edit field.
    if (_activePicker == ColorPickerType.wheel &&
        !widget.enableShadesSelection &&
        !widget.enableTonalPalette) {
      _wheelShouldFocus = true;
    }
  }

  // Update shades swatch to the correct swatch for the selected color.
  void _updateActiveSwatch() {
    // The typical case is that we have a normal swatch where we need to
    // find the swatch that contains the selectedColor.
    if (_activePicker != ColorPickerType.wheel && !_tonalOperated) {
      // Get list of color swatches from the map for the active picker.
      _activeColorSwatchList = _typeToSwatchMap[_activePicker]!;

      if (_debug) {
        debugPrint('_updateActiveSwatch _tappedColor= $_tappedColor');
      }
      // Find the swatch that selected color belongs to from the swatches in
      // the active picker and set this swatch as _activeSwatch.
      _activeSwatch = findColorSwatch(
        _tappedColor,
        _activeColorSwatchList,
        widget.includeIndex850,
      ) as ColorSwatch<Object>?;
      if (_debug) {
        debugPrint('_updateActiveSwatch _activeSwatch= $_activeSwatch');
      }
      // For the wheel picker we need to check if the selected color belongs to
      // a pre-defined swatch and if it does, return that as the active swatch.
      // If the selected color does not belong to any pre-defined color swatch,
      // then we compute a color swatch for it.
    } else {
      if (ColorTools.isAccentColor(_tappedColor)) {
        _activeSwatch = ColorTools.accentSwatch(_tappedColor);
      } else if (ColorTools.isPrimaryColor(_tappedColor)) {
        _activeSwatch = ColorTools.primarySwatch(_tappedColor);
      } else if (ColorTools.isBlackAndWhiteColor(_tappedColor)) {
        _activeSwatch = ColorTools.blackAndWhiteSwatch(_tappedColor);
      } else if (ColorTools.isCustomColor(
          _tappedColor, widget.customColorSwatchesAndNames)) {
        _activeSwatch = ColorTools.customSwatch(
            _tappedColor, widget.customColorSwatchesAndNames);
      } else if (ColorTools.isCustomColor(
          _tappedColor, widget.customSecondaryColorSwatchesAndNames)) {
        _activeSwatch = ColorTools.customSwatch(
            _tappedColor, widget.customSecondaryColorSwatchesAndNames);
      } else {
        _activeSwatch = ColorTools.customSwatch(
            _tappedColor, widget.customSecondaryColorSwatchesAndNames);
      }
    }
    // We did not find the selected color in any active swatch list, in that
    // case we set active swatch to the first swatch in active list, just
    // to get an active swatch selection. This is a fall back from an error
    // situation where a selected color was passed to the color picker that was
    // not found in any of the provided swatches in any enabled pickers.
    // If the wheel picker is enabled, then the color will always be found
    // in it as a last resort, if not we may end up here, using the first
    // swatch in the list as the active swatch.
    _activeSwatch ??= _activeColorSwatchList[0];
  }

  @override
  void dispose() {
    _focusNode.dispose();
    _pickerFocusNode.dispose();
    _opacityFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // The effective used text style, if null was passed in we assign defaults.
    final TextStyle effectiveMaterialNameStyle =
        (widget.materialNameTextStyle ??
                Theme.of(context).textTheme.bodyMedium) ??
            const TextStyle();
    final TextStyle effectiveGenericNameStyle =
        (widget.colorNameTextStyle ?? Theme.of(context).textTheme.bodyMedium) ??
            const TextStyle();
    // Set the default integer code value text style to bodyMedium if not given.
    final TextStyle effectiveCodeStyle =
        (widget.colorCodeTextStyle ?? Theme.of(context).textTheme.bodyMedium) ??
            const TextStyle();
    // The logic below is used to determine if we will have a context menu
    // present at all in the Widget tree.
    final bool useContextMenu = widget.copyPasteBehavior.longPressMenu ||
        widget.copyPasteBehavior.secondaryMenu ||
        widget.copyPasteBehavior.secondaryOnDesktopLongOnDevice ||
        widget.copyPasteBehavior.secondaryOnDesktopLongOnDeviceAndWeb;
    // Should keyboard listener grab focus? If neither copy and paste keyboard
    // shortcuts are enabled, there is no need to autofocus, so let's skip it
    // then too, regardless of autofocus setting.
    final bool autoFocus = widget.copyPasteBehavior.autoFocus &&
        (widget.copyPasteBehavior.ctrlC || widget.copyPasteBehavior.ctrlV);

    // Weather to show the edit icon button, if color code is enabled and
    // not read only, and the wheel picker is active.
    final bool showEditIconButton = widget.showEditIconButton &&
        widget.showColorCode &&
        !widget.colorCodeReadOnly &&
        _activePicker == ColorPickerType.wheel;

    if (_debug) {
      debugPrint('Build color=${widget.color} selectedColor=$_selectedColor');
    }
    // Use a copy paste handler to handle copy and paste keyboard shortcuts,
    // and also to handle the context menu for copy and paste.
    return CopyPasteHandler(
      pasteFromClipboard: _getClipboard,
      copyToClipboard: _setClipboard,
      useContextMenu: useContextMenu,
      useLongPress: widget.copyPasteBehavior.longPressMenu,
      useSecondaryTapDown: widget.copyPasteBehavior.secondaryMenu,
      useSecondaryOnDesktopLongOnDevice:
          widget.copyPasteBehavior.secondaryOnDesktopLongOnDevice,
      useSecondaryOnDesktopLongOnDeviceAndWeb:
          widget.copyPasteBehavior.secondaryOnDesktopLongOnDeviceAndWeb,
      onCopyPasteMenuOpened: () {
        // If we were on the wheel when the menu got opened, it's
        // operation got cancelled by the context menu and we need to
        // call onColorChangeEnd event with the selected color.
        if (_onWheel) {
          widget.onColorChangeEnd?.call(_selectedColor);
          // We set onWheel to false as well, as we are no longer on
          // the wheel and we handled the event.
          setState(() {
            _onWheel = false;
          });
        }
      },
      focusNode: _focusNode,
      autoFocus: autoFocus,
      // If edit color code is focused and we do not use the parsed paste
      // feature, pass in that info and will not use the color code paste
      // parser. The TextField's normal paste action will then handle
      // the paste as before in v1.x and normally in a TextField. With the
      // difference that this particular TextField will still filter out
      // all none valid RGB color code chars and limit the length.
      noPasteIntent:
          _editCodeFocused && !widget.copyPasteBehavior.editUsesParsedPaste,
      child: Padding(
        padding: widget.padding,
        child: Column(
          mainAxisSize: widget.mainAxisSize,
          crossAxisAlignment: widget.crossAxisAlignment,
          children: <Widget>[
            // Show title bar widget if we have one.
            if (widget.title != null ||
                widget.copyPasteBehavior.copyButton ||
                widget.copyPasteBehavior.pasteButton ||
                widget.actionButtons.okButton ||
                widget.actionButtons.closeButton)
              Padding(
                padding: EdgeInsets.only(
                    bottom: widget.toolbarSpacing ?? widget.columnSpacing),
                child: ColorPickerToolbar(
                  title: widget.title,
                  onCopy: widget.copyPasteBehavior.copyButton
                      ? _setClipboard
                      : null,
                  onPaste: widget.copyPasteBehavior.pasteButton
                      ? _getClipboard
                      : null,
                  onOk: widget.actionButtons.okButton
                      ? () {
                          // OK was pressed, we pop and return TRUE.
                          // In case this was not used in a dialog the
                          // canPop will at least avoid a crash, but may
                          // still do the wrong thing.
                          if (Navigator.of(context,
                                  rootNavigator:
                                      widget.actionButtons.useRootNavigator)
                              .canPop()) {
                            Navigator.of(context,
                                    rootNavigator:
                                        widget.actionButtons.useRootNavigator)
                                .pop(true);
                          }
                        }
                      : null,
                  onClose: widget.actionButtons.closeButton
                      ? () {
                          // Cancel was pressed, we pop and return FALSE.
                          // In case this was not used in a dialog the
                          // canPop will at least avoid a crash, but may
                          // still do the wrong thing.
                          if (Navigator.of(context,
                                  rootNavigator:
                                      widget.actionButtons.useRootNavigator)
                              .canPop()) {
                            Navigator.of(context,
                                    rootNavigator:
                                        widget.actionButtons.useRootNavigator)
                                .pop(false);
                          }
                        }
                      : null,
                  toolIcons: widget.actionButtons,
                  copyPasteBehavior: widget.copyPasteBehavior,
                  enableTooltips: widget.enableTooltips,
                ),
              ),
            // Show heading widget if we have one.
            if (widget.heading != null)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.heading,
              ),
            // Show picker selector, if more than one picker is enabled.
            if (_usePickerSelector)
              Focus(
                focusNode: _pickerFocusNode,
                child: SelectPicker(
                  pickers: _pickers,
                  pickerLabels: _pickerLabels,
                  picker: _activePicker,
                  onPickerChanged: (ColorPickerType value) {
                    // If there is no color in the picker that will grab
                    // focus when we move to the picker, then the picker
                    // selector itself will keep focus, and kick the key
                    // events (copy/paste) to the handler, we can thus use
                    // CTRL-C/V also when picker selector has focus.
                    _pickerFocusNode.requestFocus();
                    setState(() {
                      _activePicker = value;
                      // If there is a color indicator, it will grab focus.
                      _selectedShouldFocus = true;
                      // If we are on the wheel no shades selection is not
                      // enabled, then the wheel will grab the focus.
                      if (_activePicker == ColorPickerType.wheel &&
                          !widget.enableShadesSelection &&
                          !widget.enableTonalPalette) {
                        _wheelShouldFocus = true;
                      }
                      _tonalOperated = false;
                      _updateActiveSwatch();
                    });
                  },
                  thumbColor: widget.selectedPickerTypeColor,
                  textStyle: widget.pickerTypeTextStyle,
                  columnSpacing: widget.columnSpacing,
                ),
              ),
            // Add a tiny bit of extra hard coded space after the picker
            // type selector if there was one.
            if (_usePickerSelector) const SizedBox(height: 4),
            // This is not the Wheel case, so we draw all the main colors
            // for the active swatch list.
            if (_activePicker != ColorPickerType.wheel)
              MainColors(
                spacing: widget.spacing,
                runSpacing: widget.runSpacing,
                columnSpacing: widget.columnSpacing,
                activeColorSwatchList: _activeColorSwatchList,
                selectedColor:
                    widget.enableOpacity ? _selectedColor : _tappedColor,
                onSelectColor: (Color color) {
                  _tonalOperated = false;
                  _onSelectColor(color);
                },
                includeIndex850: widget.includeIndex850,
                width: widget.width,
                height: widget.height,
                borderRadius: widget.borderRadius,
                hasBorder: widget.hasBorder,
                borderColor: widget.borderColor,
                elevation: widget.elevation,
                selectedColorIcon: widget.selectedColorIcon,
                selectedRequestsFocus: _selectedShouldFocus,
              ),
            // This is the wheel case, draw the custom ColorWheelPicker.
            if (_activePicker == ColorPickerType.wheel)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: SizedBox(
                  height: widget.wheelDiameter,
                  width: widget.wheelDiameter,
                  child: ColorWheelPicker(
                    color: _selectedColor,
                    wheelWidth: widget.wheelWidth,
                    wheelSquarePadding: widget.wheelSquarePadding,
                    wheelSquareBorderRadius: widget.wheelSquareBorderRadius,
                    hasBorder: widget.wheelHasBorder,
                    borderColor: widget.borderColor,
                    shouldUpdate: _wheelShouldUpdate,
                    shouldRequestsFocus: _wheelShouldFocus,
                    onChangeStart: (Color color) {
                      widget.onColorChangeStart
                          ?.call(color.withOpacity(_opacity));
                      _addToRecentColors(color.withOpacity(_opacity));
                    },
                    onChanged: (Color color) {
                      setState(() {
                        _fromInternal = true;
                        _tappedColor = color;
                        _selectedColor = color;
                        _wheelShouldUpdate = false;
                        _editShouldUpdate = true;
                        _tonalShouldUpdate = true;
                        _tonalOperated = false;
                        _selectedShouldFocus = true;
                        _wheelShouldFocus = false;
                        _updateActiveSwatch();
                      });
                      widget
                          .onColorChanged(_selectedColor.withOpacity(_opacity));
                    },
                    onChangeEnd: (Color color) {
                      widget.onColorChangeEnd?.call(
                        color.withOpacity(_opacity),
                      );
                    },
                    onWheel: (bool value) {
                      setState(() {
                        _onWheel = value;
                      });
                    },
                  ),
                ),
              ),
            // Show the sub-heading for the none wheel case.
            if (widget.subheading != null &&
                widget.enableShadesSelection &&
                _activePicker != ColorPickerType.wheel)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.subheading,
              ),
            // Show the sub-heading for the wheel case.
            if (widget.wheelSubheading != null &&
                widget.enableShadesSelection &&
                _activePicker == ColorPickerType.wheel)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.wheelSubheading,
              ),
            // Draw the shade colors for the selected main color.
            if (widget.enableShadesSelection)
              ShadeColors(
                spacing: widget.spacing,
                runSpacing: widget.runSpacing,
                columnSpacing: widget.shadesSpacing ?? widget.columnSpacing,
                activeSwatch: _activeSwatch!,
                selectedColor:
                    widget.enableOpacity ? _selectedColor : _tappedColor,
                onSelectColor: (Color color) {
                  _tonalOperated = false;
                  _onSelectColor(color);
                  if (_activePicker == ColorPickerType.wheel) {
                    setState(() {
                      _selectedShouldFocus = true;
                      _tonalShouldUpdate = true;
                    });
                  }
                },
                includeIndex850: widget.includeIndex850,
                width: widget.width,
                height: widget.height,
                borderRadius: widget.borderRadius,
                hasBorder: widget.hasBorder,
                borderColor: widget.borderColor,
                elevation: widget.elevation,
                selectedColorIcon: widget.selectedColorIcon,
                selectedRequestsFocus: _selectedShouldFocus,
              ),
            // Show the tonal sub-heading
            if (widget.tonalSubheading != null && widget.enableTonalPalette)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.tonalSubheading,
              ),
            // Draw the tonal palette.
            if (widget.enableTonalPalette) ...<Widget>[
              TonalPaletteColors(
                spacing: widget.spacing,
                runSpacing: widget.runSpacing,
                selectedColor: _tappedColor,
                onSelectColor: (Color color) {
                  _tonalOperated = true;
                  _onSelectColor(color);
                  setState(() {
                    _selectedShouldFocus = true;
                    _tonalShouldUpdate = false;
                  });
                },
                tonalShouldUpdate: _tonalShouldUpdate,
                width: widget.tonalColorSameSize
                    ? widget.width
                    : (widget.width + widget.spacing) * 10 / 15 -
                        widget.spacing,
                height: widget.tonalColorSameSize
                    ? widget.width
                    : (widget.width + widget.spacing) * 10 / 15 -
                        widget.spacing,
                borderRadius: widget.borderRadius,
                hasBorder: widget.hasBorder,
                borderColor: widget.borderColor,
                elevation: widget.elevation,
                selectedColorIcon: widget.selectedColorIcon,
                selectedRequestsFocus: _selectedShouldFocus,
                tonalPaletteFixedMinChroma: widget.tonalPaletteFixedMinChroma,
              ),
              SizedBox(height: widget.columnSpacing),
            ],
            // Show the sub-heading for the opacity control.
            if (widget.opacitySubheading != null && widget.enableOpacity)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.opacitySubheading,
              ),
            // Draw the opacity slider if enabled.
            if (widget.enableOpacity)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: SizedBox(
                  width: (widget.opacityTrackWidth ?? 0) < 150
                      ? double.infinity
                      : widget.opacityTrackWidth,
                  child: RepaintBoundary(
                    child: OpacitySlider(
                      color: _selectedColor.withAlpha(0xFF),
                      opacity: _opacity,
                      trackHeight: widget.opacityTrackHeight,
                      thumbRadius: widget.opacityThumbRadius,
                      focusNode: _opacityFocusNode,
                      onChangeStart: (double value) {
                        if (widget.onColorChangeStart != null) {
                          // Request focus when change starts.
                          _opacityFocusNode.requestFocus();
                          setState(() {
                            _fromInternal = true;
                            _opacity = value;
                          });
                          widget.onColorChangeStart!(
                              _selectedColor.withOpacity(_opacity));
                          _addToRecentColors(
                              _selectedColor.withOpacity(_opacity));
                        }
                      },
                      onChanged: (double value) {
                        setState(() {
                          _fromInternal = true;
                          _opacity = value;
                          _wheelShouldUpdate = false;
                          _editShouldUpdate = true;
                          _selectedShouldFocus = true;
                          _wheelShouldFocus = false;
                        });
                        widget.onColorChanged(
                            _selectedColor.withOpacity(_opacity));
                      },
                      onChangeEnd: (double value) {
                        if (widget.onColorChangeEnd != null) {
                          setState(() {
                            _fromInternal = true;
                            _opacity = value;
                          });
                          widget.onColorChangeEnd!(
                              _selectedColor.withOpacity(_opacity));
                          // _addToRecentColors(
                          //     _selectedColor.withOpacity(_opacity));
                        }
                      },
                    ),
                  ),
                ),
              ),
            // If we show material or generic name, we enclose them in a
            // Wrap, they will be on same row nicely if there is room
            // enough, but also wrap to two rows when so needed when both
            // are shown at the same and they don't fit on one row.
            if (widget.showMaterialName || widget.showColorName)
              Wrap(
                children: <Widget>[
                  // Show the Material color name, if enabled.
                  if (widget.showMaterialName)
                    Padding(
                      padding: EdgeInsets.only(bottom: widget.columnSpacing),
                      child: Text(
                        ColorTools.materialName(
                          _selectedColor.withAlpha(0xFF),
                          colorSwatchNameMap:
                              widget.customColorSwatchesAndNames,
                        ),
                        style: effectiveMaterialNameStyle,
                      ),
                    ),
                  // If we show both material and generic name, add some
                  // hard coded horizontal space between them.
                  if (widget.showMaterialName && widget.showColorName)
                    const SizedBox(width: 8),
                  // Show the generic color name, if enabled.
                  if (widget.showColorName)
                    Padding(
                      padding: EdgeInsets.only(bottom: widget.columnSpacing),
                      child: Text(
                        ColorTools.nameThatColor(
                            _selectedColor.withAlpha(0xFF)),
                        style: effectiveGenericNameStyle,
                      ),
                    ),
                ],
              ),
            // If we show color code or its int value, we enclose them in a
            // Wrap, they will be on same row nicely if there is room enough
            // but also wrap to two rows when so needed when both are
            // shown at the same and they don't fit on one row.
            if (widget.showColorCode || widget.showColorValue)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: Padding(
                  padding: EdgeInsetsDirectional.only(
                    // Padding to offset the edit icon button.
                    end: showEditIconButton ? 40.0 : 0,
                  ),
                  child: Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    runAlignment: WrapAlignment.center,
                    alignment: WrapAlignment.center,
                    children: <Widget>[
                      if (showEditIconButton)
                        Padding(
                          padding: const EdgeInsetsDirectional.only(end: 4),
                          child: IconButton(
                            constraints: const BoxConstraints(
                              minWidth: 36,
                              minHeight: 36,
                            ),
                            padding: const EdgeInsets.all(6),
                            splashRadius: 36,
                            icon: Icon(widget.editIcon, size: 24),
                            onPressed: () {
                              setState(() {
                                _requestEditFocus = true;
                              });
                            },
                          ),
                        ),
                      // Show the color code view and edit field, if enabled.
                      if (widget.showColorCode)
                        ColorCodeField(
                          color: widget.enableOpacity
                              ? _selectedColor.withOpacity(_opacity)
                              : _tappedColor,
                          readOnly: _activePicker != ColorPickerType.wheel ||
                              widget.colorCodeReadOnly,
                          textStyle: widget.colorCodeTextStyle,
                          requestFocus: _requestEditFocus,
                          focusedEditHasNoColor: widget.focusedEditHasNoColor,
                          prefixStyle: widget.colorCodePrefixStyle,
                          colorCodeHasColor: widget.colorCodeHasColor,
                          enableTooltips: widget.enableTooltips,
                          shouldUpdate: _editShouldUpdate,
                          onColorChanged: (Color color) {
                            widget.onColorChangeStart
                                ?.call(_selectedColor.withOpacity(_opacity));
                            setState(() {
                              _tappedColor = color;
                              _selectedColor = color;
                              // Color changed outside wheel picker, when the
                              // code was edited, the wheel should update.
                              _wheelShouldUpdate = true;
                              _editShouldUpdate = false;
                              _tonalOperated = false;
                              _updateActiveSwatch();
                            });
                            widget.onColorChanged(
                                _selectedColor.withOpacity(_opacity));
                            widget.onColorChangeEnd
                                ?.call(_selectedColor.withOpacity(_opacity));
                            _addToRecentColors(
                                _selectedColor.withOpacity(_opacity));
                          },
                          onEditFocused: (bool editInFocus) {
                            _requestEditFocus = false;
                            _editCodeFocused = editInFocus;
                            if (editInFocus) {
                              setState(() {
                                _selectedShouldFocus = false;
                                _wheelShouldFocus = false;
                              });
                            }
                          },
                          toolIcons: widget.actionButtons,
                          copyPasteBehavior: widget.copyPasteBehavior,
                        ),
                      // If we show both hex code and int value, add some
                      // hardcoded horizontal space between them.
                      if (widget.showColorCode && widget.showColorValue)
                        const SizedBox(width: 8),
                      if (widget.showColorValue)
                        SelectableText(
                          _selectedColor.value.toString(),
                          style: effectiveCodeStyle,
                        ),
                    ],
                  ),
                ),
              ),
            // Show the sub-heading for recent colors.
            if (widget.recentColorsSubheading != null &&
                widget.showRecentColors)
              Padding(
                padding: EdgeInsets.only(bottom: widget.columnSpacing),
                child: widget.recentColorsSubheading,
              ),
            // Show the color indicators for the recently used colors.
            if (widget.showRecentColors)
              RecentColors(
                spacing: widget.spacing,
                runSpacing: widget.runSpacing,
                recentColors: _recentColors,
                selectedColor: _tappedColor,
                onSelectColor: (Color color) {
                  _tonalOperated = false;
                  _onSelectColor(color, findPicker: true);
                },
                includeIndex850: widget.includeIndex850,
                width: widget.width,
                height: widget.height,
                borderRadius: widget.borderRadius,
                hasBorder: widget.hasBorder,
                borderColor: widget.borderColor,
                elevation: widget.elevation,
                selectedColorIcon: widget.selectedColorIcon,
                selectedRequestsFocus: _selectedShouldFocus,
              ),
            if (widget.showRecentColors) SizedBox(height: widget.columnSpacing),
          ],
        ),
      ),
    );
  }

  // A color was selected by tapping it, update state and notify via callbacks.
  void _onSelectColor(
    Color color, {
    // Normally when colors are selected, we do not need find the picker as they
    // are in the same picker. However, recently used colors sets this to true
    // as its colors can be in any picker, so it must be found.
    bool findPicker = false,
  }) {
    // Any callbacks that are called here, should set the _fromInternal flag.
    _fromInternal = true;

    // If findPicker is true we came from RecentColors, it is the only one that
    // uses this flag. If we use Recent colors, we should also extract the
    // opacity from the selected color, and set it as the current opacity.
    if (findPicker) {
      _opacity = color.opacity;
    }
    // Call start callback with current selectedColor before change.
    if (widget.enableOpacity) {
      widget.onColorChangeStart?.call(_selectedColor.withOpacity(_opacity));
      _addToRecentColors(_selectedColor.withOpacity(_opacity));
    } else {
      // This is to allow custom colors with opacity to callback with their
      // in-built opacity, when opacity is control is not enabled.
      widget.onColorChangeStart?.call(_tappedColor);
      _addToRecentColors(_tappedColor);
    }
    // update the state of the selectedColor to the new selected color.
    setState(() {
      // Set selected color to the one that was "clicked"
      _tappedColor = color;
      _selectedColor = _tappedColor.withAlpha(0xFF);
      if (_debug) debugPrint('_onSelectColor _tappedColor: $_tappedColor');
      if (_debug) debugPrint('_onSelectColor _selectedColor: $_selectedColor');
      // When the a color was clicked and selected, the right item is already
      // focused an other selected color indicators and wheel should not focus.
      _selectedShouldFocus = false;
      _wheelShouldFocus = false;
      // Color changed outside wheel and edit, a new shade or color was
      // selected outside the wheel and edit field, they should update!
      _wheelShouldUpdate = true;
      _editShouldUpdate = true;
      // Tonal palette should be updated.
      _tonalShouldUpdate = true;

      // Find best matching picker of the enabled ones for _selectedColor.
      if (findPicker) {
        // Make a swatch of the selected color in the wheel.
        _typeToSwatchMap[ColorPickerType.wheel] = <ColorSwatch<Object>>[
          ColorTools.createPrimarySwatch(_selectedColor)
        ];
        _findPicker();
      }
      // If we are not on the wheel, and a selected color is not a member
      // of the current selected swatch, then update the active swatch.
      // This check eliminates a rebuild when the selected color is a member
      // of the currently displayed color swatch.
      // Needed to update the active swatch when opacity is used on custom
      // colors when opacity is not enabled.
      if ((_activePicker != ColorPickerType.wheel ||
              !ColorTools.swatchContainsColor(
                  _activeSwatch!, _selectedColor)) &&
          !_tonalOperated) {
        // Update the active swatch to match the selected color.
        if (_debug) {
          debugPrint('**** _onSelectColor calls _updateActiveSwatch ****');
        }
        _updateActiveSwatch();
      }
    });
    // Call the change call back with the new color.
    if (widget.enableOpacity) {
      widget.onColorChanged(_selectedColor.withOpacity(_opacity));
      widget.onColorChangeEnd?.call(_selectedColor.withOpacity(_opacity));
    } else {
      // This is to allow custom colors with opacity to callback with their
      // in-built opacity, when opacity is control is not enabled.
      widget.onColorChanged(_tappedColor);
      widget.onColorChangeEnd?.call(_tappedColor);
    }
  }

  void _addToRecentColors(Color color) {
    // We are not showing recent colors, do nothing.
    if (!widget.showRecentColors) return;
    // If recent colors already contains the color, do nothing.
    if (_recentColors.contains(color)) return;
    // Clamp the max recent colors to the allowed range.
    final int clampedRecentColors =
        widget.maxRecentColors.clamp(_minRecentColors, _maxRecentColors);
    // Max recent colors reached, remove first one added.
    if (_recentColors.length >= clampedRecentColors) {
      _recentColors.removeRange(clampedRecentColors - 1, _recentColors.length);
    }
    setState(() {
      _recentColors = <Color>[color, ..._recentColors];
    });
    // Call callback for the handling recent colors, if there is one.
    widget.onRecentColorsChanged?.call(_recentColors);
  }

  // Get the current clipboard data. Try to parse it to a Color object.
  // If successful, set the current color to the resulting Color.
  Future<void> _getClipboard() async {
    final ClipboardData? data = await Clipboard.getData(Clipboard.kTextPlain);
    // Clipboard data was null, exit.
    if (data == null) return;

    // Try to parse the clipboard data for a valid color value
    final Color? clipColor = data.text
        .toColorShortMaybeNull(widget.copyPasteBehavior.parseShortHexCode);
    // If result is not null, we got a valid color.
    if (clipColor != null) {
      widget.onColorChangeStart?.call(_selectedColor);
      // Add the previous selected color to recent colors
      _addToRecentColors(_selectedColor);
      // This wait for 100ms feels a bit like a hack, but if not done, the
      // paste when using paste parser in the color code edit field DOES NOT
      // WORK correctly. This delay allows the used TextField to first process
      // its variant of the paste, that we will then override when the paste
      // parser is active for the edit field, the paste command will be caught
      // here again, and we get the complete value as evaluated by the parser
      // applied instead. The extra wait here has no visible impact on other
      // pasting when the edit field is not active.
      await Future<void>.delayed(const Duration(milliseconds: 100));
      setState(() {
        // We always remove any alpha from pasted colors.
        _selectedColor = clipColor.withAlpha(0xFF);
        // If opacity is enabled, we capture the opacity from the pasted color.
        _opacity = widget.enableOpacity ? clipColor.opacity : 1;
        _addToRecentColors(_selectedColor.withOpacity(_opacity));
        // Color changed outside wheel and edit field, a new shade or color was
        // selected outside the wheel and edit, they should update!
        _wheelShouldUpdate = true;
        _editShouldUpdate = true;
        // Make a swatch of the new via paste _selectedColor for the wheel.
        _typeToSwatchMap[ColorPickerType.wheel] = <ColorSwatch<Object>>[
          ColorTools.createPrimarySwatch(_selectedColor),
        ];
        // Move the picker to the pasted color value and update active swatch.
        _findPicker();
        _updateActiveSwatch();
      });
      // Callback with new color
      widget.onColorChanged(_selectedColor);
      widget.onColorChangeEnd?.call(_selectedColor);
    }
    // ELSE FOR: Clipboard could not parsed to a Color()
    else {
      // TODO(rydmike): Improve sound when it can be done with SDK features.
      // This is a nice idea, but it does not do much on most platforms.
      // Would just like to get a nice "error bleep" sound on all platforms
      // without any plugin by using SDK only, but not doable, bummer.
      // Keeping it around as an experimental feature.
      // If we do all, vibrate, click and alert, there just might some kind
      // of feedback on most platforms.
      // Maybe one day Material sound design will be supported out of the box,
      // then we can make this nicer.
      if (widget.copyPasteBehavior.feedbackParseError) {
        await HapticFeedback.vibrate();
        await SystemSound.play(SystemSoundType.click);
        await SystemSound.play(SystemSoundType.alert);
      }
      // Show a snack bar paste format error message.
      if (widget.copyPasteBehavior.snackBarParseError) {
        String? snackBarMessage = widget.copyPasteBehavior.snackBarMessage;
        if (snackBarMessage == null) {
          // The new experimental lint rules "use_build_context_synchronously"
          // warns us to check if context is still mounted if we have
          // async delays in an async function, since the state could have been
          // unmounted and disposed during the delay(s).
          if (!mounted) return;
          // Get the Material localizations.
          final MaterialLocalizations translate =
              MaterialLocalizations.of(context);
          snackBarMessage = '${translate.pasteButtonLabel}: '
              '${translate.invalidDateFormatLabel}';
        }
        // Wait 300ms, if we show it at once, it feels to fast.
        await Future<void>.delayed(const Duration(milliseconds: 300));
        // The new experimental lint rules "use_build_context_synchronously"
        // warns us to check if context is still mounted if we have
        // async delays in an async function, since the state could have been
        // unmounted and disposed during the delay(s).
        if (!mounted) return;
        // Show a snack bar with the paste error message.
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(snackBarMessage),
            duration: widget.copyPasteBehavior.snackBarDuration,
          ),
        );
      }
    }
  }

  // Set current selected color value as a String on the Clipboard in
  // currently configured format.
  Future<void> _setClipboard() async {
    String colorString = '00000000';
    switch (widget.copyPasteBehavior.copyFormat) {
      case ColorPickerCopyFormat.dartCode:
        colorString = '0x${_selectedColor.hexAlpha}';
      case ColorPickerCopyFormat.hexRRGGBB:
        colorString = _selectedColor.hex;
      case ColorPickerCopyFormat.hexAARRGGBB:
        colorString = _selectedColor.hexAlpha;
      case ColorPickerCopyFormat.numHexRRGGBB:
        colorString = '#${_selectedColor.hex}';
      case ColorPickerCopyFormat.numHexAARRGGBB:
        colorString = '#${_selectedColor.hexAlpha}';
    }
    final ClipboardData data = ClipboardData(text: colorString);
    await Clipboard.setData(data);
  }
}

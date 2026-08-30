import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

/// Enum that controls the RGB string format of the copied color value.
///
/// When you copy a [Color] value from the color picker, this enum is
/// used to configure the desired default format of the received RGB string.
///
/// If you have opacity enabled for the picker, the alpha value will not
/// be included in the copied RGB string if you use a format that does
/// not include the alpha value.
///
/// When you paste a color string value into the color picker, it can
/// automatically parse a string from any of the available RGB strings formats
/// to a Dart and Flutter [Color] object, regardless of what the copy format is.
/// Additionally the pasted color value can be in the 3-char short RGB hex
/// format, it will also be correctly parsed to its [Color] value, provided
/// that [ColorPickerCopyPasteBehavior.parseShortHexCode] is true.
enum ColorPickerCopyFormat {
  /// In Flutter/Dart Hex RGB format '0xAARRGGBB'.
  dartCode,

  /// Hex RGB format with no alpha 'RRGGBB'.
  hexRRGGBB,

  /// Hex RGB format with alpha 'AARRGGBB'.
  hexAARRGGBB,

  /// Web Hex RGB format with leading num # sign and no alpha '#RRGGBB'.
  numHexRRGGBB,

  /// Web Hex RGB format with leading num # sign and alpha '#AARRGGBB'.
  numHexAARRGGBB,
}

/// Used by FlexColorPicker to define how copy-paste operations behave.
///
/// * Copy and paste action buttons in the top toolbar.
/// * Long press and/or right click copy and paste context menu.
/// * Ctrl-C and Ctrl-V keyboard shortcuts, also when not in edit field.
///   Keyboard shortcuts automatically uses Command instead of Ctrl on macOS.
/// * A copy color action button in the code entry and display field.
///
/// You can also:
///
/// * Define default result RGB string format of a copy command.
/// * Define icons for copy and paste action buttons.
/// * Define icon theme's for the copy and paste icons.
/// * Define paste color string parsing error feedback type and message if used.
/// * Modify the tooltips for copy and paste buttons.
///
/// Paste operation supports all RGB string formats defined by
/// [ColorPickerCopyFormat], but copy format is only in selected
/// [copyFormat].
@immutable
class ColorPickerCopyPasteBehavior with Diagnosticable {
  /// Default constructor
  const ColorPickerCopyPasteBehavior(
      {this.ctrlC = true,
      this.ctrlV = true,
      this.autoFocus = true,
      this.copyButton = false,
      this.pasteButton = false,
      this.copyIcon = Icons.copy,
      this.pasteIcon = Icons.paste,
      this.copyTooltip,
      this.pasteTooltip,
      this.copyFormat = ColorPickerCopyFormat.dartCode,
      this.longPressMenu = false,
      this.secondaryMenu = false,
      this.secondaryOnDesktopLongOnDevice = false,
      this.secondaryOnDesktopLongOnDeviceAndWeb = false,
      this.editFieldCopyButton = true,
      this.menuIconThemeData,
      this.menuThemeData,
      this.menuWidth = 80,
      this.menuItemHeight = 30,
      this.snackBarParseError = false,
      this.snackBarMessage,
      this.snackBarDuration = const Duration(milliseconds: 1800),
      this.feedbackParseError = false,
      this.parseShortHexCode = false,
      this.editUsesParsedPaste = true});

  /// A keyboard CMD/CTRL-C press will copy the clipboard into the picker.
  ///
  /// When enabled, this keyboard copy color shortcut works when the
  /// ColorPicker and one of its focusable widgets have focus. Those include
  /// color indicator, color field, buttons, opacity slider and the picker
  /// selector as well as the color wheel.
  ///
  /// Defaults to true.
  final bool ctrlC;

  /// A keyboard CMD/CTRL-V press will paste the clipboard into the picker.
  ///
  /// When enabled, this keyboard copy color shortcut works when the
  /// ColorPicker and one of its focusable widgets have focus. Those include
  /// color indicator, color field, buttons, opacity slider and the picker
  /// selector as well as the color wheel.
  ///
  /// Defaults to true.
  final bool ctrlV;

  /// When true, the picker tries to grab the focus when the picker is created.
  ///
  /// By default the picker tries to set focus to its own widgets when it is
  /// created. It does this when either [ctrlC] or [ctrlV] are enabled in
  /// order for the keyboard listener to be able to react to copy-paste events
  /// even if no control on the widget has been focused yet.
  ///
  /// If you need another widget to retain focus. e.g. if the picker is used on
  /// surface/scope shared with other widgets and not in its own dialog, then
  /// setting [autoFocus] to false might help.
  ///
  /// If both [ctrlC] and [ctrlV] are false, the picker yields the focus the
  /// same way as setting [autoFocus] false, but then you have no keyboard
  /// shortcut copy-paste functions at all. With [autoFocus] false, you
  /// can still use keyboard copy-paste shortcuts and yield the focus from
  /// the picker. When you do this, the copy-paste keyboard shortcuts will not
  /// work until one of the picker's components have been focused by
  /// interacting with them.
  ///
  /// The picker still grabs focus when you click on its background, as one way
  /// to set focus to keyboard listener to enable copy-paste keyboard shortcuts
  /// or when you operate any of its controls, the control in question
  /// always gains focus.
  ///
  /// Default to true.
  final bool autoFocus;

  /// Show a copy action icon in the picker top tool bar.
  ///
  /// Defaults to false.
  final bool copyButton;

  /// Show a paste action icon in the picker top tool bar.
  ///
  /// Defaults to false.
  final bool pasteButton;

  /// Icon used for the copy action.
  ///
  /// The same COPY icon is used in the top tool bar, on the context menu and
  /// after the code field, when those features are enabled.
  ///
  /// Defaults to [Icons.copy].
  final IconData copyIcon;

  /// Icon used for the paste action icon in the title bar.
  ///
  /// The same PASTE icon is used in the top tool bar and on the context menu,
  /// when those features are enabled.
  ///
  /// Defaults to [Icons.paste].
  final IconData pasteIcon;

  /// Label used as tooltip for copy action.
  ///
  /// Provide your own or use the default material localization label.
  ///
  /// Defaults to MaterialLocalizations.of(context).copyButtonLabel.
  /// If CTRL-C copying is also enabled, the string ' (CTRL-C)' is added
  /// on Linux and Windows platforms and on macOS ' (CMD-C)' is added.
  final String? copyTooltip;

  /// Label used as tooltip for paste action.
  ///
  /// Provide your own or use the default material localization label.
  ///
  /// Defaults to: MaterialLocalizations.of(context).pasteButtonLabel.
  /// If CTRL-V pasting is also enabled, the string ' (CTRL-V)' is added
  /// on Linux and Windows platforms and on macOS ' (CMD-V)' is added.
  final String? pasteTooltip;

  /// Defines the format of the copied color code string.
  ///
  /// Defaults to [ColorPickerCopyFormat.dartCode].
  ///
  /// * [ColorPickerCopyFormat.dartCode] is Flutter Hex RGB format '0xAARRGGBB'.
  /// * [ColorPickerCopyFormat.hexRRGGBB] is Hex RGB format with no
  ///   alpha 'RRGGBB'.
  /// * [ColorPickerCopyFormat.hexAARRGGBB] is Hex RGB format with
  ///   alpha 'AARRGGBB'.
  /// * [ColorPickerCopyFormat.numHexRRGGBB] is Web Hex RGB format with a
  ///   leading num # sign and no alpha '#RRGGBB'.
  /// * [ColorPickerCopyFormat.numHexAARRGGBB] is Web Hex RGB format with a
  ///   * leading num # sign and alpha '#AARRGGBB'.
  final ColorPickerCopyFormat copyFormat;

  /// Use long press in the picker to open a color copy and paste menu.
  ///
  /// Defaults to false.
  final bool longPressMenu;

  /// Use secondary button click in the picker to open a color copy and paste
  /// menu.
  ///
  /// Defaults to false.
  final bool secondaryMenu;

  /// Use secondary button click on desktop and their web version and long
  /// press on iOs/Android devices in the picker, to open a color copy and
  /// paste context menu.
  ///
  /// Defaults to false.
  final bool secondaryOnDesktopLongOnDevice;

  /// Use secondary button click on desktop and long press on iOs/Android
  /// devices and all web builds in the picker, to open a color copy and
  /// paste context menu.
  ///
  /// Defaults to false.
  final bool secondaryOnDesktopLongOnDeviceAndWeb;

  /// Show a copy button suffix in the color code edit and display field.
  ///
  /// Defaults to true.
  final bool editFieldCopyButton;

  /// The theme for the menu icons.
  ///
  /// The menu is compact, so icons are small by design.
  ///
  /// Uses any none null property in passed in [IconThemeData]. If it is
  /// is null, or any property in it is null, then it uses the
  /// property values from surrounding `Theme.of(context).iconTheme` if they
  /// are defined. For any values that remain null value, the following
  /// fallback defaults are used:
  /// ```
  ///   color: remains null, so default [IconThemeData] color behavior is kept.
  ///   size: 16
  ///   opacity: 0.90
  /// ```
  final IconThemeData? menuIconThemeData;

  /// The theme of the popup menu.
  ///
  /// Uses any none null property in provided [PopupMenuThemeData], if it is
  /// null or any property in it is null, then it uses property values from
  /// `Theme.of(context).popupMenuTheme` if they are not null, for any null
  /// value the following fallback defaults are used:
  /// ```
  ///   color: theme.cardColor.withOpacity(0.9)
  ///   shape: RoundedRectangleBorder(
  ///            borderRadius: BorderRadius.circular(8),
  ///            side: BorderSide(
  ///            color: theme.dividerColor))
  ///   elevation: 3
  ///   textStyle: theme.textTheme.bodyMedium!
  ///   enableFeedback: true
  /// ```
  final PopupMenuThemeData? menuThemeData;

  /// The width of the menu.
  ///
  /// Defaults to 80 dp.
  final double menuWidth;

  /// The height of each menu item.
  ///
  /// Defaults to 30 dp.
  final double menuItemHeight;

  /// Show a snack bar paste parse error message when pasting something that
  /// could not be parsed to a color value.
  ///
  /// A paste parse error occurs when something is pasted into the color picker
  /// that cannot parsed to a color value.
  ///
  /// Defaults to false.
  final bool snackBarParseError;

  /// The message shown in the paste parse error snack bar.
  ///
  /// The String is shown in the snack bar when there
  /// is a paste parse error and [snackBarParseError] is true.
  ///
  /// If null, it defaults to the combination of the two Material localization
  /// labels `pasteButtonLabel`: `invalidDateFormatLabel` in a [Text] widget.
  /// In English it says "Paste: Invalid format.".
  ///
  /// The snackBar uses the closest theme with SnackBarThemeData for its
  /// theming.
  final String? snackBarMessage;

  /// The duration the paste parse error snack bar message is shown.
  ///
  /// Defaults to const Duration(milliseconds: 1800).
  final Duration snackBarDuration;

  /// If true then vibrate, play audible click or an alert sound, when a
  /// paste parse error occurs.
  ///
  /// A paste parse error occurs when something is pasted into the color picker
  /// that cannot parsed to a color value.
  ///
  /// This feature is experimental, its support is limited on most platforms
  /// in Flutter. If Flutter one day supports the Material Sound Guide, this
  /// feature can be improved with better sound effects. Currently it cannot be
  /// improved without importing none SDK plugins/packages to make sounds.
  /// This package strives to work without any plugins or packages, so it will
  /// not add any additional none Flutter SDK imports.
  ///
  /// Defaults to false.
  final bool feedbackParseError;

  /// When true the hex color code paste action and field entry parser,
  /// interpret short three character web hex color codes like in CSS.
  ///
  /// Web allows for short HEX RGB color codes like 123, ABC, F0C and 5D1
  /// being used as RGB hex color values. These will be interpreted as
  /// 112233, AABBCC, FF00CC and 55DD11 when [parseShortHexCode] is true.
  /// This parsing applies to both pasted color values and entries in the color
  /// code field when [parseShortHexCode] is true.
  ///
  /// Defaults to false.
  final bool parseShortHexCode;

  /// If true, the color code entry field uses parsed paste action for
  /// keyboard shortcuts CTRL-V and CMD-V,
  ///
  /// A standard text field, will just paste whatever text is in the copy/paste
  /// buffer into the field. This is the `false` default behavior here too,
  /// with the exception that the field only accepts valid hex value input
  /// chars (0-9, A-F), so it always filters and pastes only the acceptable
  /// input chars from the paste buffer.
  ///
  /// If this property is `true`, the edit field will use the same color
  /// paste value parsing used by the other paste actions used when the input
  /// field is not in focus.
  /// This results in a paste action in the field that always fully replaces
  /// the content with the parsed color value of the pasted data, not just
  /// pasting in the string in the paste buffer over selected text.
  ///
  /// Currently this setting only impacts CTRL-V and CMD-V keyboard shortcut
  /// pasting on desktops. The paste on Android and iOS are not intercepted
  /// when this setting is true.
  ///
  /// Defaults to true.
  ///
  /// **ERRATA INFO:**
  ///
  /// This setting was in version 3.6.0 changed to be true by default. The
  /// feature had been broken and missing since version 3.4.0 and the default
  /// behavior from 3.4.0 and forward was the same as setting this to true,
  /// there was no false feature. Generally it is recommended to keep this
  /// setting true for a more consistent and better paste experience. Thus
  /// decided to make it true by default, which matches the behavior of the
  /// past versions 3.4.0 and 3.5.0. The true behavior is also what pretty
  /// much all uses cases should use. Was about to deprecate this setting
  /// and remove it in version 4.0.0, but decided to keep it undeprecated for
  /// now since it works as stated again.
  ///
  /// The false setting is equivalent to past versions (1.x) default behavior
  /// when pasting strings into the code entry field.
  final bool editUsesParsedPaste;

  /// Copy the object with one or more provided properties changed.
  ColorPickerCopyPasteBehavior copyWith({
    bool? ctrlC,
    bool? ctrlV,
    bool? autoFocus,
    bool? copyButton,
    bool? pasteButton,
    IconData? copyIcon,
    IconData? pasteIcon,
    String? copyTooltip,
    String? pasteTooltip,
    ColorPickerCopyFormat? copyFormat,
    bool? longPressMenu,
    bool? secondaryMenu,
    bool? secondaryOnDesktopLongOnDevice,
    bool? secondaryOnDesktopLongOnDeviceAndWeb,
    bool? editFieldCopyButton,
    IconThemeData? menuIconThemeData,
    PopupMenuThemeData? menuThemeData,
    double? menuWidth,
    double? menuItemHeight,
    bool? snackBarParseError,
    String? snackBarMessage,
    Duration? snackBarDuration,
    bool? feedbackParseError,
    bool? parseShortHexCode,
    bool? editUsesParsedPaste,
  }) {
    return ColorPickerCopyPasteBehavior(
      ctrlC: ctrlC ?? this.ctrlC,
      ctrlV: ctrlV ?? this.ctrlV,
      autoFocus: autoFocus ?? this.autoFocus,
      copyButton: copyButton ?? this.copyButton,
      pasteButton: pasteButton ?? this.pasteButton,
      copyIcon: copyIcon ?? this.copyIcon,
      pasteIcon: pasteIcon ?? this.pasteIcon,
      copyTooltip: copyTooltip ?? this.copyTooltip,
      pasteTooltip: pasteTooltip ?? this.pasteTooltip,
      copyFormat: copyFormat ?? this.copyFormat,
      longPressMenu: longPressMenu ?? this.longPressMenu,
      secondaryMenu: secondaryMenu ?? this.secondaryMenu,
      secondaryOnDesktopLongOnDevice:
          secondaryOnDesktopLongOnDevice ?? this.secondaryOnDesktopLongOnDevice,
      secondaryOnDesktopLongOnDeviceAndWeb:
          secondaryOnDesktopLongOnDeviceAndWeb ??
              this.secondaryOnDesktopLongOnDeviceAndWeb,
      editFieldCopyButton: editFieldCopyButton ?? this.editFieldCopyButton,
      menuIconThemeData: menuIconThemeData ?? this.menuIconThemeData,
      menuThemeData: menuThemeData ?? this.menuThemeData,
      menuWidth: menuWidth ?? this.menuWidth,
      menuItemHeight: menuItemHeight ?? this.menuItemHeight,
      snackBarParseError: snackBarParseError ?? this.snackBarParseError,
      snackBarMessage: snackBarMessage ?? this.snackBarMessage,
      snackBarDuration: snackBarDuration ?? this.snackBarDuration,
      feedbackParseError: feedbackParseError ?? this.feedbackParseError,
      parseShortHexCode: parseShortHexCode ?? this.parseShortHexCode,
      editUsesParsedPaste: editUsesParsedPaste ?? this.editUsesParsedPaste,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;
    return other is ColorPickerCopyPasteBehavior &&
        ctrlC == other.ctrlC &&
        ctrlV == other.ctrlV &&
        autoFocus == other.autoFocus &&
        copyButton == other.copyButton &&
        pasteButton == other.pasteButton &&
        copyIcon == other.copyIcon &&
        pasteIcon == other.pasteIcon &&
        copyTooltip == other.copyTooltip &&
        pasteTooltip == other.pasteTooltip &&
        copyFormat == other.copyFormat &&
        longPressMenu == other.longPressMenu &&
        secondaryMenu == other.secondaryMenu &&
        secondaryOnDesktopLongOnDevice ==
            other.secondaryOnDesktopLongOnDevice &&
        secondaryOnDesktopLongOnDeviceAndWeb ==
            other.secondaryOnDesktopLongOnDeviceAndWeb &&
        editFieldCopyButton == other.editFieldCopyButton &&
        menuIconThemeData == other.menuIconThemeData &&
        menuThemeData == other.menuThemeData &&
        menuWidth == other.menuWidth &&
        menuItemHeight == other.menuItemHeight &&
        snackBarParseError == other.snackBarParseError &&
        snackBarMessage == other.snackBarMessage &&
        snackBarDuration == other.snackBarDuration &&
        feedbackParseError == other.feedbackParseError &&
        parseShortHexCode == other.parseShortHexCode &&
        editUsesParsedPaste == other.editUsesParsedPaste;
  }

  @override
  int get hashCode => Object.hashAll(<Object?>[
        ctrlC,
        ctrlV,
        autoFocus,
        copyButton,
        pasteButton,
        copyIcon,
        pasteIcon,
        copyTooltip,
        pasteTooltip,
        copyFormat,
        longPressMenu,
        secondaryMenu,
        secondaryOnDesktopLongOnDevice,
        secondaryOnDesktopLongOnDeviceAndWeb,
        editFieldCopyButton,
        menuIconThemeData,
        menuThemeData,
        menuWidth,
        menuItemHeight,
        snackBarParseError,
        snackBarMessage,
        snackBarDuration,
        feedbackParseError,
        parseShortHexCode,
        editUsesParsedPaste,
      ]);

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('ctrlC', ctrlC));
    properties.add(DiagnosticsProperty<bool>('ctrlV', ctrlV));
    properties.add(DiagnosticsProperty<bool>('autoFocus', autoFocus));
    properties.add(DiagnosticsProperty<bool>('copyButton', copyButton));
    properties.add(DiagnosticsProperty<bool>('pasteButton', pasteButton));
    properties.add(DiagnosticsProperty<IconData>('copyIcon', copyIcon));
    properties.add(DiagnosticsProperty<IconData>('pasteIcon', pasteIcon));
    properties.add(StringProperty('copyTooltip', copyTooltip));
    properties.add(StringProperty('pasteTooltip', pasteTooltip));
    properties
        .add(EnumProperty<ColorPickerCopyFormat>('copyFormat', copyFormat));
    properties.add(DiagnosticsProperty<bool>('longPressMenu', longPressMenu));
    properties.add(DiagnosticsProperty<bool>('secondaryMenu', secondaryMenu));
    properties.add(DiagnosticsProperty<bool>(
        'secondaryOnDesktopLongOnDevice', secondaryOnDesktopLongOnDevice));
    properties.add(DiagnosticsProperty<bool>(
        'secondaryOnDesktopLongOnDeviceAndWeb',
        secondaryOnDesktopLongOnDeviceAndWeb));
    properties.add(
        DiagnosticsProperty<bool>('editFieldCopyButton', editFieldCopyButton));
    properties.add(DiagnosticsProperty<IconThemeData?>(
        'menuIconThemeData', menuIconThemeData));
    properties.add(DiagnosticsProperty<PopupMenuThemeData?>(
        'menuThemeData', menuThemeData));
    properties.add(DoubleProperty('menuWidth', menuWidth));
    properties.add(DoubleProperty('menuItemHeight', menuItemHeight));
    properties.add(
        DiagnosticsProperty<bool>('snackBarParseError', snackBarParseError));
    properties.add(StringProperty('snackBarMessage', snackBarMessage));
    properties.add(
        DiagnosticsProperty<Duration>('snackBarDuration', snackBarDuration));
    properties.add(
        DiagnosticsProperty<bool>('feedbackParseError', feedbackParseError));
    properties
        .add(DiagnosticsProperty<bool>('parseShortHexCode', parseShortHexCode));
    properties.add(
        DiagnosticsProperty<bool>('editUsesParsedPaste', editUsesParsedPaste));
  }
}

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

/// Type of button used for OK or Cancel action button on a FlexColorPicker
/// dialog.
enum ColorPickerActionButtonType {
  /// Use a [TextButton] button.
  text,

  /// Use [OutlinedButton] button.
  outlined,

  /// Use [ElevatedButton] button.
  elevated,

  /// Use [FilledButton] button.
  filled,

  /// Use [FilledButton.tonal] button.
  filledTonal,
}

/// Used to define the order of OK and Cancel buttons on the FlexColorPicker
/// dialog.
enum ColorPickerActionButtonOrder {
  /// OK is the right button.
  okIsRight,

  /// OK is the left button.
  okIsLeft,

  /// OK is on the place where it belongs depending on used platform
  ///
  /// Right:
  ///
  /// Cancel - OK : macOS, iOS, Android, Linux, Fuchsia
  ///
  /// Left:
  ///
  /// OK - Cancel : Windows
  adaptive,
}

/// Defines the FlexColorPicker OK and Cancel actions buttons and their style.
///
/// You can define if action buttons are on a top toolbar or only
/// in the dialog bottom. The toolbar buttons are plain icon only
/// buttons. For the Dialog buttons you can choose between
/// [TextButton], [OutlinedButton] and [ElevatedButton].
///
/// The used icons can be changed form default ones, as can used tooltips.
///
/// Used by FlexColorPicker to define action buttons and their style.
@immutable
class ColorPickerActionButtons with Diagnosticable {
  /// Default const constructor.
  const ColorPickerActionButtons({
    this.okButton = false,
    this.closeButton = false,
    this.okIcon = Icons.check,
    this.closeIcon = Icons.close,
    this.closeIsLast = true,
    this.okTooltip,
    this.closeTooltip,
    this.closeTooltipIsClose = true,
    this.toolIconsThemeData,
    this.visualDensity,
    this.padding = EdgeInsets.zero,
    this.alignment = Alignment.center,
    this.splashRadius = 24,
    this.constraints = const BoxConstraints(minHeight: 42, minWidth: 42),
    this.dialogActionButtons = true,
    this.dialogActionOnlyOkButton = false,
    this.dialogActionOrder = ColorPickerActionButtonOrder.okIsRight,
    this.dialogActionIcons = false,
    this.dialogCancelButtonLabel,
    this.dialogCancelButtonType = ColorPickerActionButtonType.text,
    this.dialogOkButtonLabel,
    this.dialogOkButtonType = ColorPickerActionButtonType.text,
    this.useRootNavigator = true,
  });

  /// Dialog has an OK icon button on top toolbar to select active color
  /// and close the color picker dialog.
  ///
  /// This will pop the current top route on navigation stack and return true.
  /// Only enable this toolbar button when you are using the picker in a dialog.
  ///
  /// Defaults to false.
  final bool okButton;

  /// Dialog has a Close icon button on top toolbar to cancel and close the
  /// color picker dialog.
  ///
  /// This will pop the current top route on navigation stack and return false.
  /// Only enable this toolbar button when you are using the picker in a dialog.
  ///
  /// Defaults to false.
  final bool closeButton;

  /// Icon used for the OK action icon in the color picker dialog.
  ///
  /// Used both on the toolbar icon button and as a prefix icon on the bottom
  /// action button, when icons have been enabled for the bottom action buttons.
  ///
  /// Defaults to [Icons.check].
  final IconData okIcon;

  /// Icon used for the close action icon in the color picker dialog.
  ///
  /// Used both on the toolbar icon button and as a prefix icon on the bottom
  /// action button, when prefix icons have been enabled for the bottom
  /// action buttons.
  ///
  /// Defaults to [Icons.close].
  final IconData closeIcon;

  /// Close icon is last in the color picker title toolbar.
  ///
  /// Set to false to swap the order of the OK and Close toolbar icons.
  ///
  /// Defaults to true, which results in the close 'x' icon being in upper
  /// end corner of the picker dialog.
  ///
  /// Based on Material guide, the bottom action buttons in a dialog are always
  /// in the Cancel-OK order. If the top toolbar buttons are used at the
  /// same time, this value can be set to false to show the top toolbar buttons
  /// in the same order as the Material bottom dialog action buttons.
  ///
  /// The recommendation is to not use the top and bottom action buttons at the
  /// same time, but rather select one of the two options. The API does
  /// however allow using both or even a mix and match. It is possible to show
  /// **Cancel** and **OK** actions at the bottom of the dialog, and also add
  /// just an 'x' icon in the upper end corner of the dialog that also
  /// cancel-closes the dialog as expected.
  final bool closeIsLast;

  /// Label used as tooltip for OK toolbar button.
  ///
  /// Provide your own or use the default material localization label.
  ///
  /// Defaults to MaterialLocalizations.of(context).okButtonLabel.
  final String? okTooltip;

  /// Label used as tooltip for close toolbar button.
  ///
  /// Provide your own or use the default material localization label.
  ///
  /// Defaults to MaterialLocalizations.of(context).cancelButtonLabel if
  /// closeTooltipIsClose is true. If false it defaults to
  /// MaterialLocalizations.of(context).cancelButtonLabel.
  final String? closeTooltip;

  /// Close toolbar icon button uses "close" material localization as
  /// default label.
  ///
  /// If set to false, it uses "cancel" localization. You can still also
  /// provide your own custom tooltip. This toggle is just convenient
  /// if you want to call the top toolbar buttons that closes/cancels the
  /// dialog "Close" (default) or "Cancel" like on a typical dialog bottom.
  /// The "Close" tooltip, is more in line with the 'x' icon,
  /// that here also closes the dialog with the dismiss result.
  ///
  /// Defaults to true.
  final bool closeTooltipIsClose;

  /// The theme for the toolbar icons.
  ///
  /// The toolbar is compact, so icons are small by design.
  ///
  /// Effective style will uses any none null property in the passed in
  /// [IconThemeData]. If the passed in theme data is null, or any property in
  /// it, is null, then the following fallback defaults are used:
  ///
  ///   color: remains null, so default [IconThemeData] color behavior is kept.
  ///   size: 22
  ///   opacity: 0.90
  ///
  /// NOTE: This theme on purpose does not merge any ambient theme. To keep the
  /// compact fallback values as its default style. To change them you have
  /// to pass in the desired [IconThemeData].
  final IconThemeData? toolIconsThemeData;

  /// Defines how compact the toolbar icon button layout will be.
  ///
  /// See also:
  ///
  ///  * [ThemeData.visualDensity], which specifies the [visualDensity] for all
  ///    widgets within a [Theme].
  final VisualDensity? visualDensity;

  /// The padding around the toolbar icon buttons. The entire padded icon will
  /// react to input gestures.
  ///
  /// Defaults to const EdgeInsets.all(0),
  final EdgeInsetsGeometry padding;

  /// Defines how the icon is positioned within the IconButton.
  ///
  /// Defaults to [Alignment.center].
  final AlignmentGeometry alignment;

  /// The splash radius on the toolbar icon buttons.
  ///
  /// Defaults to 24.
  final double splashRadius;

  /// Optional size constraints for the icon button.
  ///
  /// Defaults to: const BoxConstraints(minHeight: 34, minWidth: 34),
  final BoxConstraints constraints;

  /// If set to false, the bottom dialog action buttons att the bottom
  /// are removed.
  ///
  /// If you remove the bottom dialog action buttons, make sure to enabled the
  /// ones in the dialog toolbar.
  ///
  /// Defaults to true.
  final bool dialogActionButtons;

  /// If set to true, the dialog will only have an OK button and no Cancel when
  /// [dialogActionButtons] is true.
  final bool dialogActionOnlyOkButton;

  /// Defines the order of the OK and Cancel actions buttons at the bottom
  /// of the dialog.
  ///
  /// Options are:
  ///
  /// * okIsRight: OK is the right button.
  /// * okIsLeft: OK is the left button.
  /// * adaptive: OK is on the place where it belongs on used platform
  ///   Right: Cancel - OK : macOS, iOS, Android, Linux, Fuchsia
  ///   Left : OK - Cancel : Windows
  ///
  /// Defaults to "okIsRight". Prefer using "adaptive", but defaults to
  /// "okIsRight" in order to not break past behavior.
  final ColorPickerActionButtonOrder dialogActionOrder;

  /// If set to true, the dialog bottom action buttons will be prefixed with
  /// an icon.
  ///
  /// The cancel buttons will get the [closeIcon] and the ok button will get
  /// prefixed with the [okIcon].
  ///
  /// Defaults to false.
  final bool dialogActionIcons;

  /// Color picker dialog cancel button label.
  ///
  /// Label shown on the button for cancelling the color picking and closing
  /// the dialog and returning false.
  ///
  /// If null, defaults to  MaterialLocalizations.of(context).cancelButtonLabel.
  final String? dialogCancelButtonLabel;

  /// Type of button used in the dialog for the Cancel button.
  ///
  /// The button will be themed based on closest ambient theme.
  ///
  /// Defaults to [ColorPickerActionButtonType.text] resulting in [TextButton].
  final ColorPickerActionButtonType dialogCancelButtonType;

  /// Color picker dialog OK button label.
  ///
  /// Label shown on bottom action button for selecting the current color in
  /// the color picker dialog and closing the dialog and returning true.
  ///
  /// If null, defaults to MaterialLocalizations.of(context).okButtonLabel.
  final String? dialogOkButtonLabel;

  /// Type of button used in the dialog for the OK button.
  ///
  /// The button will be themed based on closest ambient theme.
  ///
  /// Defaults to [ColorPickerActionButtonType.text] resulting in [TextButton].
  final ColorPickerActionButtonType dialogOkButtonType;

  /// The `useRootNavigator` argument is used to determine whether to push the
  /// ColorPicker dialog to the [Navigator] furthest from or nearest to the
  /// given `context`.
  ///
  /// By default, `useRootNavigator` is `true` and the dialog route created
  /// by build of ColorPicker dialogs are on the root.
  ///
  /// This setting was moved here in version 2.1.0 in order to make the
  /// property accessible by Navigator pop functions both in the ColorPicker
  /// widget itself, as well as built-in dialogs that uses the ColorPicker.
  final bool useRootNavigator;

  /// Copy the object with one or more provided properties changed.
  ColorPickerActionButtons copyWith({
    bool? okButton,
    bool? closeButton,
    IconData? okIcon,
    IconData? closeIcon,
    bool? closeIsLast,
    String? okTooltip,
    String? closeTooltip,
    bool? closeTooltipIsClose,
    IconThemeData? toolIconsThemeData,
    VisualDensity? visualDensity,
    EdgeInsetsGeometry? padding,
    AlignmentGeometry? alignment,
    double? splashRadius,
    BoxConstraints? constraints,
    bool? dialogActionButtons,
    bool? dialogActionOnlyOkButton,
    ColorPickerActionButtonOrder? dialogActionOrder,
    bool? dialogActionIcons,
    String? dialogCancelButtonLabel,
    ColorPickerActionButtonType? dialogCancelButtonType,
    String? dialogOkButtonLabel,
    ColorPickerActionButtonType? dialogOkButtonType,
    bool? useRootNavigator,
  }) {
    return ColorPickerActionButtons(
      okButton: okButton ?? this.okButton,
      closeButton: closeButton ?? this.closeButton,
      okIcon: okIcon ?? this.okIcon,
      closeIcon: closeIcon ?? this.closeIcon,
      closeIsLast: closeIsLast ?? this.closeIsLast,
      okTooltip: okTooltip ?? this.okTooltip,
      closeTooltip: closeTooltip ?? this.closeTooltip,
      closeTooltipIsClose: closeTooltipIsClose ?? this.closeTooltipIsClose,
      toolIconsThemeData: toolIconsThemeData ?? this.toolIconsThemeData,
      visualDensity: visualDensity ?? this.visualDensity,
      padding: padding ?? this.padding,
      alignment: alignment ?? this.alignment,
      splashRadius: splashRadius ?? this.splashRadius,
      constraints: constraints ?? this.constraints,
      dialogActionButtons: dialogActionButtons ?? this.dialogActionButtons,
      dialogActionOnlyOkButton:
          dialogActionOnlyOkButton ?? this.dialogActionOnlyOkButton,
      dialogActionOrder: dialogActionOrder ?? this.dialogActionOrder,
      dialogActionIcons: dialogActionIcons ?? this.dialogActionIcons,
      dialogCancelButtonLabel:
          dialogCancelButtonLabel ?? this.dialogCancelButtonLabel,
      dialogCancelButtonType:
          dialogCancelButtonType ?? this.dialogCancelButtonType,
      dialogOkButtonLabel: dialogOkButtonLabel ?? this.dialogOkButtonLabel,
      dialogOkButtonType: dialogOkButtonType ?? this.dialogOkButtonType,
      useRootNavigator: useRootNavigator ?? this.useRootNavigator,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other.runtimeType != runtimeType) return false;
    return other is ColorPickerActionButtons &&
        runtimeType == other.runtimeType &&
        okButton == other.okButton &&
        closeButton == other.closeButton &&
        okIcon == other.okIcon &&
        closeIcon == other.closeIcon &&
        closeIsLast == other.closeIsLast &&
        okTooltip == other.okTooltip &&
        closeTooltip == other.closeTooltip &&
        closeTooltipIsClose == other.closeTooltipIsClose &&
        toolIconsThemeData == other.toolIconsThemeData &&
        visualDensity == other.visualDensity &&
        padding == other.padding &&
        alignment == other.alignment &&
        splashRadius == other.splashRadius &&
        constraints == other.constraints &&
        dialogActionButtons == other.dialogActionButtons &&
        dialogActionOnlyOkButton == other.dialogActionOnlyOkButton &&
        dialogActionOrder == other.dialogActionOrder &&
        dialogActionIcons == other.dialogActionIcons &&
        dialogCancelButtonLabel == other.dialogCancelButtonLabel &&
        dialogCancelButtonType == other.dialogCancelButtonType &&
        dialogOkButtonLabel == other.dialogOkButtonLabel &&
        dialogOkButtonType == other.dialogOkButtonType &&
        useRootNavigator == other.useRootNavigator;
  }

  @override
  int get hashCode => Object.hashAll(<Object?>[
        okButton,
        closeButton,
        okIcon,
        closeIcon,
        closeIsLast,
        okTooltip,
        closeTooltip,
        closeTooltipIsClose,
        toolIconsThemeData,
        visualDensity,
        padding,
        alignment,
        splashRadius,
        constraints,
        dialogActionButtons,
        dialogActionOnlyOkButton,
        dialogActionOrder,
        dialogActionIcons,
        dialogCancelButtonLabel,
        dialogCancelButtonType,
        dialogOkButtonLabel,
        dialogOkButtonType,
        useRootNavigator,
      ]);

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(DiagnosticsProperty<bool>('okButton', okButton));
    properties.add(DiagnosticsProperty<bool>('closeButton', closeButton));
    properties.add(DiagnosticsProperty<IconData>('okIcon', okIcon));
    properties.add(DiagnosticsProperty<IconData>('closeIcon', closeIcon));
    properties.add(DiagnosticsProperty<bool>('closeIsLast', closeIsLast));
    properties.add(StringProperty('okTooltip', okTooltip));
    properties.add(StringProperty('closeTooltip', closeTooltip));
    properties.add(
        DiagnosticsProperty<bool>('closeTooltipIsClose', closeTooltipIsClose));
    properties.add(DiagnosticsProperty<IconThemeData?>(
        'toolIconsThemeData', toolIconsThemeData));
    properties.add(
        DiagnosticsProperty<VisualDensity?>('visualDensity', visualDensity));
    properties
        .add(DiagnosticsProperty<AlignmentGeometry>('alignment', alignment));
    properties.add(DoubleProperty('splashRadius', splashRadius));
    properties
        .add(DiagnosticsProperty<BoxConstraints?>('constraints', constraints));
    properties.add(
        DiagnosticsProperty<bool>('dialogActionButtons', dialogActionButtons));
    properties.add(DiagnosticsProperty<bool>(
        'dialogActionOnlyOkButton', dialogActionOnlyOkButton));
    properties.add(EnumProperty<ColorPickerActionButtonOrder>(
        'dialogActionOrder', dialogActionOrder));
    properties
        .add(DiagnosticsProperty<bool>('dialogActionIcons', dialogActionIcons));
    properties.add(
        StringProperty('dialogCancelButtonLabel', dialogCancelButtonLabel));
    properties.add(EnumProperty<ColorPickerActionButtonType>(
        'dialogCancelButtonType', dialogCancelButtonType));
    properties.add(StringProperty('dialogOkButtonLabel', dialogOkButtonLabel));
    properties.add(EnumProperty<ColorPickerActionButtonType>(
        'dialogOkButtonType', dialogOkButtonType));
    properties
        .add(DiagnosticsProperty<bool>('useRootNavigator', useRootNavigator));
  }
}

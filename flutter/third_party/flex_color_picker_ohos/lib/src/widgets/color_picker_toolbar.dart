// ignore_for_file: use_super_parameters

import 'package:flutter/material.dart';

import '../functions/picker_functions.dart';
import '../models/color_picker_action_buttons.dart';
import '../models/color_picker_copy_paste_behavior.dart';

/// A top toolbar with title and action buttons for the color picker.
///
/// Not library exposed, private to the library.
class ColorPickerToolbar extends StatelessWidget {
  /// Default const constructor.
  const ColorPickerToolbar({
    Key? key,
    this.title,
    this.onCopy,
    this.onPaste,
    this.onOk,
    this.onClose,
    this.toolIcons = const ColorPickerActionButtons(),
    this.copyPasteBehavior = const ColorPickerCopyPasteBehavior(),
    this.enableTooltips = true,
  }) : super(key: key);

  /// A title widget, usually a Text widget.
  final Widget? title;

  /// Optional close button, if null there is no close button.
  final VoidCallback? onCopy;

  /// Optional close button, if null there is no close button.
  final VoidCallback? onPaste;

  /// Optional Ok button, if null there is no close button.
  final VoidCallback? onOk;

  /// Optional close button, if null there is no close button.
  final VoidCallback? onClose;

  /// Defines icons for the color picker title bar and its actions.
  ///
  /// Defaults to ColorPickerToolIcons().
  final ColorPickerActionButtons toolIcons;

  /// Defines the color picker's copy and paste behavior.
  ///
  /// Defaults to ColorPickerPasteBehavior().
  final ColorPickerCopyPasteBehavior copyPasteBehavior;

  /// Controls if tooltips are shown or not
  ///
  /// Defaults to true.
  final bool enableTooltips;

  @override
  Widget build(BuildContext context) {
    String? copyTooltip;
    String? pasteTooltip;
    String? okTooltip;
    String? closeTooltip;

    if (enableTooltips) {
      // Get current platform.
      final TargetPlatform platform = Theme.of(context).platform;
      // Get the Material localizations.
      final MaterialLocalizations translate = MaterialLocalizations.of(context);
      // If shortcut keys enabled, make a shortcut platform aware info tooltip.
      String copyKeyTooltip = '';
      if (copyPasteBehavior.ctrlC) {
        copyKeyTooltip = platformControlKey(platform, 'C');
      }
      String pasteKeyTooltip = '';
      if (copyPasteBehavior.ctrlV) {
        pasteKeyTooltip = platformControlKey(platform, 'V');
      }
      // Make the Copy, Paste, OK and close tooltips.
      copyTooltip =
          (copyPasteBehavior.copyTooltip ?? translate.copyButtonLabel) +
              copyKeyTooltip;
      pasteTooltip =
          (copyPasteBehavior.pasteTooltip ?? translate.pasteButtonLabel) +
              pasteKeyTooltip;
      okTooltip = toolIcons.okTooltip ?? translate.okButtonLabel;
      closeTooltip = toolIcons.closeTooltip ??
          (toolIcons.closeTooltipIsClose
              ? translate.closeButtonTooltip
              : translate.cancelButtonLabel);
    }
    // Get current theme and passed in icon theme.
    final ThemeData theme = Theme.of(context);
    final IconThemeData? iconTheme = toolIcons.toolIconsThemeData;
    final double effectiveIconSize = iconTheme?.size ?? 22;
    // This is a merge of provided iconThemeData, with
    // fallback to default values, color has no default, remains as null.
    final IconThemeData effectiveIconTheme = theme.iconTheme.copyWith(
      color: iconTheme?.color,
      size: effectiveIconSize,
      opacity: iconTheme?.opacity ?? 0.90,
    );
    return Theme(
      data: theme.copyWith(iconTheme: effectiveIconTheme),
      child: Row(
        children: <Widget>[
          if (title != null) title!,
          if (title != null ||
              onCopy != null ||
              onPaste != null ||
              onOk != null ||
              onClose != null)
            const Spacer(),
          if (onCopy != null)
            IconButton(
              icon: Icon(copyPasteBehavior.copyIcon),
              onPressed: onCopy,
              iconSize: effectiveIconSize,
              visualDensity: toolIcons.visualDensity,
              padding: toolIcons.padding,
              alignment: toolIcons.alignment,
              splashRadius: toolIcons.splashRadius,
              tooltip: copyTooltip,
              constraints: toolIcons.constraints,
            ),
          if (onPaste != null)
            IconButton(
              icon: Icon(copyPasteBehavior.pasteIcon),
              onPressed: onPaste,
              iconSize: effectiveIconSize,
              visualDensity: toolIcons.visualDensity,
              padding: toolIcons.padding,
              alignment: toolIcons.alignment,
              splashRadius: toolIcons.splashRadius,
              tooltip: pasteTooltip,
              constraints: toolIcons.constraints,
            ),
          if (onClose != null && !toolIcons.closeIsLast)
            IconButton(
              icon: Icon(toolIcons.closeIcon),
              onPressed: onClose,
              iconSize: effectiveIconSize,
              visualDensity: toolIcons.visualDensity,
              padding: toolIcons.padding,
              alignment: toolIcons.alignment,
              splashRadius: toolIcons.splashRadius,
              tooltip: closeTooltip,
              constraints: toolIcons.constraints,
            ),
          if (onOk != null)
            IconButton(
              icon: Icon(toolIcons.okIcon),
              onPressed: onOk,
              iconSize: effectiveIconSize,
              visualDensity: toolIcons.visualDensity,
              padding: toolIcons.padding,
              alignment: toolIcons.alignment,
              splashRadius: toolIcons.splashRadius,
              tooltip: okTooltip,
              constraints: toolIcons.constraints,
            ),
          if (onClose != null && toolIcons.closeIsLast)
            IconButton(
              icon: Icon(toolIcons.closeIcon),
              onPressed: onClose,
              iconSize: effectiveIconSize,
              visualDensity: toolIcons.visualDensity,
              padding: toolIcons.padding,
              alignment: toolIcons.alignment,
              splashRadius: toolIcons.splashRadius,
              tooltip: closeTooltip,
              constraints: toolIcons.constraints,
            ),
        ],
      ),
    );
  }
}

// ignore_for_file: use_super_parameters

import 'package:flutter/material.dart';

import '../universal_widgets/context_popup_menu.dart';

/// Enum to handle copy and paste commands.
///
/// Not library exposed, private to the library.
enum CopyPasteCommands {
  /// Copy command
  copy,

  /// Paste command
  paste,
}

/// A cut, copy paste long press menu.
///
/// Not library exposed, private to the library.
@immutable
class ContextCopyPasteMenu extends StatelessWidget {
  /// Default const constructor.
  const ContextCopyPasteMenu({
    Key? key,
    this.useLongPress = false,
    this.useSecondaryTapDown = false,
    this.useSecondaryOnDesktopLongOnDevice = false,
    this.useSecondaryOnDesktopLongOnDeviceAndWeb = true,
    this.menuWidth = 80,
    this.menuItemHeight = 30,
    this.copyLabel,
    this.copyIcon = Icons.copy,
    this.pasteLabel,
    this.pasteIcon = Icons.paste,
    this.menuIconThemeData,
    this.menuThemeData,
    required this.onSelected,
    this.onOpen,
    required this.child,
  }) : super(key: key);

  /// Use long press to show context menu.
  ///
  /// Defaults to false.
  final bool useLongPress;

  /// Use secondary button tap down to show context menu.
  ///
  /// Secondary button is typically the right button on a mouse, but may in the
  /// host system be configured to be some other buttons as well, often by
  /// switching mouse right and left buttons.
  /// Defaults to false.
  final bool useSecondaryTapDown;

  /// Use secondary tap down on desktop and web, but long press on
  /// iOS/Android device.
  ///
  /// Secondary button is typically the right button on a mouse, but may in the
  /// host system be configured to be some other buttons as well, often by
  /// switching mouse right and left buttons.
  ///
  /// Defaults to false.
  final bool useSecondaryOnDesktopLongOnDevice;

  /// Use secondary tap down on desktop, but long press on
  /// iOS/Android device and Web.
  ///
  /// Secondary button is typically the right button on a mouse, but may in the
  /// host system be configured to be some other buttons as well, often by
  /// switching mouse right and left buttons.
  ///
  /// Defaults to true.
  final bool useSecondaryOnDesktopLongOnDeviceAndWeb;

  /// The width of the menu.
  ///
  /// Defaults to 80 dp.
  final double menuWidth;

  /// The height of each menu item.
  ///
  /// Defaults to 30 dp.
  final double menuItemHeight;

  /// Text label used for the copy action in the menu.
  ///
  /// If null, defaults to MaterialLocalizations.of(context).copyButtonLabel.
  final String? copyLabel;

  /// Icon used for the copy action in the menu.
  ///
  /// Defaults to [Icons.copy].
  final IconData copyIcon;

  /// Text label used for the paste action in the menu.
  ///
  /// If null, defaults to MaterialLocalizations.of(context).pasteButtonLabel.
  final String? pasteLabel;

  /// Icon used for the paste action in the menu.
  ///
  /// Defaults to [Icons.paste].
  final IconData pasteIcon;

  /// The theme for the menu icons.
  ///
  /// The menu is compact, so icons are small by design.
  ///
  /// Uses any none null property in passed in [IconThemeData]. If the
  /// passed value is null, or any property in it is null, then it uses
  /// property values from `Theme.of(context).iconTheme`, if they are not
  /// null. For any null value, the following fallback defaults are used:
  ///   color: remains null, so default [IconThemeData] color behavior is kept.
  ///   size: 16
  ///   opacity: 0.90
  final IconThemeData? menuIconThemeData;

  /// The theme of the popup menu.
  ///
  /// Uses any none null property in passed in [PopupMenuThemeData]. If the
  /// passed value is null, or any property in it is null, then it uses
  /// property values from `Theme.of(context).popupMenuTheme`, if they are not
  /// null. For any null value, the following fallback defaults are used:
  ///   color: theme.cardColor.withOpacity(0.9)
  ///   shape: RoundedRectangleBorder(
  ///            borderRadius: BorderRadius.circular(8),
  ///            side: BorderSide(
  ///            color: theme.dividerColor))
  ///   elevation: 3
  ///   textStyle: theme.textTheme.bodyMedium
  ///   enableFeedback: true
  final PopupMenuThemeData? menuThemeData;

  /// ValueChanged callback with selected item in the long press menu.
  /// Is null if menu closed without selection by clicking outside the menu.
  final ValueChanged<CopyPasteCommands?> onSelected;

  /// Optional void callback, called when the long press menu is opened.
  /// A way to tell when a long press opened the menu.
  final VoidCallback? onOpen;

  /// The child that gets the CopyPaste long press menu.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    // This is a merge of provided menuThemeData, with surrounding theme, with
    // fallback to default values.
    final PopupMenuThemeData effectiveMenuTheme = theme.popupMenuTheme.copyWith(
      color: menuThemeData?.color ??
          theme.popupMenuTheme.color ??
          theme.cardColor.withOpacity(0.9),
      shape: menuThemeData?.shape ??
          theme.popupMenuTheme.shape ??
          RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8),
              side: BorderSide(color: theme.dividerColor)),
      elevation:
          menuThemeData?.elevation ?? theme.popupMenuTheme.elevation ?? 3,
      textStyle: menuThemeData?.textStyle ??
          theme.popupMenuTheme.textStyle ??
          theme.textTheme.bodyMedium ??
          const TextStyle(fontSize: 14),
      enableFeedback: menuThemeData?.enableFeedback ??
          theme.popupMenuTheme.enableFeedback ??
          true,
    );
    // This is a merge of provided iconThemeData, with surrounding theme, with
    // fallback to default values, color has no default, remains as null.
    final IconThemeData effectiveIconTheme = theme.iconTheme.copyWith(
      color: menuIconThemeData?.color ?? theme.iconTheme.color,
      size: menuIconThemeData?.size ?? theme.iconTheme.size ?? 16,
      opacity: menuIconThemeData?.opacity ?? theme.iconTheme.opacity ?? 0.90,
    );
    // Get the Material localizations.
    final MaterialLocalizations translate = MaterialLocalizations.of(context);
    return Theme(
      data: theme.copyWith(
          popupMenuTheme: effectiveMenuTheme, iconTheme: effectiveIconTheme),
      child: ContextPopupMenu<CopyPasteCommands>(
        useLongPress: useLongPress,
        useSecondaryTapDown: useSecondaryTapDown,
        useSecondaryOnDesktopLongOnDevice: useSecondaryOnDesktopLongOnDevice,
        useSecondaryOnDesktopLongOnDeviceAndWeb:
            useSecondaryOnDesktopLongOnDeviceAndWeb,
        items: <PopupMenuEntry<CopyPasteCommands>>[
          PopupMenuItem<CopyPasteCommands>(
            value: CopyPasteCommands.copy,
            height: menuItemHeight,
            child: SizedBox(
              width: menuWidth,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: <Widget>[
                  Text(copyLabel ?? translate.copyButtonLabel),
                  Icon(copyIcon),
                ],
              ),
            ),
          ),
          PopupMenuItem<CopyPasteCommands>(
            value: CopyPasteCommands.paste,
            height: menuItemHeight,
            child: SizedBox(
              width: menuWidth,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: <Widget>[
                  Text(pasteLabel ?? translate.pasteButtonLabel),
                  Icon(pasteIcon),
                ],
              ),
            ),
          ),
        ],
        onSelected: onSelected,
        onOpen: onOpen,
        child: child,
      ),
    );
  }
}

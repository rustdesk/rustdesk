import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../universal_widgets/if_wrapper.dart';
import 'context_copy_paste_menu.dart';

/// A function that returns a future with no return value.
///
/// Used as signature for the clipboard actions function.
typedef VoidClipboard = Future<void> Function();

/// Manages the keyboard shortcuts for the color picker.
class CopyPasteHandler extends StatelessWidget {
  /// Manages the keyboard shortcuts for the color picker.
  const CopyPasteHandler({
    super.key,
    required this.pasteFromClipboard,
    required this.copyToClipboard,
    required this.useContextMenu,
    required this.useLongPress,
    required this.useSecondaryTapDown,
    required this.useSecondaryOnDesktopLongOnDevice,
    required this.useSecondaryOnDesktopLongOnDeviceAndWeb,
    required this.onCopyPasteMenuOpened,
    required this.focusNode,
    required this.autoFocus,
    required this.noPasteIntent,
    required this.child,
  });

  /// A function that is called when the paste action is invoked.
  ///
  /// The function should paste from the clipboard, including any custom
  /// logic needed.
  final VoidClipboard pasteFromClipboard;

  /// A function that is called when the copy action is invoked.
  ///
  /// The function should copy from the clipboard, including any custom
  /// logic needed.
  final VoidClipboard copyToClipboard;

  /// Use the context menu for copy and paste.
  final bool useContextMenu;

  /// Use long press for copy and paste.
  final bool useLongPress;

  /// Use secondary tap down for copy and paste.
  final bool useSecondaryTapDown;

  /// Use secondary on desktop and long press on device for copy and paste.
  final bool useSecondaryOnDesktopLongOnDevice;

  /// Use secondary on desktop and long press on device and web for copy
  /// and paste.
  final bool useSecondaryOnDesktopLongOnDeviceAndWeb;

  /// A function that is called when the copy paste menu is opened.
  final VoidCallback onCopyPasteMenuOpened;

  /// A focus node that will be used to focus the child.
  final FocusNode focusNode;

  /// A boolean that determines if the child should be focused.
  final bool autoFocus;

  /// Typically the custom past intent is used, but when [noPasteIntent]
  /// is set to `true` it is not.
  ///
  /// This is only done when widget sets `copyPasteBehavior.editUsesParsedPaste`
  /// to `false` true and the color code entry field is focused.
  ///
  /// This results in that the paste intent's color code parser is not used
  /// and that text is pasted into the ColorCode entry field as is.
  ///
  /// The color code TextField still has supported hex RGB character and max
  /// length validation, so anything not adhering to that will still be
  /// silently filtered out of the paster result, but the paste will not be
  /// validate via the supported multiple RGB color code RGB parser.
  ///
  /// Typically you want to use the paste intent and set
  /// `copyPasteBehavior.editUsesParsedPaste` to `true` to get the full
  /// color code parsing support. Setting it to false is supported for
  /// legacy compatibility with FlexColorPicker before 2.0.0.
  final bool noPasteIntent;

  /// The child that will use the keyboard shortcuts.
  ///
  /// Should wrap the entire color picker.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final TargetPlatform platform = Theme.of(context).platform;
    final bool useMetaCommand =
        platform == TargetPlatform.iOS || platform == TargetPlatform.macOS;
    return Shortcuts(
      shortcuts: <ShortcutActivator, Intent>{
        if (useMetaCommand)
          LogicalKeySet(LogicalKeyboardKey.meta, LogicalKeyboardKey.keyC):
              const CopyIntent()
        else
          LogicalKeySet(LogicalKeyboardKey.control, LogicalKeyboardKey.keyC):
              const CopyIntent(),
        if (useMetaCommand)
          LogicalKeySet(LogicalKeyboardKey.meta, LogicalKeyboardKey.keyV):
              const PasteIntent()
        else
          LogicalKeySet(LogicalKeyboardKey.control, LogicalKeyboardKey.keyC):
              const PasteIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          if (!noPasteIntent) PasteIntent: PasteAction(pasteFromClipboard),
          CopyIntent: CopyAction(copyToClipboard),
        },
        child: Builder(builder: (BuildContext context) {
          return Focus(
              focusNode: focusNode,
              autofocus: autoFocus,
              canRequestFocus: true,
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: focusNode.requestFocus,
                child: IfWrapper(
                    condition: useContextMenu,
                    builder: (BuildContext context, Widget child) {
                      return ContextCopyPasteMenu(
                        useLongPress: useLongPress,
                        useSecondaryTapDown: useSecondaryTapDown,
                        useSecondaryOnDesktopLongOnDevice:
                            useSecondaryOnDesktopLongOnDevice,
                        useSecondaryOnDesktopLongOnDeviceAndWeb:
                            useSecondaryOnDesktopLongOnDeviceAndWeb,
                        onSelected: (CopyPasteCommands? value) {
                          if (value == CopyPasteCommands.copy) {
                            unawaited(copyToClipboard());
                          }
                          if (value == CopyPasteCommands.paste) {
                            unawaited(pasteFromClipboard());
                          }
                        },
                        onOpen: onCopyPasteMenuOpened,
                        child: child,
                      );
                    },
                    child: child),
              ));
        }),
      ),
    );
  }
}

/// An intent that is bound to PasteAction to paste from clipboard.
class PasteIntent extends Intent {
  /// An intent that is bound to PasteAction to paste from clipboard.
  const PasteIntent();
}

/// An action that is bound to PasteIntent.
class PasteAction extends Action<PasteIntent> {
  /// An action that is bound to PasteIntent.
  PasteAction(this.pasteFromClipboard);

  /// A function that is called when the paste action is invoked.
  ///
  /// The function should paste from the clipboard, including any custom
  /// logic needed.
  final VoidClipboard pasteFromClipboard;

  @override
  Object? invoke(covariant PasteIntent intent) async {
    await pasteFromClipboard.call();
    return KeyEventResult.handled;
  }
}

/// An intent that is bound to CopyAction to copy to ClipBoard.
class CopyIntent extends Intent {
  /// An intent that is bound to CopyAction to copy to ClipBoard.
  const CopyIntent();
}

/// An action that is bound to CopyIntent.
class CopyAction extends Action<CopyIntent> {
  /// An action that is bound to CopyIntent.
  CopyAction(this.copyToClipboard);

  /// A function that is called when the copy action is invoked.
  ///
  /// The function should copy from the clipboard, including any custom
  /// logic needed.
  final VoidClipboard copyToClipboard;

  @override
  Object? invoke(covariant CopyIntent intent) async {
    await copyToClipboard.call();
    return KeyEventResult.handled;
  }
}

// flutter/lib/mobile/pages/mobile_keyboard_shortcuts_page.dart
//
// Mobile shell for the Keyboard Shortcuts configuration page. Mirrors
// `desktop/pages/desktop_keyboard_shortcuts_page.dart` but with a touch-
// friendly layout (ListTile rows instead of dense rows) and a hint banner
// that explains the recording flow only works with a physical keyboard.
//
// All actual logic — group definitions, JSON I/O, conflict-replace flow,
// recording-dialog round-trip, "Reset to defaults" — lives in the shared
// `common/widgets/keyboard_shortcuts/page_body.dart`. This file only
// supplies the AppBar, the AppBar action, and the platform hint banner.
//
// Mobile keyboard detection limitation: Flutter has no reliable
// "is a physical keyboard attached?" API on iOS or Android. Soft keyboards
// don't generate the `KeyDownEvent`s the recording dialog listens for, so
// in practice the dialog only does anything useful when the user actually
// has a hardware keyboard plugged in (USB / Bluetooth / Smart Connector).
// For V1 we don't try to detect attachment — we just surface the
// requirement as an in-page hint instead of disabling the Edit button.

import 'package:flutter/material.dart';

import '../../common.dart';
import '../../common/widgets/keyboard_shortcuts/page_body.dart';

class MobileKeyboardShortcutsPage extends StatefulWidget {
  const MobileKeyboardShortcutsPage({Key? key}) : super(key: key);

  @override
  State<MobileKeyboardShortcutsPage> createState() =>
      _MobileKeyboardShortcutsPageState();
}

class _MobileKeyboardShortcutsPageState
    extends State<MobileKeyboardShortcutsPage> {
  final GlobalKey<KeyboardShortcutsPageBodyState> _bodyKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(translate('Keyboard Shortcuts')),
        actions: [
          IconButton(
            tooltip: translate('Reset to defaults'),
            onPressed: () =>
                _bodyKey.currentState?.resetToDefaultsWithConfirm(),
            icon: const Icon(Icons.restore),
          ),
        ],
      ),
      body: KeyboardShortcutsPageBody(
        key: _bodyKey,
        compact: false,
        editButtonHint: translate('shortcut-mobile-physical-keyboard-tip'),
        headerBanner: _PhysicalKeyboardHintBanner(theme: theme),
      ),
    );
  }
}

/// A muted info banner shown above the master toggle on mobile. We can't
/// reliably detect whether a physical keyboard is attached, so instead of
/// disabling the Edit button we surface the requirement up front.
class _PhysicalKeyboardHintBanner extends StatelessWidget {
  final ThemeData theme;
  const _PhysicalKeyboardHintBanner({required this.theme});

  @override
  Widget build(BuildContext context) {
    final color = theme.colorScheme.primary.withOpacity(0.08);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.info_outline,
              size: 18, color: theme.colorScheme.primary),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              translate('shortcut-mobile-physical-keyboard-tip'),
              style: TextStyle(color: theme.colorScheme.onSurface),
            ),
          ),
        ],
      ),
    );
  }
}

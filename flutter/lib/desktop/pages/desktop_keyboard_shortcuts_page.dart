// flutter/lib/desktop/pages/desktop_keyboard_shortcuts_page.dart
//
// Desktop shell for the Keyboard Shortcuts configuration page. Users land
// here from the General settings tab. The page exposes:
//   * A top-level enable/disable toggle (mirrors the General-tab toggle —
//     same JSON key, same semantics).
//   * A grouped, scrollable list of actions, each with a current binding and
//     edit / clear icons.
//   * An AppBar "Reset to defaults" action with a confirmation dialog.
//
// All edits write back to LocalConfig under [kShortcutLocalConfigKey] in the
// canonical {enabled, bindings:[{action,mods,key}]} shape that the Rust and
// Web matchers consume.
//
// The body — group definitions, JSON I/O, conflict-replace flow,
// recording-dialog round-trip — lives in
// `common/widgets/keyboard_shortcuts/page_body.dart` and is shared with the
// mobile shell at `mobile/pages/mobile_keyboard_shortcuts_page.dart`.

import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../common/widgets/keyboard_shortcuts/page_body.dart';

class DesktopKeyboardShortcutsPage extends StatefulWidget {
  const DesktopKeyboardShortcutsPage({Key? key}) : super(key: key);

  @override
  State<DesktopKeyboardShortcutsPage> createState() =>
      _DesktopKeyboardShortcutsPageState();
}

class _DesktopKeyboardShortcutsPageState
    extends State<DesktopKeyboardShortcutsPage> {
  final GlobalKey<KeyboardShortcutsPageBodyState> _bodyKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    final foregroundColor =
        AppBarTheme.of(context).titleTextStyle?.color ?? Colors.white;
    return Scaffold(
      appBar: AppBar(
        title: Text(translate('Keyboard Shortcuts')),
        actions: [
          TextButton.icon(
            style: TextButton.styleFrom(foregroundColor: foregroundColor),
            onPressed: () =>
                _bodyKey.currentState?.resetToDefaultsWithConfirm(),
            icon: const Icon(Icons.restore),
            label: Text(translate('Reset to defaults')),
          ).marginOnly(right: 12),
        ],
      ),
      body: KeyboardShortcutsPageBody(
        key: _bodyKey,
        compact: true,
        // Desktop's General settings tab already exposes the Enable +
        // Pass-through checkboxes (it's the only entry point to this page),
        // so we hide the duplicates here. Mobile shells keep the default
        // (true) because their entry tile doesn't carry the toggles.
        showMasterToggles: false,
      ),
    );
  }
}

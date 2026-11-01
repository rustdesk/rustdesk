import 'package:flutter/material.dart';

enum SessionModifier { control, option, shift, command }

/// Special keys stay available with either the virtual deck or system keyboard.
class SessionKeyMenu extends StatelessWidget {
  const SessionKeyMenu(
      {super.key,
      required this.enabled,
      required this.isMac,
      required this.modifiers,
      required this.onModifier,
      required this.onKey,
      required this.onShortcut});

  final bool enabled;
  final bool isMac;
  final Set<SessionModifier> modifiers;
  final ValueChanged<SessionModifier> onModifier;
  final ValueChanged<String> onKey;
  final ValueChanged<String> onShortcut;

  @override
  Widget build(BuildContext context) => PopupMenuButton<String>(
        tooltip: 'Fn',
        enabled: enabled,
        icon: Text('fn',
            style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: Theme.of(context)
                    .colorScheme
                    .onSurface
                    .withOpacity(enabled ? 1 : .38))),
        onSelected: (value) {
          if (!enabled) return;
          if (value.startsWith('modifier:')) {
            onModifier(SessionModifier.values[int.parse(value.substring(9))]);
          } else if (value.startsWith('shortcut:')) {
            onShortcut.call(value.substring(9));
          } else {
            onKey(value);
          }
        },
        itemBuilder: (_) => [
          for (final modifier in SessionModifier.values)
            CheckedPopupMenuItem(
              value: 'modifier:${modifier.index}',
              checked: modifiers.contains(modifier),
              child: Text(switch (modifier) {
                SessionModifier.control => 'Ctrl',
                SessionModifier.option => isMac ? 'Option' : 'Alt',
                SessionModifier.shift => 'Shift',
                SessionModifier.command => isMac ? 'Command' : 'Win',
              }),
            ),
          const PopupMenuDivider(),
          for (final entry in const {
            'Esc': 'VK_ESCAPE',
            'Tab': 'VK_TAB',
            'Enter': 'VK_RETURN',
            'Left': 'VK_LEFT',
            'Up': 'VK_UP',
            'Down': 'VK_DOWN',
            'Right': 'VK_RIGHT',
            'Home': 'VK_HOME',
            'End': 'VK_END',
            'Ins': 'VK_INSERT',
            'Del': 'VK_DELETE',
            'PgUp': 'VK_PRIOR',
            'PgDn': 'VK_NEXT',
          }.entries)
            PopupMenuItem(value: entry.value, child: Text(entry.key)),
          for (var i = 1; i <= 12; i++)
            PopupMenuItem(value: 'VK_F$i', child: Text('F$i')),
          if (!isMac)
            for (final entry in const {
              'PrtSc': 'VK_SNAPSHOT',
              'ScrLk': 'VK_SCROLL',
              'Pause': 'VK_PAUSE',
              'Menu': 'Apps'
            }.entries)
              PopupMenuItem(value: entry.value, child: Text(entry.key)),
          for (final letter in ['C', 'V', 'S'])
            PopupMenuItem(
                value: 'shortcut:VK_$letter',
                child: Text('${isMac ? 'Cmd' : 'Ctrl'}+$letter')),
        ],
      );
}

import '../../../consts.dart';
import '../models/key_def.dart';

StripLayout stripLayoutForPlatform(String platform) {
  final modifiers = switch (platform) {
    kPeerPlatformMacOS => [
        KeyDef(label: 'Ctrl', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: '⌥', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: '⌘', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: '⇧', keyName: 'shift', type: KeyType.modifier),
      ],
    kPeerPlatformWindows => [
        KeyDef(label: 'Ctrl', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: 'Alt', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: '⊞', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: 'Shift', keyName: 'shift', type: KeyType.modifier),
      ],
    _ => [
        KeyDef(label: 'Ctrl', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: 'Alt', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: 'Super', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: 'Shift', keyName: 'shift', type: KeyType.modifier),
      ],
  };

  return StripLayout(
    rows: [
      // Row 1: utilities + modifiers + Enter (left)  |  strip-toggle + ⌨ + ✕ + ⌫ + Tab (right)
      StripRow(
        left: [
          KeyDef(label: 'Esc', keyName: 'escape', type: KeyType.regular),
          ...modifiers,
          KeyDef(label: '⏎', keyName: 'return', type: KeyType.regular),
        ],
        right: [
          KeyDef(label: '▲▼', keyName: '', type: KeyType.stripToggle, widthFactor: 0.7),
          KeyDef(label: '⌨', keyName: '', type: KeyType.keyboardToggle, widthFactor: 0.7),
          KeyDef(label: '✕', keyName: '', type: KeyType.disconnect, widthFactor: 0.7),
          KeyDef(label: '⌫', keyName: 'backspace', type: KeyType.regular),
          KeyDef(label: 'Tab', keyName: 'tab', type: KeyType.regular),
          KeyDef(label: '💬', keyName: '', type: KeyType.chatToggle, widthFactor: 0.7),
        ],
      ),
      // Row 2: macros (left)  |  arrow cluster (right)
      StripRow(
        left: [
          KeyDef(label: '⚡', keyName: '', type: KeyType.macroOpener),
        ],
        right: [
          KeyDef(label: '←', keyName: 'left', type: KeyType.regular),
          KeyDef(label: '↓', keyName: 'down', type: KeyType.regular),
          KeyDef(label: '↑', keyName: 'up', type: KeyType.regular),
          KeyDef(label: '→', keyName: 'right', type: KeyType.regular),
        ],
      ),
      // Row 3: nav cluster
      StripRow(
        left: [],
        right: [
          KeyDef(label: 'Home', keyName: 'home', type: KeyType.regular),
          KeyDef(label: 'PgUp', keyName: 'pageup', type: KeyType.regular),
          KeyDef(label: 'PgDn', keyName: 'pagedown', type: KeyType.regular),
          KeyDef(label: 'End', keyName: 'end', type: KeyType.regular),
        ],
      ),
    ],
  );
}

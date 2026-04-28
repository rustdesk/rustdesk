import '../models/key_def.dart';

const defaultStripLayout = StripLayout(
  rows: [
    // Row 1: utilities + modifiers + Enter (left)  |  ⌫ + Tab (right)
    StripRow(
      left: [
        KeyDef(label: 'Esc', keyName: 'escape', type: KeyType.regular),
        KeyDef(label: '⌃', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: '⌥', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: '⌘', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: '⏎', keyName: 'return', type: KeyType.regular),
      ],
      right: [
        KeyDef(label: '⌫', keyName: 'backspace', type: KeyType.regular),
        KeyDef(label: 'Tab', keyName: 'tab', type: KeyType.regular),
      ],
    ),
    // Row 2: macros + keyboard + disconnect (left)  |  arrow cluster (right)
    StripRow(
      left: [
        KeyDef(label: '⚡', keyName: '', type: KeyType.macroOpener),
        KeyDef(label: '⌨', keyName: '', type: KeyType.keyboardToggle),
        KeyDef(label: 'End', keyName: '', type: KeyType.disconnect),
      ],
      right: [
        KeyDef(label: '←', keyName: 'left', type: KeyType.regular),
        KeyDef(label: '↓', keyName: 'down', type: KeyType.regular),
        KeyDef(label: '↑', keyName: 'up', type: KeyType.regular),
        KeyDef(label: '→', keyName: 'right', type: KeyType.regular),
      ],
    ),
  ],
);

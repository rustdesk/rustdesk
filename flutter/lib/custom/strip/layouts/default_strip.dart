import '../models/key_def.dart';

const defaultStripLayout = StripLayout(
  rows: [
    // Row 1: utilities + modifiers (left)  |  ⌫ + Tab (right)
    StripRow(
      left: [
        KeyDef(label: 'Esc', keyName: 'escape', type: KeyType.regular),
        KeyDef(label: '⌃', keyName: 'control', type: KeyType.modifier),
        KeyDef(label: '⌥', keyName: 'alt', type: KeyType.modifier),
        KeyDef(label: '⌘', keyName: 'meta', type: KeyType.modifier),
        KeyDef(label: 'Fn', keyName: '', type: KeyType.layer),
      ],
      right: [
        KeyDef(label: '⌫', keyName: 'backspace', type: KeyType.regular),
        KeyDef(
          label: 'Tab',
          keyName: 'tab',
          type: KeyType.regular,
          widthFactor: 1.2,
        ),
      ],
    ),
    // Row 2: macros + keyboard toggle (left)  |  arrow cluster (right)
    StripRow(
      left: [
        KeyDef(
          label: '⚡ Macros',
          keyName: '',
          type: KeyType.macroOpener,
          widthFactor: 1.6,
        ),
        KeyDef(label: '⌨', keyName: '', type: KeyType.keyboardToggle),
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

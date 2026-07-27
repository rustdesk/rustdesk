/// Reviewed mobile terminal keyboard layout from PR #15532.
///
/// Keeping the key order outside the widget makes the intended layout explicit
/// and prevents behavior fixes from silently moving keys between rows.
const terminalKeyboardRow1Keys = ['Esc', '/', '|', 'Home', '↑', 'End', r'\'];
const terminalKeyboardRow2Keys = ['Tab', 'Ctrl+C', '~', '←', '↓', '→'];
const terminalKeyboardRow3Keys = ['Ctrl', 'Alt', '-', 'PgUp', 'PgDn'];

const terminalKeyboardKeyWidth = 48.0;
const terminalKeyboardKeySpacing = 2.0;

/// Empty 48dp slots keep expanded Row3 aligned with the two rows above it.
const terminalKeyboardRow3TrailingPlaceholderCount = 2;

/// Returns the fixed width occupied by a row of equally sized key slots.
double terminalKeyboardRowWidth(int slotCount) {
  if (slotCount <= 0) return 0;
  return slotCount * terminalKeyboardKeyWidth +
      (slotCount - 1) * terminalKeyboardKeySpacing;
}

/// A physical USB HID keyboard stroke used by VM keyboard mode.
class VmKeyStroke {
  const VmKeyStroke(this.usbHid, {this.shift = false});

  final int usbHid;
  final bool shift;

  @override
  bool operator ==(Object other) =>
      other is VmKeyStroke &&
      other.usbHid == usbHid &&
      other.shift == shift;

  @override
  int get hashCode => Object.hash(usbHid, shift);

  @override
  String toString() =>
      'VmKeyStroke(usbHid: 0x${usbHid.toRadixString(16)}, shift: $shift)';
}

// USB HID usages from the Keyboard/Keypad usage page (0x07). Flutter's
// PhysicalKeyboardKey.usbHidUsage exposes the same values in its low 16 bits.
const vmLeftControlUsbHid = 0xE0;
const vmLeftShiftUsbHid = 0xE1;
const vmLeftAltUsbHid = 0xE2;
const vmLeftGuiUsbHid = 0xE3;

/// One physical key transition sent by VM keyboard mode.
class VmKeyEvent {
  const VmKeyEvent(this.usbHid, this.down);

  final int usbHid;
  final bool down;

  @override
  bool operator ==(Object other) =>
      other is VmKeyEvent && other.usbHid == usbHid && other.down == down;

  @override
  int get hashCode => Object.hash(usbHid, down);

  @override
  String toString() =>
      'VmKeyEvent(usbHid: 0x${usbHid.toRadixString(16)}, down: $down)';
}

/// Builds a balanced physical event sequence without releasing modifiers that
/// are already held by a physical keyboard.
List<VmKeyEvent> vmKeyEventsForStroke(
  VmKeyStroke stroke, {
  required bool ctrl,
  required bool shift,
  required bool alt,
  required bool command,
  bool ctrlAlreadyDown = false,
  bool shiftAlreadyDown = false,
  bool altAlreadyDown = false,
  bool commandAlreadyDown = false,
}) {
  final modifiers = <int>[
    if (ctrl && !ctrlAlreadyDown) vmLeftControlUsbHid,
    if ((shift || stroke.shift) && !shiftAlreadyDown) vmLeftShiftUsbHid,
    if (alt && !altAlreadyDown) vmLeftAltUsbHid,
    if (command && !commandAlreadyDown) vmLeftGuiUsbHid,
  ];
  return [
    for (final modifier in modifiers) VmKeyEvent(modifier, true),
    VmKeyEvent(stroke.usbHid, true),
    VmKeyEvent(stroke.usbHid, false),
    for (final modifier in modifiers.reversed) VmKeyEvent(modifier, false),
  ];
}

const Map<int, VmKeyStroke> _specialAsciiStrokes = {
  0x08: VmKeyStroke(0x2A),
  0x09: VmKeyStroke(0x2B),
  0x0A: VmKeyStroke(0x28),
  0x0D: VmKeyStroke(0x28),
  0x20: VmKeyStroke(0x2C),
  0x21: VmKeyStroke(0x1E, shift: true),
  0x22: VmKeyStroke(0x34, shift: true),
  0x23: VmKeyStroke(0x20, shift: true),
  0x24: VmKeyStroke(0x21, shift: true),
  0x25: VmKeyStroke(0x22, shift: true),
  0x26: VmKeyStroke(0x24, shift: true),
  0x27: VmKeyStroke(0x34),
  0x28: VmKeyStroke(0x26, shift: true),
  0x29: VmKeyStroke(0x27, shift: true),
  0x2A: VmKeyStroke(0x25, shift: true),
  0x2B: VmKeyStroke(0x2E, shift: true),
  0x2C: VmKeyStroke(0x36),
  0x2D: VmKeyStroke(0x2D),
  0x2E: VmKeyStroke(0x37),
  0x2F: VmKeyStroke(0x38),
  0x3A: VmKeyStroke(0x33, shift: true),
  0x3B: VmKeyStroke(0x33),
  0x3C: VmKeyStroke(0x36, shift: true),
  0x3D: VmKeyStroke(0x2E),
  0x3E: VmKeyStroke(0x37, shift: true),
  0x3F: VmKeyStroke(0x38, shift: true),
  0x40: VmKeyStroke(0x1F, shift: true),
  0x5B: VmKeyStroke(0x2F),
  0x5C: VmKeyStroke(0x31),
  0x5D: VmKeyStroke(0x30),
  0x5E: VmKeyStroke(0x23, shift: true),
  0x5F: VmKeyStroke(0x2D, shift: true),
  0x60: VmKeyStroke(0x35),
  0x7B: VmKeyStroke(0x2F, shift: true),
  0x7C: VmKeyStroke(0x31, shift: true),
  0x7D: VmKeyStroke(0x30, shift: true),
  0x7E: VmKeyStroke(0x35, shift: true),
};

/// Converts an ASCII rune to a physical US-layout keyboard stroke.
///
/// The receiving VM guest must use a US-compatible keyboard layout. Returning
/// null leaves the character on the existing Unicode/IME input path.
VmKeyStroke? vmKeyStrokeForRune(int rune) {
  if (rune >= 0x61 && rune <= 0x7A) {
    return VmKeyStroke(0x04 + rune - 0x61);
  }
  if (rune >= 0x41 && rune <= 0x5A) {
    return VmKeyStroke(0x04 + rune - 0x41, shift: true);
  }
  if (rune >= 0x31 && rune <= 0x39) {
    return VmKeyStroke(0x1E + rune - 0x31);
  }
  if (rune == 0x30) {
    return const VmKeyStroke(0x27);
  }
  return _specialAsciiStrokes[rune];
}

/// Returns null when any character requires the normal Unicode/IME path.
List<VmKeyStroke>? vmKeyStrokesForText(String text) {
  if (text.isEmpty) return null;

  final strokes = <VmKeyStroke>[];
  for (final rune in text.runes) {
    final stroke = vmKeyStrokeForRune(rune);
    if (stroke == null) return null;
    strokes.add(stroke);
  }
  return strokes;
}

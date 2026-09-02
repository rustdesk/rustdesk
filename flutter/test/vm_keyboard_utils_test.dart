import 'package:flutter_hbb/models/vm_keyboard_utils.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('VM keyboard utilities', () {
    test('maps 12345 to USB HID usages instead of ASCII codes', () {
      final usages = vmKeyStrokesForText('12345')!
          .map((stroke) => stroke.usbHid)
          .toList();

      expect(usages, const [0x1E, 0x1F, 0x20, 0x21, 0x22]);
      expect(usages, isNot(const [0x31, 0x32, 0x33, 0x34, 0x35]));
    });

    test('maps lowercase and uppercase letters', () {
      expect(vmKeyStrokeForRune('a'.runes.single), const VmKeyStroke(0x04));
      expect(vmKeyStrokeForRune('Z'.runes.single),
          const VmKeyStroke(0x1D, shift: true));
    });

    test('maps editing keys used by the hidden iOS text field', () {
      expect(vmKeyStrokeForRune('\b'.runes.single), const VmKeyStroke(0x2A));
      expect(vmKeyStrokeForRune('\n'.runes.single), const VmKeyStroke(0x28));
      expect(vmKeyStrokeForRune(' '.runes.single), const VmKeyStroke(0x2C));
    });

    test('leaves non-ASCII text on the Unicode path', () {
      expect(vmKeyStrokeForRune('中'.runes.single), isNull);
      expect(vmKeyStrokesForText('abc中'), isNull);
    });

    test('maps shifted number-row symbols', () {
      expect(vmKeyStrokeForRune(r'$'.runes.single),
          const VmKeyStroke(0x21, shift: true));
      expect(vmKeyStrokeForRune('@'.runes.single),
          const VmKeyStroke(0x1F, shift: true));
    });

    test('wraps a VM stroke in active toolbar modifiers', () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x06),
        ctrl: true,
        shift: false,
        alt: false,
        command: false,
      );

      expect(
        events,
        const [
          VmKeyEvent(vmLeftControlUsbHid, true),
          VmKeyEvent(0x06, true),
          VmKeyEvent(0x06, false),
          VmKeyEvent(vmLeftControlUsbHid, false),
        ],
      );
    });

    test('wraps a VM stroke in the active Alt modifier', () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x2B),
        ctrl: false,
        shift: false,
        alt: true,
        command: false,
      );

      expect(
        events,
        const [
          VmKeyEvent(vmLeftAltUsbHid, true),
          VmKeyEvent(0x2B, true),
          VmKeyEvent(0x2B, false),
          VmKeyEvent(vmLeftAltUsbHid, false),
        ],
      );
    });

    test('wraps a VM stroke in the active Command or Win modifier', () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x0F),
        ctrl: false,
        shift: false,
        alt: false,
        command: true,
      );

      expect(
        events,
        const [
          VmKeyEvent(vmLeftGuiUsbHid, true),
          VmKeyEvent(0x0F, true),
          VmKeyEvent(0x0F, false),
          VmKeyEvent(vmLeftGuiUsbHid, false),
        ],
      );
    });

    test('does not duplicate toolbar and character shift', () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x1D, shift: true),
        ctrl: false,
        shift: true,
        alt: false,
        command: false,
      );

      expect(
        events,
        const [
          VmKeyEvent(vmLeftShiftUsbHid, true),
          VmKeyEvent(0x1D, true),
          VmKeyEvent(0x1D, false),
          VmKeyEvent(vmLeftShiftUsbHid, false),
        ],
      );
    });

    test('applies toolbar shift to an unshifted character', () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x04),
        ctrl: false,
        shift: true,
        alt: false,
        command: false,
      );

      expect(
        events,
        const [
          VmKeyEvent(vmLeftShiftUsbHid, true),
          VmKeyEvent(0x04, true),
          VmKeyEvent(0x04, false),
          VmKeyEvent(vmLeftShiftUsbHid, false),
        ],
      );
    });

    test('does not synthesize modifiers already held by a physical keyboard',
        () {
      final events = vmKeyEventsForStroke(
        const VmKeyStroke(0x04, shift: true),
        ctrl: true,
        shift: true,
        alt: true,
        command: true,
        ctrlAlreadyDown: true,
        shiftAlreadyDown: true,
        altAlreadyDown: true,
        commandAlreadyDown: true,
      );

      expect(
        events,
        const [
          VmKeyEvent(0x04, true),
          VmKeyEvent(0x04, false),
        ],
      );
    });

    test('keeps a physical modifier held across a VM soft-key stroke', () {
      final vmEvents = vmKeyEventsForStroke(
        const VmKeyStroke(0x06),
        ctrl: true,
        shift: false,
        alt: false,
        command: false,
        ctrlAlreadyDown: true,
      );

      final completeSequence = [
        const VmKeyEvent(vmLeftControlUsbHid, true),
        ...vmEvents,
        const VmKeyEvent(0x1B, true),
        const VmKeyEvent(0x1B, false),
        const VmKeyEvent(vmLeftControlUsbHid, false),
      ];

      expect(
        completeSequence,
        const [
          VmKeyEvent(vmLeftControlUsbHid, true),
          VmKeyEvent(0x06, true),
          VmKeyEvent(0x06, false),
          VmKeyEvent(0x1B, true),
          VmKeyEvent(0x1B, false),
          VmKeyEvent(vmLeftControlUsbHid, false),
        ],
      );
    });
  });
}

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_hbb/models/ime_text_bridge.dart';
import 'package:flutter_test/flutter_test.dart';

TextEditingValue _committed(String text) => TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );

TextEditingValue _composing(String text) => TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
      composing: TextRange(start: 0, end: text.length),
    );

void main() {
  group('ImeTextBridge', () {
    late List<String> sent;
    late ImeTextBridge bridge;

    setUp(() {
      sent = [];
      bridge = ImeTextBridge(onText: sent.add);
    });

    test('forwards text injected by an input method (Unikey/EVKey VK_PACKET)',
        () {
      bridge.updateEditingValue(_committed('ô'));
      expect(sent, ['ô']);
    });

    test('forwards only what was added since the last update', () {
      bridge.updateEditingValue(_committed('ô'));
      bridge.updateEditingValue(_committed('ôn'));
      bridge.updateEditingValue(_committed('ông'));
      expect(sent, ['ô', 'n', 'g']);
    });

    test('a multi-character insert is forwarded in one piece', () {
      bridge.updateEditingValue(_committed('ảng'));
      expect(sent, ['ảng']);
    });

    test('waits for an IME composition to be committed', () {
      bridge.updateEditingValue(_composing('vi'));
      bridge.updateEditingValue(_composing('việ'));
      expect(sent, isEmpty);
      bridge.updateEditingValue(_committed('việt'));
      expect(sent, ['việt']);
    });

    test('ignores updates that add nothing', () {
      bridge.updateEditingValue(_committed('a'));
      bridge.updateEditingValue(_committed('a'));
      bridge.updateEditingValue(TextEditingValue.empty);
      expect(sent, ['a']);
    });

    test('text that no longer extends the buffer is forwarded whole', () {
      bridge.updateEditingValue(_committed('abc'));
      bridge.updateEditingValue(_committed('x'));
      expect(sent, ['abc', 'x']);
    });

    test('after a buffer reset, a late update based on the old buffer is not '
        'sent twice', () {
      final b = ImeTextBridge(onText: sent.add, maxBufferLength: 3);
      b.updateEditingValue(_committed('abc')); // reaches the limit → reset
      b.updateEditingValue(_committed('abcd')); // engine had not reset yet
      b.updateEditingValue(_committed('e')); // engine reset applied
      expect(sent, ['abc', 'd', 'e']);
    });

    test('nothing is forwarded after detach', () {
      bridge.detach();
      expect(bridge.isAttached, isFalse);
    });
  });

  group('OrderedSender', () {
    test('runs tasks one after another in submission order', () async {
      final log = <String>[];
      final sender = OrderedSender();
      final slow = Completer<void>();
      sender.run(() async {
        await slow.future;
        log.add('backspace');
      });
      final done = sender.run(() async => log.add('ô'));
      await Future<void>.delayed(Duration.zero);
      expect(log, isEmpty, reason: 'the second task must wait for the first');
      slow.complete();
      await done;
      expect(log, ['backspace', 'ô']);
    });

    test('a failing task does not block the ones after it', () async {
      final log = <String>[];
      final sender = OrderedSender();
      sender.run(() async => throw StateError('boom'));
      await sender.run(() async => log.add('next'));
      expect(log, ['next']);
    });
  });

  group('shouldUseImeTextBridge', () {
    bool use({
      bool isWindows = true,
      bool inputSourceFlutter = true,
      String keyboardMode = 'translate',
      bool keyboardPerm = true,
      bool viewOnly = false,
    }) =>
        shouldUseImeTextBridge(
          isWindows: isWindows,
          inputSourceFlutter: inputSourceFlutter,
          keyboardMode: keyboardMode,
          keyboardPerm: keyboardPerm,
          viewOnly: viewOnly,
        );

    test('on for Windows + Input source 2, in Translate and in Map mode', () {
      expect(use(), isTrue);
      // Map mode is the default and is stored per peer: requiring Translate
      // would mean switching it for every remote device.
      expect(use(keyboardMode: 'map'), isTrue);
    });

    test('off everywhere else', () {
      expect(use(isWindows: false), isFalse);
      expect(use(inputSourceFlutter: false), isFalse);
      expect(use(keyboardMode: 'legacy'), isFalse);
      expect(use(keyboardPerm: false), isFalse);
      expect(use(viewOnly: true), isFalse);
    });
  });

  group('forwardingUsbHid', () {
    test('keeps a physical key Flutter recognised', () {
      expect(
        forwardingUsbHid(PhysicalKeyboardKey.keyA, LogicalKeyboardKey.keyA),
        PhysicalKeyboardKey.keyA.usbHidUsage & 0xFFFF,
      );
    });

    test('recovers keys an input method injects without a scan code', () {
      // Flutter maps scan code 0 to an id in its "windows plane" (unknown key).
      const unknown = PhysicalKeyboardKey(0x1500000000);
      expect(forwardingUsbHid(unknown, LogicalKeyboardKey.backspace),
          PhysicalKeyboardKey.backspace.usbHidUsage & 0xFFFF);
      expect(forwardingUsbHid(unknown, LogicalKeyboardKey.enter),
          PhysicalKeyboardKey.enter.usbHidUsage & 0xFFFF);
      expect(forwardingUsbHid(unknown, LogicalKeyboardKey.arrowLeft),
          PhysicalKeyboardKey.arrowLeft.usbHidUsage & 0xFFFF);
    });

    test('an unknown key with no fallback stays unknown', () {
      const unknown = PhysicalKeyboardKey(0x1500000000);
      expect(forwardingUsbHid(unknown, LogicalKeyboardKey.f13), 0);
    });
  });
}

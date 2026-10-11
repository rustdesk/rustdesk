import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/mobile_text_input_utils.dart';

void main() {
  group('mobileSoftKeyboardCommitUsesSeq', () {
    test('single CJK character commit uses the seq text channel', () {
      expect(mobileSoftKeyboardCommitUsesSeq('你', hasModifierOn: false),
          isTrue);
      expect(mobileSoftKeyboardCommitUsesSeq('你好', hasModifierOn: false),
          isTrue);
    });

    test('toolbar modifiers keep keystrokes on the key-event path', () {
      expect(mobileSoftKeyboardCommitUsesSeq('c', hasModifierOn: true), isFalse);
      expect(mobileSoftKeyboardCommitUsesSeq('你', hasModifierOn: true), isFalse);
    });

    test('multi-character commits use seq even with a modifier held', () {
      expect(mobileSoftKeyboardCommitUsesSeq('你好', hasModifierOn: true),
          isTrue);
    });

    test('control characters stay key events; empty commits are not sent', () {
      expect(
          mobileSoftKeyboardCommitUsesSeq('\n', hasModifierOn: false), isFalse);
      expect(
          mobileSoftKeyboardCommitUsesSeq(' ', hasModifierOn: false), isFalse);
      expect(mobileSoftKeyboardCommitUsesSeq('', hasModifierOn: false), isFalse);
    });
  });
}

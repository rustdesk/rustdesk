import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/mobile_text_input_utils.dart';

void main() {
  group('mobileSoftKeyboardCommitUsesSeq', () {
    test('single CJK character commit uses the seq text channel', () {
      expect(
          mobileSoftKeyboardCommitUsesSeq('你', hasModifierOn: false), isTrue);
      expect(
          mobileSoftKeyboardCommitUsesSeq('你好', hasModifierOn: false), isTrue);
    });

    test('toolbar modifiers keep keystrokes on the key-event path', () {
      expect(
          mobileSoftKeyboardCommitUsesSeq('c', hasModifierOn: true), isFalse);
      expect(
          mobileSoftKeyboardCommitUsesSeq('你', hasModifierOn: true), isFalse);
    });

    test('multi-character commits use seq even with a modifier held', () {
      expect(
          mobileSoftKeyboardCommitUsesSeq('你好', hasModifierOn: true), isTrue);
    });

    test('control characters stay key events; empty commits are not sent', () {
      expect(
          mobileSoftKeyboardCommitUsesSeq('\n', hasModifierOn: false), isFalse);
      expect(
          mobileSoftKeyboardCommitUsesSeq(' ', hasModifierOn: false), isFalse);
      expect(
          mobileSoftKeyboardCommitUsesSeq('', hasModifierOn: false), isFalse);
    });
  });

  group('diffMobileSoftKeyboard', () {
    const sentinel = '111';

    MobileSoftKeyboardUpdate step(
      String tracked,
      String current, {
      TextRange composing = TextRange.empty,
      bool hasModifierOn = false,
    }) {
      return diffMobileSoftKeyboard(
        trackedText: tracked,
        currentText: current,
        composing: composing,
        hasModifierOn: hasModifierOn,
      );
    }

    test('holds Pinyin letters and sends the confirmed character once', () {
      final n = step(sentinel, '${sentinel}n',
          composing: const TextRange(start: 3, end: 4));
      expect(n.actions, isEmpty);
      expect(n.trackedText, sentinel);

      final ni = step(n.trackedText, '${sentinel}ni',
          composing: const TextRange(start: 3, end: 5));
      expect(ni.actions, isEmpty);

      final committed = step(ni.trackedText, '${sentinel}你');
      expect(committed.actions, [const MobileSoftKeyboardAction.text('你')]);
      expect(committed.trackedText, '${sentinel}你');
    });

    test('sends Han text while a stale composing range still covers it', () {
      final update = step(
        sentinel,
        '${sentinel}你',
        composing: const TextRange(start: 3, end: 4),
      );
      expect(update.actions, [const MobileSoftKeyboardAction.text('你')]);
    });

    test('replaces in-progress Han text when the confirmed character arrives',
        () {
      final strokes = step(
        sentinel,
        '$sentinel一丨',
        composing: const TextRange(start: 3, end: 5),
      );
      expect(strokes.actions, [const MobileSoftKeyboardAction.text('一丨')]);

      final committed = step(strokes.trackedText, '${sentinel}中');
      expect(committed.actions, [
        MobileSoftKeyboardAction.backspace,
        MobileSoftKeyboardAction.backspace,
        const MobileSoftKeyboardAction.text('中'),
      ]);
    });

    test(
        'sends a handwriting character and replaces it when recognition changes',
        () {
      final first = step(
        sentinel,
        '${sentinel}中',
        composing: const TextRange(start: 3, end: 4),
      );
      expect(first.actions, [const MobileSoftKeyboardAction.text('中')]);

      final replaced = step(
        first.trackedText,
        '${sentinel}国',
        composing: const TextRange(start: 3, end: 4),
      );
      expect(replaced.actions, [
        MobileSoftKeyboardAction.backspace,
        const MobileSoftKeyboardAction.text('国'),
      ]);
    });

    test(
        'sends ASCII one character at a time and keeps space and return as keys',
        () {
      final a = step(sentinel, '${sentinel}a');
      expect(a.actions, [const MobileSoftKeyboardAction.text('a')]);

      final space = step(a.trackedText, '${a.trackedText} ');
      expect(space.actions, [const MobileSoftKeyboardAction.key(' ')]);

      final newline = step(space.trackedText, '${space.trackedText}\n');
      expect(newline.actions, [const MobileSoftKeyboardAction.key('\n')]);
    });

    test('modifier plus a letter stays a key event', () {
      final update = step(sentinel, '${sentinel}c', hasModifierOn: true);
      expect(update.actions, [const MobileSoftKeyboardAction.key('c')]);
    });

    test(
        'backspace of a committed character and of the sentinel each delete one',
        () {
      final typed = step(sentinel, '${sentinel}你');
      final deleted = step(typed.trackedText, sentinel);
      expect(deleted.actions, [MobileSoftKeyboardAction.backspace]);

      final sentinelDeleted = step(deleted.trackedText, '11');
      expect(sentinelDeleted.actions, [MobileSoftKeyboardAction.backspace]);
      expect(sentinelDeleted.trackedText, '11');
    });

    test('pasted text is one seq commit', () {
      final update = step(sentinel, '${sentinel}你好世界');
      expect(update.actions, [const MobileSoftKeyboardAction.text('你好世界')]);
    });
  });
}

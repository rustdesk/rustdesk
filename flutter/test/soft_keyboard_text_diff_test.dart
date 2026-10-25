import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';

void main() {
  group('softKeyboardTextDiff', () {
    test('detects same-length in-place IME edits', () {
      // Vietnamese Telex replaces "oo" with "ô" without changing the length,
      // which a length-only diff misses.
      final diff = softKeyboardTextDiff('$initText' 'oo', '$initText' 'ô');
      expect(diff.backspaces, 2);
      expect(diff.newStr, 'ô');
    });

    test('appends and deletes plain text', () {
      final append = softKeyboardTextDiff(initText, '$initText' 'a');
      expect(append.backspaces, 0);
      expect(append.newStr, 'a');

      final delete = softKeyboardTextDiff('$initText' 'ab', '$initText' 'a');
      expect(delete.backspaces, 1);
      expect(delete.newStr, isEmpty);
    });

    test('does not split surrogate pairs', () {
      // Deleting an emoji removes one character, not two UTF-16 code units.
      final delete = softKeyboardTextDiff('$initText' 'a😀', '$initText' 'a');
      expect(delete.backspaces, 1);
      expect(delete.newStr, isEmpty);

      // "a😀" -> "a😃" shares the 'a' and a high surrogate; the common prefix
      // must not split the pair.
      final replace = softKeyboardTextDiff('$initText' 'a😀', '$initText' 'a😃');
      expect(replace.backspaces, 1);
      expect(replace.newStr, '😃');
    });

    test('distinguishes typed 1 from padding', () {
      // A '1' the user typed must not be mistaken for the padding marker.
      final diff =
          softKeyboardTextDiff('$initText' 'a1', '$initText' 'b1');
      expect(diff.backspaces, 2);
      expect(diff.newStr, 'b1');

      // Deleting past the typed text consumes padding as backspace credit.
      final consumed =
          softKeyboardTextDiff(initText, '1' * (initText.length - 1));
      expect(consumed.backspaces, 1);
      expect(consumed.newStr, isEmpty);

      // Clearing the whole field deletes only the typed text, not padding.
      final cleared = softKeyboardTextDiff('$initText' 'a', '');
      expect(cleared.backspaces, 1);
      expect(cleared.newStr, isEmpty);
    });
  });
}

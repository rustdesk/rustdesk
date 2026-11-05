import 'package:flutter_hbb/common/whitelist_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('whitelistInputEntries', () {
    test('splits at the separators the dialogs accept', () {
      expect(
          whitelistInputEntries(
              ' 10.0.0.1, 10.0.0.2;10.0.0.0/24\n192.168.1.1 '),
          ['10.0.0.1', '10.0.0.2', '10.0.0.0/24', '192.168.1.1']);
      expect(whitelistInputEntries('10.0.0.1,'), ['10.0.0.1']);
      expect(
          whitelistInputEntries('123456789; 98765*'), ['123456789', '98765*']);
    });

    test('empty input clears the whitelist', () {
      expect(whitelistInputEntries(''), isEmpty);
      expect(whitelistInputEntries('  \n '), isEmpty);
    });

    test('separator-only input is rejected instead of clearing the whitelist',
        () {
      for (final input in [',', ';', ' , ', ',,;', ' ;\n, ']) {
        expect(whitelistInputEntries(input), isNull, reason: input);
      }
    });
  });

  group('whitelist dialog round trip', () {
    // The stored values of a cleared whitelist: ',' for a custom client, else ''.
    const clearedValues = [',', ''];

    test(
        'keepsStoredWhitelist keeps an unchanged whitelist that lets no one in',
        () {
      for (final cleared in clearedValues) {
        for (final stored in [' , ', ';', ' ', ' ;\n, ', ',,;']) {
          final shown = whitelistDialogText(stored, cleared);
          expect(whitelistLetsNoOneIn(stored), isTrue, reason: stored);
          // The field holds blank lines or separators; trimmed, it would be
          // empty or rejected, and saving it would lift the restriction.
          expect(keepsStoredWhitelist(stored, shown, shown), isTrue,
              reason: stored);
        }
      }
    });

    test('clearing or editing the field is saved as before', () {
      const stored = ' , ';
      final shown = whitelistDialogText(stored, ',');
      expect(shown, ' \n ');
      expect(keepsStoredWhitelist(stored, shown, ''), isFalse);
      expect(keepsStoredWhitelist(stored, shown, '10.0.0.1'), isFalse);
    });

    test('a whitelist that is off or names entries is saved as before', () {
      for (final cleared in clearedValues) {
        for (final stored in ['', ',', ',,', '10.0.0.1,10.0.0.0/24', '123*']) {
          final shown = whitelistDialogText(stored, cleared);
          expect(whitelistLetsNoOneIn(stored), isFalse, reason: stored);
          expect(keepsStoredWhitelist(stored, shown, shown), isFalse,
              reason: stored);
        }
      }
      expect(
          whitelistDialogText('10.0.0.1,10.0.0.2', ','), '10.0.0.1\n10.0.0.2');
      expect(whitelistDialogText(',', ','), '');
    });
  });
}

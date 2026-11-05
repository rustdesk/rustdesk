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
      expect(whitelistInputEntries('123456789; 98765*'),
          ['123456789', '98765*']);
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
}

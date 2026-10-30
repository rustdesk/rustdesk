import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('formatID', () {
    test('groups typed IDs from the right', () {
      expect(formatID('53464938'), '53 464 938');
      expect(formatID('123456789'), '123 456 789');
      expect(formatID('1234'), '1 234');
    });

    test('ignores line breaks, tabs and non-breaking spaces from a paste', () {
      expect(formatID('53464938\r\n'), '53 464 938');
      expect(formatID('53464938\n'), '53 464 938');
      expect(formatID('\t53464938'), '53 464 938');
      expect(formatID('53\u00a0464\u00a0938'), '53 464 938');
    });

    test('keeps the relay suffix', () {
      expect(formatID(r'53464938\r'), r'53 464 938\r');
    });

    test('leaves non-numeric input alone', () {
      expect(formatID('my-host.example.com'), 'my-host.example.com');
    });
  });

  test('trimID drops all whitespace', () {
    expect(trimID('53 464 938\r\n'), '53464938');
    expect(trimID('53\u00a0464\u00a0938'), '53464938');
  });

  test('pasting an ID with a trailing newline formats it like typing', () {
    final formatted = IDTextInputFormatter().formatEditUpdate(
      TextEditingValue.empty,
      const TextEditingValue(
        text: '53464938\r\n',
        selection: TextSelection.collapsed(offset: 10),
      ),
    );
    expect(formatted.text, '53 464 938');
  });
}

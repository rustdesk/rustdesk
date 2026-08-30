import 'package:xterm/src/core/input/keys.dart';
import 'package:xterm/src/core/input/keytab/keytab_default.dart';
import 'package:xterm/src/core/input/keytab/keytab_parse.dart';
import 'package:xterm/src/core/input/keytab/keytab_record.dart';
import 'package:xterm/src/core/input/keytab/keytab_token.dart';

class Keytab {
  Keytab({
    required this.name,
    required this.records,
  });

  factory Keytab.parse(String source) {
    final tokens = tokenize(source).toList();
    final parser = KeytabParser()..addTokens(tokens);
    return parser.result;
  }

  static final defaultKeytab = Keytab.parse(kDefaultKeytab);

  final String? name;

  final List<KeytabRecord> records;

  KeytabRecord? find(
    TerminalKey key, {
    bool ctrl = false,
    bool alt = false,
    bool shift = false,
    bool newLineMode = false,
    bool appCursorKeys = false,
    bool appKeyPad = false,
    bool keyPad = false,
    bool appScreen = false,
    bool macos = false,
    // bool meta,
  }) {
    for (var record in records) {
      if (record.key != key) {
        continue;
      }

      if (record.anyModifier == true) {
        if (ctrl == false && alt == false && shift == false) {
          continue;
        }
      } else if (record.anyModifier == false) {
        if (ctrl != false || alt != false || shift != false) {
          continue;
        }
      } else {
        if (record.ctrl != null && record.ctrl != ctrl) {
          continue;
        }

        if (record.shift != null && record.shift != shift) {
          continue;
        }

        if (record.alt != null && record.alt != alt) {
          continue;
        }
      }

      if (record.newLine != null && record.newLine != newLineMode) {
        continue;
      }

      if (record.appCursorKeys != null &&
          record.appCursorKeys != appCursorKeys) {
        continue;
      }

      if (record.appKeyPad != null && record.appKeyPad != appKeyPad) {
        continue;
      }

      if (record.keyPad != null && record.keyPad != keyPad) {
        continue;
      }

      if (record.appScreen != null && record.appScreen != appScreen) {
        continue;
      }

      if (record.macos != null && record.macos != macos) {
        continue;
      }

      // TODO: support VT52
      if (record.ansi == false) {
        continue;
      }

      return record;
    }

    return null;
  }

  @override
  String toString() {
    final buffer = StringBuffer();

    buffer.writeln('keyboard "$name"');

    for (var record in records) {
      buffer.writeln(record);
    }

    return buffer.toString();
  }
}

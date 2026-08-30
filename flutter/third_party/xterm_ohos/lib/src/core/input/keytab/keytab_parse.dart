import 'package:xterm/src/core/input/keytab/keytab.dart';
import 'package:xterm/src/core/input/keytab/keytab_record.dart';
import 'package:xterm/src/core/input/keytab/keytab_token.dart';
import 'package:xterm/src/core/input/keytab/qt_keyname.dart';

class ParseError {}

class TokensReader {
  TokensReader(this.tokens);

  final List<KeytabToken> tokens;

  var _pos = 0;

  bool get done => _pos > tokens.length - 1;

  KeytabToken? take() {
    final result = peek();
    _pos += 1;
    return result;
  }

  KeytabToken? peek() {
    if (done) return null;
    return tokens[_pos];
  }
}

class KeytabParser {
  String? _name;
  final _records = <KeytabRecord>[];

  void addTokens(List<KeytabToken> tokens) {
    final reader = TokensReader(tokens);

    while (!reader.done) {
      if (reader.peek()!.type == KeytabTokenType.keyboard) {
        _parseName(reader);
        continue;
      }

      if (reader.peek()!.type == KeytabTokenType.keyDefine) {
        _parseKeyDefine(reader);
        continue;
      }

      throw ParseError();
    }
  }

  Keytab get result {
    return Keytab(name: _name, records: _records);
  }

  void _parseName(TokensReader reader) {
    if (reader.take()!.type != KeytabTokenType.keyboard) {
      throw ParseError();
    }

    final name = reader.take()!;
    if (name.type != KeytabTokenType.input) {
      throw ParseError();
    }

    _name = name.value;
  }

  void _parseKeyDefine(TokensReader reader) {
    if (reader.take()!.type != KeytabTokenType.keyDefine) {
      throw ParseError();
    }

    final keyName = reader.take()!;

    if (keyName.type != KeytabTokenType.keyName) {
      throw ParseError();
    }

    final key = qtKeynameMap[keyName.value];
    if (key == null) {
      throw ParseError();
    }

    bool? alt;
    bool? ctrl;
    bool? shift;
    bool? anyModifier;
    bool? ansi;
    bool? appScreen;
    bool? keyPad;
    bool? appCursorKeys;
    bool? appKeyPad;
    bool? newLine;
    bool? mac;

    while (reader.peek()!.type == KeytabTokenType.modeStatus) {
      bool modeStatus;
      switch (reader.take()!.value) {
        case '+':
          modeStatus = true;
          break;
        case '-':
          modeStatus = false;
          break;
        default:
          throw ParseError();
      }

      final mode = reader.take();
      if (mode!.type != KeytabTokenType.mode) {
        throw ParseError();
      }

      switch (mode.value) {
        case 'Alt':
          alt = modeStatus;
          break;
        case 'Control':
          ctrl = modeStatus;
          break;
        case 'Shift':
          shift = modeStatus;
          break;
        case 'AnyMod':
          anyModifier = modeStatus;
          break;
        case 'Ansi':
          ansi = modeStatus;
          break;
        case 'AppScreen':
          appScreen = modeStatus;
          break;
        case 'KeyPad':
          keyPad = modeStatus;
          break;
        case 'AppCuKeys':
          appCursorKeys = modeStatus;
          break;
        case 'AppKeyPad':
          appKeyPad = modeStatus;
          break;
        case 'NewLine':
          newLine = modeStatus;
          break;
        case 'Mac':
          mac = modeStatus;
          break;
        default:
          throw ParseError();
      }
    }

    if (reader.take()!.type != KeytabTokenType.colon) {
      throw ParseError();
    }

    final actionToken = reader.take()!;
    KeytabAction action;
    if (actionToken.type == KeytabTokenType.input) {
      action = KeytabAction(KeytabActionType.input, actionToken.value);
    } else if (actionToken.type == KeytabTokenType.shortcut) {
      action = KeytabAction(KeytabActionType.shortcut, actionToken.value);
    } else {
      throw ParseError();
    }

    final record = KeytabRecord(
      qtKeyName: keyName.value,
      key: key,
      action: action,
      alt: alt,
      ctrl: ctrl,
      shift: shift,
      anyModifier: anyModifier,
      ansi: ansi,
      appScreen: appScreen,
      keyPad: keyPad,
      appCursorKeys: appCursorKeys,
      appKeyPad: appKeyPad,
      newLine: newLine,
      macos: mac,
    );

    _records.add(record);
  }
}

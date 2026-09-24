import 'package:flutter/widgets.dart';

({int backspaces, String text}) getNonIOSSoftKeyboardEdit(
    String oldValue, String newValue) {
  final oldCharacters = oldValue.characters.toList(growable: false);
  final newCharacters = newValue.characters.toList(growable: false);
  var common = 0;
  while (common < oldCharacters.length &&
      common < newCharacters.length &&
      oldCharacters[common] == newCharacters[common]) {
    common++;
  }
  return (
    backspaces: oldCharacters.length - common,
    text: newCharacters.skip(common).join(),
  );
}

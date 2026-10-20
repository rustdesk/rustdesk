import 'package:flutter/services.dart';

/// Decides how a soft-keyboard committed string is delivered to the peer.
///
/// Multi-character commits always use the KeyEvent.seq text channel.
/// CJK IMEs usually commit one character at a time, and the raw-key path
/// cannot carry that text, so a single committed character also uses seq.
/// `'\n'` and `' '` keep their VK_RETURN / VK_SPACE mapping. A toolbar
/// modifier (Ctrl/Alt/Shift/Command) also stays on the key-event path so
/// shortcuts such as Ctrl+C keep their modifier flags.
bool mobileSoftKeyboardCommitUsesSeq(String committed,
    {required bool hasModifierOn}) {
  if (committed.isEmpty) return false;
  if (committed == '\n' || committed == ' ') return false;
  if (committed.length > 1) return true;
  return !hasModifierOn;
}

/// Active phonetic composition (Pinyin, Zhuyin, Kana, Jamo) must not be
/// forwarded. Han text inside a still-valid composing range is a confirm or a
/// handwriting result whose mark has not collapsed yet, and has to be sent
/// immediately. Holding that Han text drops characters such as “一” when iOS
/// never emits a second event after the range clears.
bool shouldHoldMobileComposingText(String composingText) {
  if (composingText.isEmpty) return false;
  return !RegExp(r'[\u3400-\u4DBF\u4E00-\u9FFF]').hasMatch(composingText);
}

/// Removes an in-progress composition from [currentText]. Confirmed Han text
/// is left in place even if the composing range still covers it.
String stableTextForMobileSoftKeyboard(
    String currentText, TextRange composing) {
  if (!composing.isValid || composing.isCollapsed) return currentText;
  if (composing.start > composing.end || composing.end > currentText.length) {
    return currentText;
  }
  final slice = currentText.substring(composing.start, composing.end);
  if (!shouldHoldMobileComposingText(slice)) return currentText;
  return currentText.replaceRange(composing.start, composing.end, '');
}

enum MobileSoftKeyboardActionType { backspace, text, key }

class MobileSoftKeyboardAction {
  const MobileSoftKeyboardAction._(this.type, this.value);

  final MobileSoftKeyboardActionType type;
  final String value;

  static const backspace = MobileSoftKeyboardAction._(
    MobileSoftKeyboardActionType.backspace,
    'VK_BACK',
  );

  const MobileSoftKeyboardAction.text(String text)
      : this._(MobileSoftKeyboardActionType.text, text);

  const MobileSoftKeyboardAction.key(String key)
      : this._(MobileSoftKeyboardActionType.key, key);

  @override
  bool operator ==(Object other) {
    return other is MobileSoftKeyboardAction &&
        other.type == type &&
        other.value == value;
  }

  @override
  int get hashCode => Object.hash(type, value);
}

class MobileSoftKeyboardUpdate {
  const MobileSoftKeyboardUpdate({
    required this.trackedText,
    required this.actions,
  });

  /// Controller text with any still-held composition removed. This is the
  /// baseline for the next diff, so Pinyin letters are not sent later as a
  /// second commit.
  final String trackedText;
  final List<MobileSoftKeyboardAction> actions;
}

/// Diffs the hidden soft-keyboard buffer.
///
/// The buffer keeps a run of sentinel `1`s so backspace still produces a text
/// change after the real tail is empty. Only the suffix after the last `1`
/// that is shared by both snapshots is treated as user text.
MobileSoftKeyboardUpdate diffMobileSoftKeyboard({
  required String trackedText,
  required String currentText,
  required TextRange composing,
  required bool hasModifierOn,
}) {
  final stable = stableTextForMobileSoftKeyboard(currentText, composing);
  if (stable == trackedText) {
    return MobileSoftKeyboardUpdate(
      trackedText: trackedText,
      actions: const [],
    );
  }
  return MobileSoftKeyboardUpdate(
    trackedText: stable,
    actions: _actionsForTailDiff(trackedText, stable, hasModifierOn),
  );
}

List<MobileSoftKeyboardAction> _actionsForTailDiff(
  String previous,
  String current,
  bool hasModifierOn,
) {
  var i = current.length - 1;
  for (; i >= 0 && current[i] != '1'; --i) {}
  var j = previous.length - 1;
  for (; j >= 0 && previous[j] != '1'; --j) {}
  if (i < j) j = i;
  final subNew = current.substring(j + 1);
  final subOld = previous.substring(j + 1);

  var common = 0;
  for (;
      common < subOld.length &&
          common < subNew.length &&
          subNew[common] == subOld[common];
      ++common) {}

  final actions = <MobileSoftKeyboardAction>[];
  for (var k = 0; k < subOld.length - common; ++k) {
    actions.add(MobileSoftKeyboardAction.backspace);
  }
  if (subNew.length <= common) return actions;

  final inserted = subNew.substring(common);
  if (mobileSoftKeyboardCommitUsesSeq(inserted, hasModifierOn: hasModifierOn)) {
    actions.add(MobileSoftKeyboardAction.text(inserted));
  } else if (inserted.isNotEmpty) {
    actions.add(MobileSoftKeyboardAction.key(inserted));
  }
  return actions;
}

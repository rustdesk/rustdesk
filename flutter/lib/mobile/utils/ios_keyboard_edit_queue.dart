import 'ios_soft_keyboard_input.dart';

class IosKeyboardEditQueue {
  Future<void> _pending = Future.value();

  // FRB runs normal calls on multiple workers. Wait for identical Backspaces,
  // then their replacement text, before starting another edit. A failed send
  // also fails later edits: their diffs assume the earlier text reached the peer.
  // The caller reports failures. Leaving/reopening the remote page creates a
  // fresh queue; reconnecting in place or reopening the keyboard does not.
  Future<void> send(IosKeyboardEdit edit,
      {required Future<void> Function(String) sendKey,
      required Future<void> Function(String) sendText}) {
    if (edit.backspaces == 0 && edit.text.isEmpty) return Future.value();
    return enqueue(() async {
      await Future.wait(
          List.generate(edit.backspaces, (_) => sendKey('VK_BACK')));
      if (edit.text.length > 1) {
        await sendText(edit.text);
      } else if (edit.text.isNotEmpty) {
        final key = edit.text == '\n'
            ? 'VK_RETURN'
            : edit.text == ' '
                ? 'VK_SPACE'
                : edit.text;
        await sendKey(key);
      }
    });
  }

  // Toolbar keys must follow pending edits and precede any subsequent typing.
  Future<void> enqueue(Future<void> Function() send) =>
      _pending = _pending.then((_) => send());
}

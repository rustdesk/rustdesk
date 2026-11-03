import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

// Normal Flutter/Rust bridge calls run on separate workers. A replacement
// must finish enqueueing its Delete before the following Unicode text starts.
class AndroidKeyboardQueue {
  final ValueChanged<Object> onError;
  Future<void> _tail = Future<void>.value();
  bool _closed = false;
  bool _failed = false;

  AndroidKeyboardQueue({required this.onError});

  Future<void> add(Future<void> Function() send) {
    _tail = _tail.then((_) async {
      if (_closed || _failed) return;
      try {
        await send();
      } catch (error) {
        _failed = true;
        onError(error);
      }
    });
    return _tail;
  }

  void close() => _closed = true;
}

class AndroidSoftKeyboardEdit {
  final int deleteCount;
  final String insertedText;

  const AndroidSoftKeyboardEdit(this.deleteCount, this.insertedText);
}

AndroidSoftKeyboardEdit computeAndroidSoftKeyboardEdit(
    String previous, String current) {
  final before = previous.characters.toList();
  final after = current.characters.toList();
  var common = 0;
  while (common < before.length &&
      common < after.length &&
      before[common] == after[common]) {
    common++;
  }
  return AndroidSoftKeyboardEdit(
      before.length - common, after.skip(common).join());
}

// This editor owns an empty buffer. The iOS sentinel controller must never be
// attached to Android: an IME can select, compose, or replay its filler text.
class AndroidSoftKeyboardInput extends StatefulWidget {
  final FocusNode? focusNode;
  final ValueChanged<String> onText;
  final VoidCallback onBackspace;
  final VoidCallback onEnter;

  const AndroidSoftKeyboardInput({
    super.key,
    this.focusNode,
    required this.onText,
    required this.onBackspace,
    required this.onEnter,
  });

  @override
  State<AndroidSoftKeyboardInput> createState() =>
      _AndroidSoftKeyboardInputState();
}

class _AndroidSoftKeyboardInputState extends State<AndroidSoftKeyboardInput> {
  final _controller = TextEditingController();
  String _previous = '';

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onEdit);
  }

  @override
  void dispose() {
    _controller.removeListener(_onEdit);
    _controller.dispose();
    super.dispose();
  }

  void _onEdit() {
    final current = _controller.text;
    if (current == _previous) return;
    final edit = computeAndroidSoftKeyboardEdit(_previous, current);
    _previous = current;
    for (var i = 0; i < edit.deleteCount; i++) {
      widget.onBackspace();
    }
    final lines = edit.insertedText.split('\n');
    for (var i = 0; i < lines.length; i++) {
      if (i > 0) widget.onEnter();
      if (lines[i].isNotEmpty) widget.onText(lines[i]);
    }
  }

  void _backspace() {
    final value = _controller.value;
    if (value.text.isEmpty) {
      // Android emits raw Delete when there is no local text. It must still
      // delete text that was already present on the remote PC.
      widget.onBackspace();
      return;
    }
    final selection = value.selection;
    var start = selection.isValid ? selection.start : value.text.length;
    final end = selection.isValid ? selection.end : value.text.length;
    if (start == end) {
      if (start == 0) return;
      start -= value.text.substring(0, start).characters.last.length;
    }
    // Updating the same editor drives one diff and keeps the IME buffer in
    // sync. Returning handled below prevents a second platform deletion.
    _controller.value = TextEditingValue(
        text: value.text.replaceRange(start, end, ''),
        selection: TextSelection.collapsed(offset: start));
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    final down = event is KeyDownEvent || event is KeyRepeatEvent;
    if (event.logicalKey == LogicalKeyboardKey.backspace) {
      if (down) _backspace();
      return KeyEventResult.handled;
    }
    // Let Android turn Enter into an editor newline. Send it only from
    // _onEdit, and retain the buffer so a later full IME commit shares the
    // same prefix instead of replaying the entire paragraph.
    return KeyEventResult.skipRemainingHandlers;
  }

  @override
  Widget build(BuildContext context) => Focus(
      onKeyEvent: _onKeyEvent,
      child: TextField(
        controller: _controller,
        focusNode: widget.focusNode,
        autofocus: true,
        autocorrect: false,
        enableSuggestions: false,
        maxLines: null,
        keyboardType: TextInputType.multiline,
        textInputAction: TextInputAction.newline,
      ));
}

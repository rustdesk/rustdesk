import 'package:flutter/material.dart';

import 'input_bridge.dart';
import '../strip/models/modifier_state.dart';

// Pre-filled buffer of '1' characters — same trick as upstream RustDesk's
// mobile keyboard. Keeping the field non-empty lets iOS backspace always
// produce a delta (shorter string) rather than a no-op on an empty field,
// and gives a stable anchor for the old/new diff.
final _kInitText = '1' * 1024;

class TextFieldBridge extends StatefulWidget {
  final InputBridge inputBridge;
  final ModifierController modifierController;
  final FocusNode focusNode;

  const TextFieldBridge({
    super.key,
    required this.inputBridge,
    required this.modifierController,
    required this.focusNode,
  });

  @override
  State<TextFieldBridge> createState() => _TextFieldBridgeState();
}

class _TextFieldBridgeState extends State<TextFieldBridge> {
  late final TextEditingController _controller;
  String _value = '';

  @override
  void initState() {
    super.initState();
    _value = _kInitText;
    _controller = TextEditingController(text: _kInitText);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  // Ported from upstream RustDesk _handleIOSSoftKeyboardInput.
  // Diffs newValue against _value to find what was added/removed,
  // then sends only the delta to the remote. This correctly handles
  // autocorrect, swipe typing, and suggestion-bar taps including
  // the trailing space iOS appends after word completion.
  void _onChanged(String newValue) {
    final oldValue = _value;
    _value = newValue;

    // Find the last '1' anchor in each string.
    var i = newValue.length - 1;
    for (; i >= 0 && newValue[i] != '1'; --i) {}
    var j = oldValue.length - 1;
    for (; j >= 0 && oldValue[j] != '1'; --j) {}
    if (i < j) j = i;

    final subNew = newValue.substring(j + 1);
    final subOld = oldValue.substring(j + 1);

    // Find common prefix between the two suffixes.
    var common = 0;
    for (;
        common < subOld.length &&
            common < subNew.length &&
            subNew[common] == subOld[common];
        ++common) {}

    final newStr = subNew.length > common ? subNew.substring(common) : '';

    // If still composing and new string is shorter than composing range, ignore.
    if (_controller.value.isComposingRangeValid) {
      final composingLength = _controller.value.composing.end -
          _controller.value.composing.start;
      if (composingLength > newStr.length) {
        _value = oldValue;
        return;
      }
    }

    // Send backspaces for deleted chars.
    for (var k = 0; k < subOld.length - common; ++k) {
      widget.inputBridge.tapKey('backspace');
    }

    // Send new chars — check for modifier combos first.
    if (newStr.isEmpty) return;

    // iOS multiline keyboard delivers Return as a '\n' via onChanged.
    if (newStr == '\n') {
      widget.inputBridge.tapKey('return');
      _resetBuffer();
      return;
    }

    final mods = widget.modifierController.heldModifiers;
    if (mods.isNotEmpty && newStr.length == 1) {
      widget.inputBridge.tapKey(newStr.toLowerCase(), modifiers: mods);
      widget.modifierController.releaseOneShot();
    } else if (newStr.length > 1) {
      widget.inputBridge.typeString(newStr);
    } else {
      widget.inputBridge.typeString(newStr);
    }
  }

  void _resetBuffer() {
    _value = _kInitText;
    _controller.text = _kInitText;
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 1,
      height: 1,
      child: Opacity(
        opacity: 0,
        child: TextField(
          controller: _controller,
          focusNode: widget.focusNode,
          autofocus: false,
          enableInteractiveSelection: false,
          autocorrect: false,
          enableSuggestions: false,
          maxLines: null,
          keyboardType: TextInputType.multiline,
          textInputAction: TextInputAction.newline,
          onChanged: _onChanged,
          onSubmitted: (_) {
            widget.inputBridge.tapKey('return');
            _resetBuffer();
            widget.focusNode.requestFocus();
          },
          decoration: const InputDecoration(border: InputBorder.none),
        ),
      ),
    );
  }
}

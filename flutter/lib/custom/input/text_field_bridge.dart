import 'package:flutter/material.dart';

import 'input_bridge.dart';
import '../strip/models/modifier_state.dart';

// Zero-width space used as the always-present sentinel character.
// This keeps the field non-empty so backspace is always detectable
// as a delta (sentinel → empty) rather than a no-op on an empty field.
const _kSentinel = '\u200B';
const _kSentinelValue = TextEditingValue(
  text: _kSentinel,
  selection: TextSelection.collapsed(offset: 1),
);

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
  final _controller = TextEditingController(text: _kSentinel);

  @override
  void initState() {
    super.initState();
    _controller.selection = const TextSelection.collapsed(offset: 1);
    _controller.addListener(_onChange);
  }

  @override
  void dispose() {
    _controller.removeListener(_onChange);
    _controller.dispose();
    super.dispose();
  }

  void _onChange() {
    final text = _controller.text;
    if (text == _kSentinel) return;

    if (text.isEmpty) {
      // User backspaced past the sentinel
      widget.inputBridge.tapKey('backspace');
      _reset();
      return;
    }

    final typed = text.replaceFirst(_kSentinel, '');
    if (typed.isEmpty) {
      _reset();
      return;
    }

    final mods = widget.modifierController.heldModifiers;
    if (mods.isNotEmpty && typed.length == 1) {
      // Modifier + single char → key event (e.g. ⌘C)
      widget.inputBridge.tapKeyWithModifiers(typed.toLowerCase(), mods);
      widget.modifierController.releaseOneShot();
    } else {
      // Plain text → string injection (handles Hebrew, emoji, IME)
      widget.inputBridge.typeString(typed);
    }
    _reset();
  }

  void _reset() {
    _controller.value = _kSentinelValue;
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
          textInputAction: TextInputAction.send,
          onSubmitted: (_) {
            widget.inputBridge.tapKey('return');
            widget.focusNode.requestFocus();
          },
          decoration: const InputDecoration(border: InputBorder.none),
        ),
      ),
    );
  }
}

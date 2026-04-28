import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';

import '../input/input_bridge.dart';
import '../input/scroll_gesture.dart';
import '../input/text_field_bridge.dart';
import '../strip/models/modifier_state.dart';
import '../strip/widgets/power_strip.dart';

class RemoteSessionScreen extends StatefulWidget {
  final String id;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;

  const RemoteSessionScreen({
    super.key,
    required this.id,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
  });

  @override
  State<RemoteSessionScreen> createState() => _RemoteSessionScreenState();
}

class _RemoteSessionScreenState extends State<RemoteSessionScreen> {
  late final InputBridge _bridge;
  late final ModifierController _modCtl;
  final _kbFocusNode = FocusNode();

  @override
  void initState() {
    super.initState();
    _bridge = InputBridge(gFFI.sessionId);
    _modCtl = ModifierController(_bridge);
  }

  @override
  void dispose() {
    _kbFocusNode.dispose();
    _modCtl.dispose();
    super.dispose();
  }

  void _onKeyboardTap() {
    if (_kbFocusNode.hasFocus) {
      _kbFocusNode.unfocus();
    } else {
      _kbFocusNode.requestFocus();
    }
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final keyboardHeight = mq.viewInsets.bottom;
    final isKeyboardOpen = keyboardHeight > 0;
    // When the iOS keyboard is closed, sit just above the home indicator;
    // when open, sit just above the keyboard (no home indicator below it).
    final stripBottom = isKeyboardOpen ? keyboardHeight : mq.viewPadding.bottom;

    return Stack(
      children: [
        // Layer 0: upstream RemotePage — owns canvas, connection lifecycle,
        // and all existing mobile gestures. KeyHelpTools and the
        // BottomAppBar are suppressed; PowerStrip replaces them.
        TwoFingerScrollDetector(
          inputBridge: _bridge,
          child: RemotePage(
            id: widget.id,
            password: widget.password,
            isSharedPassword: widget.isSharedPassword,
            forceRelay: widget.forceRelay,
            hideKeyHelpTools: true,
            hideBottomBar: true,
          ),
        ),

        // Layer 1: hidden 1×1 TextField — captures native iOS keyboard input
        // (letters, Hebrew, emoji, IME) and injects it to the remote.
        // Focused/unfocused via the ⌨ key in the PowerStrip.
        Positioned(
          left: 0,
          top: 0,
          child: TextFieldBridge(
            inputBridge: _bridge,
            modifierController: _modCtl,
            focusNode: _kbFocusNode,
          ),
        ),

        // Layer 2: power strip.
        Positioned(
          left: 0,
          right: 0,
          bottom: stripBottom,
          child: PowerStrip(
            inputBridge: _bridge,
            modifierController: _modCtl,
            onMacrosTap: _onMacrosTap,
            onKeyboardTap: _onKeyboardTap,
          ),
        ),
      ],
    );
  }

  void _onMacrosTap() {
    // Macro bottom sheet — Phase 3b
  }
}

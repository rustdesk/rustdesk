import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';

import '../chat/terminal_chat_overlay.dart';
import '../input/input_bridge.dart';
import '../input/text_field_bridge.dart';
import '../settings/settings_store.dart';
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
  final _modCtl = ModifierController();
  final _kbFocusNode = FocusNode();
  bool _chatOpen = false;
  double _stripHeight = 0;

  @override
  void initState() {
    super.initState();
    _bridge = InputBridge(gFFI.sessionId);
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

  void _onTwoFingerScroll(double dx, double dy) {
    _bridge.scroll(dx.round(), dy.round());
  }

  void _onDisconnect() {
    clientClose(gFFI.sessionId, gFFI);
  }

  void _onChatToggle() {
    setState(() => _chatOpen = !_chatOpen);
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final keyboardHeight = mq.viewInsets.bottom;
    final stripBottom = keyboardHeight > 0 ? keyboardHeight : mq.viewPadding.bottom;

    // Canvas bottom: reserve space for the strip (and keyboard when open) so
    // the remote screen is never occluded by the strip or system keyboard.
    final canvasBottom = _chatOpen
        ? mq.size.height * (1 - 0.55)
        : stripBottom + _stripHeight;

    // CursorPaint coordinates are relative to SafeArea (status bar excluded).
    // Offset it back down by the top padding so it aligns with the full-screen Stack.
    final safeAreaTop = mq.viewPadding.top;

    return Stack(
      clipBehavior: Clip.none,
      children: [
        // Layer 0: remote canvas — shrinks above strip and keyboard.
        // CursorPaint is suppressed inside RemotePage and hoisted to Layer 4
        // so the cursor can draw past the canvas boundary into the strip area.
        Positioned(
          top: 0,
          left: 0,
          right: 0,
          bottom: canvasBottom,
          child: _remoteCanvas(),
        ),

        // Layer 1: hidden 1×1 TextField for iOS keyboard input.
        Positioned(
          left: 0,
          top: 0,
          child: TextFieldBridge(
            inputBridge: _bridge,
            modifierController: _modCtl,
            focusNode: _kbFocusNode,
          ),
        ),

        // Layer 2: power strip — anchored above keyboard / home indicator.
        if (!_chatOpen)
          Positioned(
            left: 0,
            right: 0,
            bottom: stripBottom,
            child: _MeasureHeight(
              onChange: (h) {
                if (h != _stripHeight) setState(() => _stripHeight = h);
              },
              child: PowerStrip(
                inputBridge: _bridge,
                modifierController: _modCtl,
                onMacrosTap: _onMacrosTap,
                onKeyboardTap: _onKeyboardTap,
                onDisconnect: _onDisconnect,
                onChatToggle: _onChatToggle,
              ),
            ),
          ),

        // Layer 3: terminal chat overlay — slides up from bottom when open.
        if (_chatOpen)
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: TerminalChatOverlay(
              inputBridge: _bridge,
              startMaximized: settingsStore.chatStartMaximized,
              onClose: _onChatToggle,
            ),
          ),

        // Layer 4: cursor overlay — unconstrained so it can cross the
        // canvas/strip boundary without being clipped.
        // Offset by safeAreaTop because CursorModel coords are SafeArea-relative.
        Positioned.fill(
          top: safeAreaTop,
          child: CursorPaint(widget.id),
        ),
      ],
    );
  }

  Widget _remoteCanvas() => RemotePage(
        id: widget.id,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword,
        forceRelay: widget.forceRelay,
        hideKeyHelpTools: true,
        hideBottomBar: true,
        hideCursorPaint: true,
        onTwoFingerScroll: _onTwoFingerScroll,
      );

  void _onMacrosTap() {
    // Macro bottom sheet — Phase 3b
  }
}

// Reports its child's rendered height whenever the child's size changes,
// including when the child rebuilds internally (e.g. PowerStrip collapsing).
class _MeasureHeight extends StatefulWidget {
  final Widget child;
  final ValueChanged<double> onChange;

  const _MeasureHeight({required this.child, required this.onChange});

  @override
  State<_MeasureHeight> createState() => _MeasureHeightState();
}

class _MeasureHeightState extends State<_MeasureHeight> {
  final _key = GlobalKey();

  void _measure(_) {
    final box = _key.currentContext?.findRenderObject() as RenderBox?;
    if (box != null) widget.onChange(box.size.height);
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback(_measure);
    return NotificationListener<SizeChangedLayoutNotification>(
      onNotification: (_) {
        WidgetsBinding.instance.addPostFrameCallback(_measure);
        return true;
      },
      child: SizeChangedLayoutNotifier(
        child: KeyedSubtree(key: _key, child: widget.child),
      ),
    );
  }
}

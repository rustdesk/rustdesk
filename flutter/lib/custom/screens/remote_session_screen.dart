import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';

import '../chat/claude_session_indicator.dart';
import '../chat/terminal_chat_overlay.dart' show TerminalChatPartialBar, TerminalChatMaxView;
import '../theme/tokens.dart';
import '../input/input_bridge.dart';
import '../input/text_field_bridge.dart';
import '../overlay/floating_macro_bar.dart';
import '../strip/models/modifier_state.dart';
import '../strip/widgets/power_strip.dart';
import '../widgets/file_send_sheet.dart';
import 'package:flutter_hbb/custom/screens/connect_screen.dart';
import 'package:flutter_hbb/custom/session/session_registry.dart';
import 'package:flutter_hbb/custom/session/session_switcher_sheet.dart';

// Full-screen overlay state for this session, overrides the constrained
// BlockableOverlay set by RemotePage.applyFfi so dialogs span the screen.
class _FullScreenOverlayState extends OverlayKeyState {}

enum _ChatState { closed, partial, max }

class RemoteSessionScreen extends StatefulWidget {
  final String id;
  final FFI ffi;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;

  const RemoteSessionScreen({
    super.key,
    required this.id,
    required this.ffi,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
  });

  @override
  State<RemoteSessionScreen> createState() => _RemoteSessionScreenState();
}

class _RemoteSessionScreenState extends State<RemoteSessionScreen> {
  // Fallback Listenable used when terminalModels[0] is null, so AnimatedBuilder
  // always has a valid notifier without needing a null check at build time.
  static final _nullNotifier = ChangeNotifier();

  late final InputBridge _bridge;
  final _modCtl = ModifierController();
  final _kbFocusNode = FocusNode();
  _ChatState _chatState = _ChatState.closed;
  double _stripHeight = 0;
  double _partialBarHeight = 0;
  double _kbPanOffset = 0;
  late final StreamSubscription<bool> _kbVisibilitySub;
  final _fullScreenOverlayState = _FullScreenOverlayState();
  double _scrollAccumX = 0;
  double _scrollAccumY = 0;

  @override
  void initState() {
    super.initState();
    _bridge = InputBridge(widget.ffi.sessionId);
    _kbVisibilitySub = KeyboardVisibilityController()
        .onChange
        .listen(_onKeyboardVisibilityChanged);
    // Override dialogManager's overlay after RemotePage.applyFfi() runs.
    // RemotePage binds ffi.dialogManager to BlockableOverlay (constrained to
    // Positioned canvas area). We replace it with a full-screen overlay so
    // dialogs like the password prompt render centered over the whole screen.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.ffi.dialogManager.setOverlayState(_fullScreenOverlayState);
    });
  }

  @override
  void dispose() {
    _kbVisibilitySub.cancel();
    _kbFocusNode.dispose();
    _modCtl.dispose();
    super.dispose();
  }

  void _onKeyboardVisibilityChanged(bool visible) {
    // RemoteSessionScreen positions the canvas via Positioned(bottom: canvasBottom),
    // so the layout already reserves space for the keyboard without resizing the
    // canvas model. We save/restore the offset directly to preserve zoom level
    // without calling mobileFocusCanvasCursor(), which would call updateSize() with
    // the keyboard viewInsets and cause an unwanted zoom-out.
    if (visible) {
      widget.ffi.canvasModel.saveMobileOffsetBeforeSoftKeyboard();
      widget.ffi.canvasModel.isMobileCanvasChanged = false;
      final mq = MediaQuery.of(context);
      setState(() => _kbPanOffset = mq.viewInsets.bottom * 0.4);
    } else {
      widget.ffi.canvasModel.restoreMobileOffsetAfterSoftKeyboard();
      setState(() => _kbPanOffset = 0);
    }
  }

  void _onKeyboardTap() {
    if (_kbFocusNode.hasFocus) {
      _kbFocusNode.unfocus();
    } else {
      _kbFocusNode.requestFocus();
    }
  }

  void _onTwoFingerScroll(double dx, double dy) {
    _scrollAccumX += dx;
    _scrollAccumY += dy;
    final ix = _scrollAccumX.truncate();
    final iy = _scrollAccumY.truncate();
    if (ix != 0 || iy != 0) {
      _bridge.scroll(ix, iy);
      _scrollAccumX -= ix;
      _scrollAccumY -= iy;
    }
  }

  void _onDisconnect() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: const Color(0xFF1E293B),
        title: const Text('Disconnect', style: TextStyle(color: Colors.white)),
        content: Text(
          'Close connection to ${widget.id}?',
          style: const TextStyle(color: Color(0xFF94A3B8)),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Disconnect', style: TextStyle(color: Colors.red)),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    _doDisconnect();
  }

  void _doDisconnect() {
    final registry = SessionRegistry.instance;
    registry.unregister(widget.ffi.sessionId);
    widget.ffi.close();

    if (!mounted) return;
    if (registry.isEmpty) {
      Navigator.popUntil(context, ModalRoute.withName('/'));
      return;
    }
    final nextEntry = registry.findById(registry.activeSessionId!);
    if (nextEntry == null) {
      Navigator.popUntil(context, ModalRoute.withName('/'));
      return;
    }
    Navigator.pushReplacement(
      context,
      PageRouteBuilder(
        pageBuilder: (_, __, ___) => RemoteSessionScreen(
          id: nextEntry.peerId,
          ffi: nextEntry.ffi,
        ),
        transitionDuration: Duration.zero,
        reverseTransitionDuration: Duration.zero,
      ),
    );
  }

  void _onSessionSwitch(SessionID targetSessionId) {
    final registry = SessionRegistry.instance;
    final entry = registry.findById(targetSessionId);
    if (entry == null) return;
    registry.setActive(targetSessionId);
    Navigator.pushReplacement(
      context,
      PageRouteBuilder(
        pageBuilder: (_, __, ___) => RemoteSessionScreen(
          id: entry.peerId,
          ffi: entry.ffi,
        ),
        transitionDuration: Duration.zero,
        reverseTransitionDuration: Duration.zero,
      ),
    );
  }

  void _onAddSession() {
    if (SessionRegistry.instance.isFull) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Maximum 5 sessions reached.')),
      );
      return;
    }
    Navigator.push(context, MaterialPageRoute(builder: (_) => const ConnectScreen()));
  }

  void _onSessionsTap() {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: const Color(0xFF1E1E2E),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (ctx) => SessionSwitcherSheet(
        activeSessionId: widget.ffi.sessionId,
        onSwitch: _onSessionSwitch,
        onAddSession: _onAddSession,
      ),
    );
  }

  void _onChatToggle() {
    setState(() => _chatState = _chatState == _ChatState.closed
        ? _ChatState.partial
        : _ChatState.closed);
  }

  void _onChatMaximize() {
    setState(() => _chatState = _ChatState.max);
  }

  void _onChatMinimize() {
    setState(() => _chatState = _ChatState.partial);
  }

  void _onDisplaySwitch() {
    showOptions(context, widget.id, widget.ffi.dialogManager);
  }

  void _onNextDisplay() {
    final pi = widget.ffi.ffiModel.pi;
    final count = pi.displays.length;
    if (count <= 1) return;
    final next = (pi.currentDisplay + 1) % count;
    openMonitorInTheSameTab(next, widget.ffi, pi);
  }

  void _onZoomFit() {
    // Scale the remote canvas so its height exactly fills the canvas area
    // (from screen top to the top of the power strip / keyboard).
    final displayHeight = widget.ffi.canvasModel.getDisplayHeight();
    if (displayHeight <= 0) return;
    final mq = MediaQuery.of(context);
    final keyboardHeight = mq.viewInsets.bottom;
    final stripBottom = keyboardHeight > 0 ? keyboardHeight : mq.viewPadding.bottom;
    final chatBarHeight = _chatState == _ChatState.partial ? _partialBarHeight : 0.0;
    final canvasHeight = mq.size.height - mq.viewPadding.top - stripBottom - _stripHeight - chatBarHeight;
    if (canvasHeight <= 0) return;
    final targetScale = canvasHeight / displayHeight;
    final center = Offset(mq.size.width / 2, canvasHeight / 2);
    // updateScale takes a multiplier; divide target by current to get delta.
    final delta = targetScale / widget.ffi.canvasModel.scale;
    widget.ffi.canvasModel.updateScale(delta, center);
    widget.ffi.canvasModel.isMobileCanvasChanged = true;
  }

  void _onMouseModeToggle() {
    widget.ffi.ffiModel.toggleTouchMode();
  }

  Future<void> _onClipboardPaste() async {
    final data = await Clipboard.getData('text/plain');
    final text = data?.text;
    if (text != null && text.isNotEmpty) {
      await _bridge.typeString(text);
    }
  }

  void _onMacrosTap() {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: const Color(0xFF1E1E2E),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (ctx) => _MacroSheet(
        bridge: _bridge,
        ffi: widget.ffi,
        onZoomFit: _onZoomFit,
        onMouseModeToggle: _onMouseModeToggle,
        onClipboardPaste: _onClipboardPaste,
      ),
    );
  }

  void _onFileSend() {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: const Color(0xFF1C1C1E),
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (ctx) => FileSendSheet(ffi: widget.ffi),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final safeBottom = mq.viewPadding.bottom;
    final keyboardHeight = mq.viewInsets.bottom;
    final stripBottom = keyboardHeight > 0 ? keyboardHeight : safeBottom;

    // Canvas bottom: reserve space for the strip only; keyboard handled by pan.
    final canvasBottom = switch (_chatState) {
      _ChatState.closed  => safeBottom + _stripHeight,
      _ChatState.partial => stripBottom + _stripHeight + _partialBarHeight,
      _ChatState.max     => mq.size.height - mq.viewPadding.top,
    };

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
          child: AnimatedSlide(
            offset: Offset(0, -_kbPanOffset / (mq.size.height - canvasBottom)),
            duration: const Duration(milliseconds: 250),
            curve: Curves.easeOut,
            child: _remoteCanvas(),
          ),
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

        // Layer 2: power strip — hidden only in max chat state.
        if (_chatState != _ChatState.max)
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
                onDisplaySwitch: _onDisplaySwitch,
                onZoomFit: _onZoomFit,
                onMouseModeToggle: _onMouseModeToggle,
                onClipboardPaste: _onClipboardPaste,
                onNextDisplay: _onNextDisplay,
                onFileSend: _onFileSend,
                ffi: widget.ffi,
                onSessionsTap: _onSessionsTap,
              ),
            ),
          ),

        // Layer 3a: partial chat bar — slim input above the strip.
        if (_chatState == _ChatState.partial)
          Positioned(
            left: 0,
            right: 0,
            bottom: stripBottom + _stripHeight,
            child: _MeasureHeight(
              onChange: (h) {
                if (h != _partialBarHeight) setState(() => _partialBarHeight = h);
              },
              child: TerminalChatPartialBar(
                inputBridge: _bridge,
                onMaximize: _onChatMaximize,
                onClose: _onChatToggle,
              ),
            ),
          ),

        // Layer 3b: max chat view — fills screen above keyboard.
        if (_chatState == _ChatState.max)
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: TerminalChatMaxView(
              inputBridge: _bridge,
              terminal: widget.ffi.terminalModels[0]?.terminal,
              terminalTitle: widget.ffi.terminalModels[0]?.terminalTitle ?? '',
              onMinimize: _onChatMinimize,
              onClose: _onChatToggle,
            ),
          ),

        // Layer 3c: Claude session indicator — top-left corner dot.
        Positioned(
          top: safeAreaTop + AppTokens.spaceSm,
          left: AppTokens.spaceMd,
          child: AnimatedBuilder(
            animation: widget.ffi.terminalModels[0] ?? _nullNotifier,
            builder: (_, __) {
              final model = widget.ffi.terminalModels[0];
              return ClaudeSessionIndicator(
                terminal: model?.terminal,
                terminalTitle: model?.terminalTitle ?? '',
                terminalOpened: model?.terminalOpened ?? false,
              );
            },
          ),
        ),

        // Layer 4: cursor overlay — unconstrained so it can cross the
        // canvas/strip boundary without being clipped.
        // Offset by safeAreaTop because CursorModel coords are SafeArea-relative.
        // IgnorePointer so the full-screen overlay doesn't absorb taps/drags.
        Positioned.fill(
          top: safeAreaTop,
          child: IgnorePointer(
            child: MultiProvider(
              providers: [
                ChangeNotifierProvider.value(value: widget.ffi.cursorModel),
                ChangeNotifierProvider.value(value: widget.ffi.canvasModel),
                ChangeNotifierProvider.value(value: widget.ffi.ffiModel),
              ],
              child: CursorPaint(widget.id),
            ),
          ),
        ),

        // Layer 4.5: floating vertical macro bar — anchored just above the strip,
        // expands upward. Draggable vertically; position and collapsed state persist.
        FloatingMacroBar(
          bridge: _bridge,
          stripTop: stripBottom + _stripHeight,
          onZoomFit: _onZoomFit,
          onMouseModeToggle: _onMouseModeToggle,
          onClipboardPaste: _onClipboardPaste,
        ),

        // Layer 5: full-screen dialog overlay, keyed to _fullScreenOverlayState.
        // Dialogs (password prompt, etc.) inserted here span the entire screen.
        Positioned.fill(
          child: Overlay(key: _fullScreenOverlayState.key, initialEntries: const []),
        ),
      ],
    );
  }

  Widget _remoteCanvas() {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: widget.ffi.ffiModel),
        ChangeNotifierProvider.value(value: widget.ffi.imageModel),
        ChangeNotifierProvider.value(value: widget.ffi.cursorModel),
        ChangeNotifierProvider.value(value: widget.ffi.canvasModel),
      ],
      child: RemotePage(
        id: widget.id,
        ffi: widget.ffi,
        password: widget.password,
        isSharedPassword: widget.isSharedPassword,
        forceRelay: widget.forceRelay,
        hideKeyHelpTools: true,
        hideBottomBar: true,
        hideCursorPaint: true,
        onTwoFingerScroll: _onTwoFingerScroll,
      ),
    );
  }

}

// ─── Macro sheet ─────────────────────────────────────────────────────────────

class _MacroSheet extends StatefulWidget {
  final InputBridge bridge;
  final FFI ffi;
  final VoidCallback onZoomFit;
  final VoidCallback onMouseModeToggle;
  final VoidCallback onClipboardPaste;
  const _MacroSheet({
    required this.bridge,
    required this.ffi,
    required this.onZoomFit,
    required this.onMouseModeToggle,
    required this.onClipboardPaste,
  });

  @override
  State<_MacroSheet> createState() => _MacroSheetState();
}

class _MacroSheetState extends State<_MacroSheet> {
  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Macros',
              style: TextStyle(
                color: Colors.white70,
                fontSize: 13,
                fontWeight: FontWeight.w600,
                letterSpacing: 0.5,
              ),
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                _MacroButton(
                  label: '⌃V',
                  tooltip: 'Paste (Ctrl+V)',
                  onTap: () => widget.bridge.tapKey('v', modifiers: {'ctrl'}),
                ),
                _MacroButton(
                  label: '⌘V',
                  tooltip: 'Paste (Cmd+V)',
                  onTap: () => widget.bridge.tapKey('v', modifiers: {'meta'}),
                ),
                _MacroButton(
                  label: '⌘⇧V',
                  tooltip: 'Paste (Cmd+Shift+V)',
                  onTap: () => widget.bridge.tapKey('v', modifiers: {'meta', 'shift'}),
                ),
                _MacroButton(
                  label: '⌘⇧[',
                  tooltip: '1Password (Cmd+Shift+[)',
                  onTap: () => widget.bridge.tapKey('[', modifiers: {'meta', 'shift'}),
                ),
                _MacroButton(
                  label: '⌘⇥',
                  tooltip: 'App Switcher (Cmd+Tab)',
                  onTap: () => widget.bridge.tapKey('tab', modifiers: {'meta'}),
                ),
                _MacroButton(
                  label: '⌘N',
                  tooltip: 'New Window (Cmd+N)',
                  onTap: () => widget.bridge.tapKey('n', modifiers: {'meta'}),
                ),
                _MacroButton(
                  label: '⇱',
                  tooltip: 'Home',
                  onTap: () => widget.bridge.tapKey('home'),
                ),
                _MacroButton(
                  label: '⇲',
                  tooltip: 'End',
                  onTap: () => widget.bridge.tapKey('end'),
                ),
                _MacroButton(
                  label: '⌥↵',
                  tooltip: 'Option+Enter',
                  onTap: () => widget.bridge.tapKey('return', modifiers: {'alt'}),
                ),
                _MacroButton(
                  label: 'F12',
                  tooltip: 'F12',
                  onTap: () => widget.bridge.tapKey('f12'),
                ),
                _MacroButton(
                  label: '⌘⇧2',
                  tooltip: 'Screenshot (Cmd+Shift+2)',
                  onTap: () => widget.bridge.tapKey('2', modifiers: {'meta', 'shift'}),
                ),
                _MacroButton(
                  label: '🐁',
                  tooltip: 'Input mode',
                  onTap: () async {
                    final current = widget.ffi.ffiModel.touchMode;
                    final switched = await showDialog<bool>(
                      context: context,
                      builder: (ctx) => _InputModeDialog(touchMode: current),
                    );
                    if (switched != null && switched != current) {
                      widget.onMouseModeToggle();
                      setState(() {});
                    }
                  },
                ),
                _MacroButton(
                  label: '⤢',
                  tooltip: 'Zoom to fit height',
                  onTap: () {
                    Navigator.of(context).pop();
                    widget.onZoomFit();
                  },
                ),
                _MacroButton(
                  label: '📋→',
                  tooltip: 'Paste iPhone clipboard to remote',
                  onTap: () {
                    Navigator.of(context).pop();
                    widget.onClipboardPaste();
                  },
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _InputModeDialog extends StatefulWidget {
  final bool touchMode;
  const _InputModeDialog({required this.touchMode});

  @override
  State<_InputModeDialog> createState() => _InputModeDialogState();
}

class _InputModeDialogState extends State<_InputModeDialog> {
  late bool _touchMode;

  @override
  void initState() {
    super.initState();
    _touchMode = widget.touchMode;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      backgroundColor: const Color(0xFF1E1E2E),
      title: const Text('Input Mode', style: TextStyle(color: Colors.white)),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          RadioListTile<bool>(
            value: false,
            groupValue: _touchMode,
            onChanged: (v) => setState(() => _touchMode = v!),
            activeColor: const Color(0xFF0A84FF),
            title: const Text('Mouse mode', style: TextStyle(color: Colors.white, fontWeight: FontWeight.w500)),
            subtitle: const Text('Tap moves the remote cursor to where you tapped. Best for precise clicking.',
                style: TextStyle(color: Color(0xFF8E8E93), fontSize: 12)),
          ),
          RadioListTile<bool>(
            value: true,
            groupValue: _touchMode,
            onChanged: (v) => setState(() => _touchMode = v!),
            activeColor: const Color(0xFF0A84FF),
            title: const Text('Touch mode', style: TextStyle(color: Colors.white, fontWeight: FontWeight.w500)),
            subtitle: const Text('Drag moves the cursor relatively, like a trackpad. Long-press = right click.',
                style: TextStyle(color: Color(0xFF8E8E93), fontSize: 12)),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(null),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: () => Navigator.of(context).pop(_touchMode),
          style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF0A84FF)),
          child: const Text('Apply', style: TextStyle(color: Colors.white)),
        ),
      ],
    );
  }
}

class _MacroButton extends StatelessWidget {
  final String label;
  final String tooltip;
  final VoidCallback onTap;

  const _MacroButton({
    required this.label,
    required this.tooltip,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Material(
        color: const Color(0xFF2A2A3E),
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: () {
            HapticFeedback.lightImpact();
            onTap();
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            child: Text(
              label,
              style: const TextStyle(
                color: Colors.white,
                fontSize: 15,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// ─── Measure height ───────────────────────────────────────────────────────────

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

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/mobile/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:provider/provider.dart';
import 'package:flutter_keyboard_visibility/flutter_keyboard_visibility.dart';

import '../chat/terminal_chat_overlay.dart';
import '../input/input_bridge.dart';
import '../input/text_field_bridge.dart';
import '../overlay/floating_macro_bar.dart';
import '../strip/models/modifier_state.dart';
import '../strip/widgets/power_strip.dart';
import 'session_registry.dart';

// Full-screen overlay state for this session, overrides the constrained
// BlockableOverlay set by RemotePage.applyFfi so dialogs span the screen.
class _FullScreenOverlayState extends OverlayKeyState {}

class RemoteSessionScreen extends StatefulWidget {
  final String id;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;
  final VoidCallback? onSessionClosed;
  final VoidCallback? onActivated;
  final VoidCallback? onDeactivated;
  final void Function(String peerId)? onSwitchSession;

  const RemoteSessionScreen({
    super.key,
    required this.id,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
    this.onSessionClosed,
    this.onActivated,
    this.onDeactivated,
    this.onSwitchSession,
  });

  @override
  State<RemoteSessionScreen> createState() => _RemoteSessionScreenState();
}

class _RemoteSessionScreenState extends State<RemoteSessionScreen> {
  late FFI _ffi;
  late final InputBridge _bridge;
  final _modCtl = ModifierController();
  final _kbFocusNode = FocusNode();
  bool _chatOpen = false;
  double _stripHeight = 0;
  double _kbPanOffset = 0;
  late final StreamSubscription<bool> _kbVisibilitySub;
  final _fullScreenOverlayState = _FullScreenOverlayState();

  @override
  void initState() {
    super.initState();
    _ffi = SessionRegistry.instance.addSession(
          peerId: widget.id,
          password: widget.password,
          isSharedPassword: widget.isSharedPassword,
          forceRelay: widget.forceRelay,
        ) ??
        gFFI;
    _bridge = InputBridge(_ffi.sessionId);
    _kbVisibilitySub = KeyboardVisibilityController()
        .onChange
        .listen(_onKeyboardVisibilityChanged);
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
      _ffi.canvasModel.saveMobileOffsetBeforeSoftKeyboard();
      _ffi.canvasModel.isMobileCanvasChanged = false;
      final mq = MediaQuery.of(context);
      setState(() => _kbPanOffset = mq.viewInsets.bottom * 0.4);
    } else {
      _ffi.canvasModel.restoreMobileOffsetAfterSoftKeyboard();
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
    _bridge.scroll(dx.round(), dy.round());
  }

  void _onDisconnect() {
    clientClose(_ffi.sessionId, _ffi);
  }

  void _onChatToggle() {
    setState(() => _chatOpen = !_chatOpen);
  }

  void _onDisplaySwitch() {
    showOptions(context, widget.id, _ffi.dialogManager, _ffi);
  }

  void _onNextDisplay() {
    final pi = _ffi.ffiModel.pi;
    final count = pi.displays.length;
    if (count <= 1) return;
    final next = (pi.currentDisplay + 1) % count;
    openMonitorInTheSameTab(next, _ffi, pi);
  }

  void _onZoomFit() {
    // Scale the remote canvas so its height exactly fills the canvas area
    // (from screen top to the top of the power strip / keyboard).
    final displayHeight = _ffi.canvasModel.getDisplayHeight();
    if (displayHeight <= 0) return;
    final mq = MediaQuery.of(context);
    final keyboardHeight = mq.viewInsets.bottom;
    final stripBottom = keyboardHeight > 0 ? keyboardHeight : mq.viewPadding.bottom;
    final canvasHeight = mq.size.height - mq.viewPadding.top - stripBottom - _stripHeight;
    if (canvasHeight <= 0) return;
    final targetScale = canvasHeight / displayHeight;
    final center = Offset(mq.size.width / 2, canvasHeight / 2);
    // updateScale takes a multiplier; divide target by current to get delta.
    final delta = targetScale / _ffi.canvasModel.scale;
    _ffi.canvasModel.updateScale(delta, center);
    _ffi.canvasModel.isMobileCanvasChanged = true;
  }

  void _onMouseModeToggle() {
    _ffi.ffiModel.toggleTouchMode();
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
        onZoomFit: _onZoomFit,
        onMouseModeToggle: _onMouseModeToggle,
        onClipboardPaste: _onClipboardPaste,
      ),
    );
  }

  void _onSessionsTap() {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: const Color(0xFF1E1E2E),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (ctx) => _SessionPickerSheet(
        currentPeerId: widget.id,
        onSwitch: (peerId) {
          Navigator.pop(ctx);
          widget.onSwitchSession?.call(peerId);
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final safeBottom = mq.viewPadding.bottom;
    final stripBottom = safeBottom;

    // Canvas bottom: reserve space for the strip only; keyboard handled by pan.
    final canvasBottom = _chatOpen
        ? mq.size.height * (1 - 0.55)
        : safeBottom + _stripHeight;

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
                ffiModel: _ffi.ffiModel,
                onMacrosTap: _onMacrosTap,
                onKeyboardTap: _onKeyboardTap,
                onDisconnect: _onDisconnect,
                onChatToggle: _onChatToggle,
                onDisplaySwitch: _onDisplaySwitch,
                onZoomFit: _onZoomFit,
                onMouseModeToggle: _onMouseModeToggle,
                onClipboardPaste: _onClipboardPaste,
                onNextDisplay: _onNextDisplay,
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
              onClose: _onChatToggle,
            ),
          ),

        // Layer 4: cursor overlay — unconstrained so it can cross the
        // canvas/strip boundary without being clipped.
        // Offset by safeAreaTop because CursorModel coords are SafeArea-relative.
        // IgnorePointer so the full-screen overlay doesn't absorb taps/drags.
        Positioned.fill(
          top: safeAreaTop,
          child: IgnorePointer(child: CursorPaint(widget.id)),
        ),

        // Layer 4.5: floating vertical macro bar — anchored just above the strip,
        // expands upward. Draggable vertically; position and collapsed state persist.
        FloatingMacroBar(
          bridge: _bridge,
          ffiModel: _ffi.ffiModel,
          stripTop: canvasBottom,
          onZoomFit: _onZoomFit,
          onMouseModeToggle: _onMouseModeToggle,
          onClipboardPaste: _onClipboardPaste,
        ),

        // Layer 4.6: session switcher badge — top-left, shown when > 1 session.
        ListenableBuilder(
          listenable: SessionRegistry.instance,
          builder: (context, _) {
            if (SessionRegistry.instance.count <= 1) return const SizedBox.shrink();
            return Positioned(
              top: safeAreaTop + 8,
              left: 8,
              child: GestureDetector(
                onTap: _onSessionsTap,
                child: _SessionBadge(count: SessionRegistry.instance.count),
              ),
            );
          },
        ),

        // Layer 5: full-screen dialog overlay, keyed to _fullScreenOverlayState.
        // Dialogs (password prompt, etc.) inserted here span the entire screen.
        Positioned.fill(
          child: Overlay(key: _fullScreenOverlayState.key, initialEntries: const []),
        ),
      ],
    );
  }

  Widget _remoteCanvas() => MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: _ffi.ffiModel),
          ChangeNotifierProvider.value(value: _ffi.imageModel),
          ChangeNotifierProvider.value(value: _ffi.cursorModel),
          ChangeNotifierProvider.value(value: _ffi.canvasModel),
        ],
        child: RemotePage(
          id: widget.id,
          ffi: _ffi,
          password: widget.password,
          isSharedPassword: widget.isSharedPassword,
          forceRelay: widget.forceRelay,
          hideKeyHelpTools: true,
          hideBottomBar: true,
          hideCursorPaint: true,
          onTwoFingerScroll: _onTwoFingerScroll,
          overlayKeyState: _fullScreenOverlayState,
        ),
      );
}

// ─── Session badge ────────────────────────────────────────────────────────────

class _SessionBadge extends StatelessWidget {
  final int count;
  const _SessionBadge({required this.count});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: Colors.black54,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.layers, color: Colors.white, size: 16),
          const SizedBox(width: 4),
          Text(
            '$count',
            style: const TextStyle(
              color: Colors.white,
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
          ),
        ],
      ),
    );
  }
}

// ─── Session picker sheet ─────────────────────────────────────────────────────

class _SessionPickerSheet extends StatelessWidget {
  final String currentPeerId;
  final void Function(String peerId) onSwitch;

  const _SessionPickerSheet({
    required this.currentPeerId,
    required this.onSwitch,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: SessionRegistry.instance,
      builder: (context, _) {
        final peerIds = SessionRegistry.instance.peerIds;
        return SafeArea(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Sessions (${peerIds.length}/${SessionRegistry.kMaxSessions})',
                  style: const TextStyle(
                    color: Colors.white70,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 0.5,
                  ),
                ),
                const SizedBox(height: 12),
                ...peerIds.map((id) => _SessionTile(
                      peerId: id,
                      isActive: id == currentPeerId,
                      onSwitch: () => onSwitch(id),
                      onClose: () => SessionRegistry.instance.closeSession(id),
                    )),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _SessionTile extends StatelessWidget {
  final String peerId;
  final bool isActive;
  final VoidCallback onSwitch;
  final VoidCallback onClose;

  const _SessionTile({
    required this.peerId,
    required this.isActive,
    required this.onSwitch,
    required this.onClose,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Material(
        color: isActive ? const Color(0xFF2A2A5E) : const Color(0xFF2A2A3E),
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: isActive ? null : onSwitch,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: [
                Container(
                  width: 8,
                  height: 8,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: isActive ? Colors.greenAccent : Colors.grey,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    peerId,
                    style: const TextStyle(color: Colors.white, fontSize: 15),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (isActive)
                  const Text(
                    'active',
                    style: TextStyle(color: Colors.white38, fontSize: 12),
                  ),
                const SizedBox(width: 8),
                GestureDetector(
                  onTap: onClose,
                  child: const Icon(Icons.close, color: Colors.white54, size: 18),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

// ─── Macro sheet ─────────────────────────────────────────────────────────────

class _MacroSheet extends StatefulWidget {
  final InputBridge bridge;
  final VoidCallback onZoomFit;
  final VoidCallback onMouseModeToggle;
  final VoidCallback onClipboardPaste;
  const _MacroSheet({
    required this.bridge,
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
    // touchMode read from global gFFI for macro sheet — acceptable since
    // _MacroSheet is opened from within the active session's RemoteSessionScreen.
    final touchMode = gFFI.ffiModel.touchMode;
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
                  label: touchMode ? '🖱' : '👆',
                  tooltip: touchMode ? 'Switch to Mouse mode' : 'Switch to Touch mode',
                  onTap: () {
                    widget.onMouseModeToggle();
                    setState(() {});
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

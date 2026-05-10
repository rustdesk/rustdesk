import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';

import '../input/input_bridge.dart';
import '../settings/settings_store.dart';
import '../theme/tokens.dart';

// Width of the expanded button column.
const _kBarWidth = 56.0;
// Height of each macro button.
const _kBtnSize = 48.0;
// The collapsed drag-handle tab (just the ⚡ icon).
const _kTabSize = 36.0;
// Gap between buttons.
const _kGap = 6.0;
// Right-edge inset from screen edge.
const _kRightInset = 8.0;
// Width of horizontal submenu buttons.
const _kSubBtnSize = 48.0;

class FloatingMacroBar extends StatefulWidget {
  final InputBridge bridge;
  // Bottom of the canvas area (top of the power strip). The bar is anchored
  // just above this value so it sits flush above the custom keyboard strip.
  final double stripTop;
  final VoidCallback onZoomFit;
  final VoidCallback onMouseModeToggle;
  final VoidCallback onClipboardPaste;

  const FloatingMacroBar({
    super.key,
    required this.bridge,
    required this.stripTop,
    required this.onZoomFit,
    required this.onMouseModeToggle,
    required this.onClipboardPaste,
  });

  @override
  State<FloatingMacroBar> createState() => _FloatingMacroBarState();
}

class _FloatingMacroBarState extends State<FloatingMacroBar>
    with SingleTickerProviderStateMixin {
  // Extra upward offset above the strip (0 = flush against the strip top).
  // Positive values move the handle further up the screen.
  late double _above;
  bool _collapsed = false;
  bool _rectOpen = false;

  late final AnimationController _animCtl;
  late final Animation<double> _expandAnim;

  @override
  void initState() {
    super.initState();
    _above = settingsStore.macroBarTopOffset;
    _collapsed = settingsStore.macroBarCollapsed;

    _animCtl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 200),
    );
    _expandAnim = CurvedAnimation(parent: _animCtl, curve: Curves.easeOutCubic);
    if (!_collapsed) _animCtl.value = 1.0;
  }

  @override
  void dispose() {
    _animCtl.dispose();
    super.dispose();
  }

  void _toggleCollapse() {
    HapticFeedback.lightImpact();
    setState(() {
      _collapsed = !_collapsed;
      if (_collapsed) _rectOpen = false;
    });
    if (_collapsed) {
      _animCtl.reverse();
    } else {
      _animCtl.forward();
    }
    settingsStore.setMacroBarCollapsed(_collapsed);
  }

  void _onDrag(DragUpdateDetails d, double screenH) {
    setState(() {
      // Dragging up (negative dy) increases _above; clamp so handle stays on screen.
      _above = (_above - d.delta.dy).clamp(0.0, screenH - widget.stripTop - _kTabSize);
    });
  }

  void _onDragEnd(DragEndDetails _) {
    settingsStore.setMacroBarTopOffset(_above);
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final safeRight = mq.viewPadding.right;
    final safeTop = mq.viewPadding.top;
    final screenH = mq.size.height;

    // Bottom of the handle = top of the strip + _above extra offset.
    final handleBottom = widget.stripTop + _above;

    // Maximum height the button list can occupy before it would clip off-screen.
    final maxButtonsHeight = screenH - handleBottom - _kTabSize - _kGap - safeTop;

    return Positioned(
      bottom: handleBottom,
      right: safeRight + _kRightInset,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          // ── Rectangle corner submenu — slides in to the left ───────────────
          if (_rectOpen) ...[
            _RectSubmenu(bridge: widget.bridge),
            const SizedBox(width: _kGap),
          ],

          // ── Vertical macro bar ─────────────────────────────────────────────
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            mainAxisSize: MainAxisSize.min,
            // Buttons grow upward above the handle.
            verticalDirection: VerticalDirection.up,
            children: [
              // ── Drag handle / collapse tab ────────────────────────────────
              GestureDetector(
                onTap: _toggleCollapse,
                onVerticalDragUpdate: (d) => _onDrag(d, screenH),
                onVerticalDragEnd: _onDragEnd,
                child: _Handle(collapsed: _collapsed),
              ),

              const SizedBox(height: _kGap),

              // ── Expandable button column — grows upward ───────────────────
              SizeTransition(
                sizeFactor: _expandAnim,
                axisAlignment: 1, // anchor to bottom so it expands upward
                child: ConstrainedBox(
                  constraints: BoxConstraints(maxHeight: maxButtonsHeight.clamp(0.0, double.infinity)),
                  child: SingleChildScrollView(
                    reverse: true, // scroll origin at bottom so top buttons scroll into view
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      verticalDirection: VerticalDirection.up,
                      children: _buildButtons(),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  List<Widget> _buildButtons() {
    final b = widget.bridge;
    return [
      _Btn(label: 'git\ncmt', tooltip: 'git commit',           onTap: () => b.typeString('git commit\n')),
      _gap(),
      _Btn(label: '⌃V',  tooltip: 'Ctrl+V',                  onTap: () => b.tapKey('v', modifiers: {'ctrl'})),
      _gap(),
      _Btn(label: '⌘V',  tooltip: 'Cmd+V',                   onTap: () => b.tapKey('v', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⌘⇧V', tooltip: 'Cmd+Shift+V',             onTap: () => b.tapKey('v', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⌘⇧[', tooltip: '1Password',               onTap: () => b.tapKey('[', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⌘⎵',  tooltip: 'Spotlight (Cmd+Space)',    onTap: () => b.tapKey('space', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⌘⇥',  tooltip: 'App Switcher',            onTap: () => b.tapKey('tab', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⌘N',  tooltip: 'New Window',               onTap: () => b.tapKey('n', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⇱',   tooltip: 'Home',                     onTap: () => b.tapKey('home')),
      _gap(),
      _Btn(label: '⇲',   tooltip: 'End',                      onTap: () => b.tapKey('end')),
      _gap(),
      _Btn(label: '⌥↵',  tooltip: 'Option+Enter',             onTap: () => b.tapKey('return', modifiers: {'alt'})),
      _gap(),
      _Btn(label: 'F12', tooltip: 'F12',                      onTap: () => b.tapKey('f12')),
      _gap(),
      // ── Rectangle window manager ──────────────────────────────────────────
      _Btn(
        label: '⤢↑',
        tooltip: 'Rectangle: Maximize',
        onTap: () => b.tapKey('up', modifiers: {'ctrl', 'alt', 'meta'}),
      ),
      _gap(),
      _Btn(
        label: '⤢←',
        tooltip: 'Rectangle: Left Half',
        onTap: () => b.tapKey('left', modifiers: {'meta', 'alt'}),
      ),
      _gap(),
      _Btn(
        label: '⤢→',
        tooltip: 'Rectangle: Right Half',
        onTap: () => b.tapKey('right', modifiers: {'meta', 'alt'}),
      ),
      _gap(),
      _Btn(
        label: '▭',
        tooltip: 'Rectangle: Corners',
        onTap: () => setState(() => _rectOpen = !_rectOpen),
      ),
      _gap(),
      _Btn(label: '⌘⇧2', tooltip: 'Screenshot',              onTap: () => b.tapKey('2', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⤢',   tooltip: 'Zoom to fit height',       onTap: () { widget.onZoomFit(); _toggleCollapse(); }),
      _gap(),
      _Btn(label: '📋→', tooltip: 'Paste iPhone clipboard',   onTap: () { widget.onClipboardPaste(); _toggleCollapse(); }),
      _gap(),
      _Btn(
        tooltip: 'Toggle mouse/touch mode',
        labelBuilder: () => gFFI.ffiModel.touchMode ? '🖱' : '👆',
        onTap: widget.onMouseModeToggle,
      ),
    ];
  }

  Widget _gap() => const SizedBox(height: _kGap);
}

// ── Rectangle corner submenu (horizontal row to the left of the bar) ──────────

class _RectSubmenu extends StatelessWidget {
  final InputBridge bridge;
  const _RectSubmenu({required this.bridge});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _SubBtn(label: '↖', tooltip: 'Top Left (Ctrl+Alt+U)',    onTap: () => bridge.tapKey('u', modifiers: {'ctrl', 'alt'})),
        const SizedBox(width: _kGap),
        _SubBtn(label: '↗', tooltip: 'Top Right (Ctrl+Alt+I)',   onTap: () => bridge.tapKey('i', modifiers: {'ctrl', 'alt'})),
        const SizedBox(width: _kGap),
        _SubBtn(label: '↙', tooltip: 'Bottom Left (Ctrl+Alt+J)', onTap: () => bridge.tapKey('j', modifiers: {'ctrl', 'alt'})),
        const SizedBox(width: _kGap),
        _SubBtn(label: '↘', tooltip: 'Bottom Right (Ctrl+Alt+K)',onTap: () => bridge.tapKey('k', modifiers: {'ctrl', 'alt'})),
      ],
    );
  }
}

class _SubBtn extends StatelessWidget {
  final String label;
  final String tooltip;
  final VoidCallback onTap;
  const _SubBtn({required this.label, required this.tooltip, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      preferBelow: false,
      child: GestureDetector(
        onTap: () {
          HapticFeedback.lightImpact();
          onTap();
        },
        child: Container(
          width: _kSubBtnSize,
          height: _kSubBtnSize,
          decoration: BoxDecoration(
            color: AppTokens.colorBgSurface,
            borderRadius: BorderRadius.circular(AppTokens.radiusKey),
            boxShadow: const [
              BoxShadow(blurRadius: 4, color: Colors.black26, offset: Offset(0, 1)),
            ],
          ),
          child: Center(
            child: Text(
              label,
              style: AppTokens.fontKey.copyWith(color: AppTokens.colorTextHigh),
            ),
          ),
        ),
      ),
    );
  }
}

// ── Drag handle ────────────────────────────────────────────────────────────────

class _Handle extends StatelessWidget {
  final bool collapsed;
  const _Handle({required this.collapsed});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: _kBarWidth,
      height: _kTabSize,
      decoration: BoxDecoration(
        color: AppTokens.colorPrimary,
        borderRadius: BorderRadius.circular(AppTokens.radiusKey),
        boxShadow: const [
          BoxShadow(blurRadius: 6, color: Colors.black38, offset: Offset(0, 2)),
        ],
      ),
      child: Center(
        child: Text(
          collapsed ? '⚡' : '✕',
          style: const TextStyle(fontSize: 18),
        ),
      ),
    );
  }
}

// ── Single macro button ────────────────────────────────────────────────────────

class _Btn extends StatelessWidget {
  final String? label;
  final String Function()? labelBuilder;
  final String tooltip;
  final VoidCallback onTap;

  const _Btn({
    this.label,
    this.labelBuilder,
    required this.tooltip,
    required this.onTap,
  }) : assert(label != null || labelBuilder != null);

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      preferBelow: false,
      child: GestureDetector(
        onTap: () {
          HapticFeedback.lightImpact();
          onTap();
        },
        child: Container(
          width: _kBarWidth,
          height: _kBtnSize,
          decoration: BoxDecoration(
            color: AppTokens.colorBgSurface,
            borderRadius: BorderRadius.circular(AppTokens.radiusKey),
            boxShadow: const [
              BoxShadow(blurRadius: 4, color: Colors.black26, offset: Offset(0, 1)),
            ],
          ),
          child: Center(
            child: Text(
              labelBuilder != null ? labelBuilder!() : label!,
              style: AppTokens.fontKey.copyWith(color: AppTokens.colorTextHigh),
              textAlign: TextAlign.center,
            ),
          ),
        ),
      ),
    );
  }
}

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

class FloatingMacroBar extends StatefulWidget {
  final InputBridge bridge;
  final VoidCallback onZoomFit;
  final VoidCallback onMouseModeToggle;
  final VoidCallback onClipboardPaste;

  const FloatingMacroBar({
    super.key,
    required this.bridge,
    required this.onZoomFit,
    required this.onMouseModeToggle,
    required this.onClipboardPaste,
  });

  @override
  State<FloatingMacroBar> createState() => _FloatingMacroBarState();
}

class _FloatingMacroBarState extends State<FloatingMacroBar>
    with SingleTickerProviderStateMixin {
  // Top offset from the top of the safe area. Loaded from settings, defaults
  // to 120 so the bar starts below the status bar with some breathing room.
  late double _top;
  bool _collapsed = false;

  late final AnimationController _animCtl;
  late final Animation<double> _expandAnim;

  @override
  void initState() {
    super.initState();
    _top = settingsStore.macroBarTopOffset;
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
    setState(() => _collapsed = !_collapsed);
    if (_collapsed) {
      _animCtl.reverse();
    } else {
      _animCtl.forward();
    }
    settingsStore.setMacroBarCollapsed(_collapsed);
  }

  void _onDrag(DragUpdateDetails d, double screenH, double safeTop) {
    setState(() {
      _top = (_top + d.delta.dy).clamp(0.0, screenH - safeTop - _kBtnSize);
    });
  }

  void _onDragEnd(DragEndDetails _) {
    settingsStore.setMacroBarTopOffset(_top);
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final safeTop = mq.viewPadding.top;
    final safeRight = mq.viewPadding.right;
    final screenH = mq.size.height;

    return Positioned(
      top: safeTop + _top,
      right: safeRight + _kRightInset,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          // ── Drag handle / collapse tab ──────────────────────────────────
          GestureDetector(
            onTap: _toggleCollapse,
            onVerticalDragUpdate: (d) => _onDrag(d, screenH, safeTop),
            onVerticalDragEnd: _onDragEnd,
            child: _Handle(collapsed: _collapsed),
          ),

          const SizedBox(height: _kGap),

          // ── Expandable button column ────────────────────────────────────
          SizeTransition(
            sizeFactor: _expandAnim,
            axisAlignment: -1,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: _buildButtons(),
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _buildButtons() {
    final b = widget.bridge;
    return [
      _Btn(label: '⌃V',  tooltip: 'Ctrl+V',            onTap: () => b.tapKey('v', modifiers: {'ctrl'})),
      _gap(),
      _Btn(label: '⌘V',  tooltip: 'Cmd+V',             onTap: () => b.tapKey('v', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⌘⇧V', tooltip: 'Cmd+Shift+V',       onTap: () => b.tapKey('v', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⌘⇧[', tooltip: '1Password',         onTap: () => b.tapKey('[', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⌘⇥',  tooltip: 'App Switcher',      onTap: () => b.tapKey('tab', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⌘N',  tooltip: 'New Window',         onTap: () => b.tapKey('n', modifiers: {'meta'})),
      _gap(),
      _Btn(label: '⇱',   tooltip: 'Home',               onTap: () => b.tapKey('home')),
      _gap(),
      _Btn(label: '⇲',   tooltip: 'End',                onTap: () => b.tapKey('end')),
      _gap(),
      _Btn(label: '⌘⇧2', tooltip: 'Screenshot',        onTap: () => b.tapKey('2', modifiers: {'meta', 'shift'})),
      _gap(),
      _Btn(label: '⤢',   tooltip: 'Zoom to fit height', onTap: () { widget.onZoomFit(); _toggleCollapse(); }),
      _gap(),
      _Btn(label: '📋→', tooltip: 'Paste iPhone clipboard', onTap: () { widget.onClipboardPaste(); _toggleCollapse(); }),
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
  // Optional builder for labels that depend on reactive state (e.g. touch mode).
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

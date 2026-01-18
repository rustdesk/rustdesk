import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/overlay.dart';
import 'package:flutter_hbb/models/model.dart';

class RemoteToolDock extends StatefulWidget {
  const RemoteToolDock({
    super.key,
    required this.cursorModel,
    required this.position,
    required this.showArrowButton,
    required this.shortcutsVisible,
    required this.keyboardVisible,
    required this.onToggleKeyboard,
    required this.onToggleShortcuts,
    required this.onAddShortcut,
    required this.onArrowPressed,
  });

  final CursorModel cursorModel;
  final DraggableKeyPosition position;
  final bool showArrowButton;
  final bool shortcutsVisible;
  final bool keyboardVisible;
  final VoidCallback onToggleKeyboard;
  final VoidCallback onToggleShortcuts;
  final VoidCallback onAddShortcut;
  final VoidCallback onArrowPressed;

  @override
  State<RemoteToolDock> createState() => _RemoteToolDockState();
}

class _RemoteToolDockState extends State<RemoteToolDock> {
  static const double _btn = 46;
  static const double _gap = 10;
  static const double _margin = 10;
  Rect? _blockedRect;

  double get _dockWidth => _btn;

  int get _dockCount => 2 + (widget.showArrowButton ? 1 : 0);

  double get _dockHeight => _dockCount <= 0 ? 0 : _btn * _dockCount + _gap * (_dockCount - 1);

  void _ensureDefaultPosition(Size screenSize) {
    if (!widget.position.isInvalid()) return;
    final x = (screenSize.width - _dockWidth - _margin).clamp(0.0, screenSize.width);
    final y = (screenSize.height * 0.35).clamp(0.0, screenSize.height);
    widget.position.update(Offset(x.toDouble(), y.toDouble()));
  }

  void _updateBlockedRect() {
    final newRect = Rect.fromLTWH(
      widget.position.pos.dx,
      widget.position.pos.dy,
      _dockWidth,
      _dockHeight,
    );
    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
    }
    widget.cursorModel.addBlockedRect(newRect);
    _blockedRect = newRect;
  }

  void _moveBy(Offset delta, Size screenSize) {
    final pos = widget.position.pos;
    final maxX = (screenSize.width - _dockWidth).clamp(0.0, screenSize.width);
    final maxY = (screenSize.height - _dockHeight).clamp(0.0, screenSize.height);
    final x = (pos.dx + delta.dx).clamp(0.0, maxX);
    final y = (pos.dy + delta.dy).clamp(0.0, maxY);
    widget.position.update(Offset(x.toDouble(), y.toDouble()));
    _updateBlockedRect();
    setState(() {});
  }

  @override
  void dispose() {
    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
      _blockedRect = null;
    }
    super.dispose();
  }

  Widget _circleButton({
    required String semanticsLabel,
    required IconData icon,
    required VoidCallback onTap,
    VoidCallback? onLongPress,
    bool active = false,
  }) {
    return Semantics(
      label: semanticsLabel,
      button: true,
      child: GestureDetector(
        onTap: onTap,
        onLongPress: onLongPress,
        child: Container(
          width: _btn,
          height: _btn,
          decoration: BoxDecoration(
            color: const Color(0xFF2196F3).withOpacity(0.90),
            shape: BoxShape.circle,
            border: Border.all(
              color: active ? Colors.white : Colors.white70,
              width: active ? 2 : 1,
            ),
          ),
          child: Icon(icon, color: Colors.white, size: 22),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    _ensureDefaultPosition(screenSize);
    _updateBlockedRect();

    final children = <Widget>[
      _circleButton(
        semanticsLabel: 'u2_remote_shortcuts_toggle',
        icon: widget.shortcutsVisible ? Icons.view_list : Icons.view_list_outlined,
        onTap: widget.onToggleShortcuts,
        onLongPress: widget.onAddShortcut,
        active: widget.shortcutsVisible,
      ),
      _circleButton(
        semanticsLabel: 'u2_remote_keyboard_button',
        icon: widget.keyboardVisible ? Icons.keyboard_hide : Icons.keyboard,
        onTap: widget.onToggleKeyboard,
        active: widget.keyboardVisible,
      ),
      if (widget.showArrowButton)
        _circleButton(
          semanticsLabel: 'u2_remote_floating_arrow',
          icon: Icons.keyboard_arrow_up,
          onTap: widget.onArrowPressed,
        ),
    ];

    return Positioned(
      left: widget.position.pos.dx,
      top: widget.position.pos.dy,
      width: _dockWidth,
      height: _dockHeight,
      child: GestureDetector(
        onPanUpdate: (d) => _moveBy(d.delta, screenSize),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            for (var i = 0; i < children.length; i++) ...[
              if (i > 0) const SizedBox(height: _gap),
              children[i],
            ],
          ],
        ),
      ),
    );
  }
}


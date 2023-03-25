import 'package:flutter/material.dart';

class MenuButton extends StatefulWidget {
  final GestureTapCallback? onPressed;
  final Color color;
  final Color hoverColor;
  final Color? splashColor;
  final Widget child;
  final String? tooltip;
  final EdgeInsetsGeometry padding;
  final bool enableFeedback;
  const MenuButton({
    super.key,
    required this.onPressed,
    required this.color,
    required this.hoverColor,
    required this.child,
    this.splashColor,
    this.tooltip = "",
    this.padding = const EdgeInsets.symmetric(horizontal: 3, vertical: 6),
    this.enableFeedback = true,
  });

  @override
  State<MenuButton> createState() => _MenuButtonState();
}

class _MenuButtonState extends State<MenuButton> {
  bool _isHover = false;
  final double _borderRadius = 8.0;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: widget.padding,
      child: Tooltip(
        message: widget.tooltip,
        child: Material(
          type: MaterialType.transparency,
          child: Container(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(_borderRadius),
              color: _isHover ? widget.hoverColor : widget.color,
            ),
            child: InkWell(
              hoverColor: widget.hoverColor,
              onHover: (val) {
                setState(() {
                  _isHover = val;
                });
              },
              borderRadius: BorderRadius.circular(_borderRadius),
              splashColor: widget.splashColor,
              enableFeedback: widget.enableFeedback,
              onTap: widget.onPressed,
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }
}

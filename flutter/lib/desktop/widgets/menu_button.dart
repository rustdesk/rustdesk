import 'package:flutter/material.dart';

class MenuButton extends StatefulWidget {
  final GestureTapCallback? onPressed;
  final Color color;
  final Color hoverColor;
  final Color? splashColor;
  final Widget icon;
  final String? tooltip;
  final EdgeInsetsGeometry padding;
  final bool enableFeedback;
  const MenuButton({
    super.key,
    required this.onPressed,
    required this.color,
    required this.hoverColor,
    required this.icon,
    this.splashColor,
    this.tooltip = "",
    this.padding = const EdgeInsets.all(5),
    this.enableFeedback = true,
  });

  @override
  State<MenuButton> createState() => _MenuButtonState();
}

class _MenuButtonState extends State<MenuButton> {
  bool _isHover = false;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: widget.padding,
      child: Tooltip(
        message: widget.tooltip,
        child: Material(
          type: MaterialType.transparency,
          child: Ink(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(5),
              color: _isHover ? widget.hoverColor : widget.color,
            ),
            child: InkWell(
              onHover: (val) {
                setState(() {
                  _isHover = val;
                });
              },
              borderRadius: BorderRadius.circular(5),
              splashColor: widget.splashColor,
              enableFeedback: widget.enableFeedback,
              onTap: widget.onPressed,
              child: widget.icon,
            ),
          ),
        ),
      ),
    );
  }
}

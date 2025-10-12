import 'package:flutter/material.dart';

class DraggableDivider extends StatefulWidget {
  final Axis axis;
  final double thickness;
  final Color color;
  final Function(double)? onPointerMove;
  final VoidCallback? onHover;
  final EdgeInsets padding;
  const DraggableDivider({
    super.key,
    this.axis = Axis.horizontal,
    this.thickness = 1.0,
    this.color = const Color.fromARGB(200, 177, 175, 175),
    this.onPointerMove,
    this.padding = const EdgeInsets.symmetric(horizontal: 1.0),
    this.onHover,
  });

  @override
  State<DraggableDivider> createState() => _DraggableDividerState();
}

class _DraggableDividerState extends State<DraggableDivider> {
  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerMove: (event) {
        final dl = widget.axis == Axis.horizontal
            ? event.localDelta.dy
            : event.localDelta.dx;
        widget.onPointerMove?.call(dl);
      },
      onPointerHover: (event) => widget.onHover?.call(),
      child: MouseRegion(
        cursor: SystemMouseCursors.resizeLeftRight,
        child: Padding(
          padding: widget.padding,
          child: Container(
            decoration: BoxDecoration(color: widget.color),
            width: widget.axis == Axis.horizontal
                ? double.infinity
                : widget.thickness,
            height: widget.axis == Axis.horizontal
                ? widget.thickness
                : double.infinity,
          ),
        ),
      ),
    );
  }
}

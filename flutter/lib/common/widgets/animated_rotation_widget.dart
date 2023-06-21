import 'package:flutter/material.dart';

class AnimatedRotationWidget extends StatefulWidget {
  final VoidCallback onPressed;
  final ValueChanged<bool>? onHover;
  final Widget child;
  const AnimatedRotationWidget(
      {super.key, required this.onPressed, required this.child, this.onHover});

  @override
  State<AnimatedRotationWidget> createState() => AnimatedRotationWidgetState();
}

class AnimatedRotationWidgetState extends State<AnimatedRotationWidget> {
  double turns = 0.0;

  @override
  Widget build(BuildContext context) {
    return AnimatedRotation(
        turns: turns,
        duration: const Duration(milliseconds: 200),
        child: InkWell(
            onTap: () {
              setState(() => turns += 1.0);
              widget.onPressed();
            },
            onHover: widget.onHover,
            child: widget.child));
  }
}

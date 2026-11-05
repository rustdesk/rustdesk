import 'package:flutter/material.dart';

import 'session_palette.dart';

/// Keeps every existing action reachable in narrow windows and side rails.
class SessionToolbar extends StatelessWidget {
  const SessionToolbar(
      {super.key,
      required this.actions,
      required this.color,
      this.axis = Axis.horizontal});

  final List<Widget> actions;
  final Color color;
  final Axis axis;

  @override
  Widget build(BuildContext context) {
    final palette = SessionPalette.of(context, accent: color);
    final horizontal = axis == Axis.horizontal;
    final children = <Widget>[
      for (var i = 0; i < actions.length; i++) ...[
        if (i == 1)
          Padding(
            padding: horizontal
                ? const EdgeInsets.symmetric(horizontal: 4)
                : const EdgeInsets.symmetric(vertical: 4),
            child: SizedBox(
                width: horizontal ? 1 : 24,
                height: horizontal ? 20 : 1,
                child: ColoredBox(color: palette.line)),
          ),
        actions[i],
      ],
    ];
    return DecoratedBox(
      decoration: BoxDecoration(
          color: palette.deck,
          border: Border(bottom: BorderSide(color: palette.line))),
      child: IconTheme(
        data: IconThemeData(color: palette.foreground, size: 20),
        child: SingleChildScrollView(
          scrollDirection: axis,
          child: Padding(
            padding: horizontal
                ? const EdgeInsets.symmetric(horizontal: 4)
                : const EdgeInsets.symmetric(vertical: 8),
            child: horizontal
                ? Row(mainAxisSize: MainAxisSize.min, children: children)
                : Column(mainAxisSize: MainAxisSize.min, children: children),
          ),
        ),
      ),
    );
  }
}

class SessionToolbarReveal extends StatelessWidget {
  const SessionToolbarReveal(
      {super.key,
      required this.canvas,
      required this.tooltip,
      required this.onPressed});

  final Rect canvas;
  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) => Positioned.fromRect(
        rect: canvas,
        child: Align(
          alignment: Alignment.bottomRight,
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: FloatingActionButton(
                mini: true,
                tooltip: tooltip,
                onPressed: onPressed,
                child: const Icon(Icons.expand_less)),
          ),
        ),
      );
}

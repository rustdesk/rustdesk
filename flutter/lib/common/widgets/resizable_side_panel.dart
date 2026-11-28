import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class ResizableSidePanel extends StatefulWidget {
  static const double minWidth = 150;
  static const double maxWidth = 300;

  final String optionKey;
  final double defaultWidth;
  final Widget child;

  const ResizableSidePanel({
    Key? key,
    required this.optionKey,
    required this.defaultWidth,
    required this.child,
  }) : super(key: key);

  @override
  State<ResizableSidePanel> createState() => _ResizableSidePanelState();
}

class _ResizableSidePanelState extends State<ResizableSidePanel> {
  late double _width;

  @override
  void initState() {
    super.initState();
    final saved =
        double.tryParse(bind.mainGetLocalOption(key: widget.optionKey));
    _width = _clamp(saved ?? widget.defaultWidth);
  }

  double _clamp(double w) =>
      w.clamp(ResizableSidePanel.minWidth, ResizableSidePanel.maxWidth)
          .toDouble();

  void _persist() => bind.mainSetLocalOption(
      key: widget.optionKey, value: _width.toStringAsFixed(0));

  @override
  Widget build(BuildContext context) {
    final sign = Directionality.of(context) == TextDirection.rtl ? -1.0 : 1.0;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox(width: _width, child: widget.child),
        MouseRegion(
          cursor: SystemMouseCursors.resizeLeftRight,
          child: GestureDetector(
            behavior: HitTestBehavior.translucent,
            onHorizontalDragUpdate: (details) => setState(
                () => _width = _clamp(_width + sign * details.delta.dx)),
            onHorizontalDragEnd: (_) => _persist(),
            onHorizontalDragCancel: _persist,
            child: const SizedBox(width: 12, height: double.infinity),
          ),
        ),
      ],
    );
  }
}

extension ResizableSidePanelExt on Widget {
  Widget resizable(String optionKey, {required double defaultWidth}) =>
      ResizableSidePanel(
          optionKey: optionKey, defaultWidth: defaultWidth, child: this);
}

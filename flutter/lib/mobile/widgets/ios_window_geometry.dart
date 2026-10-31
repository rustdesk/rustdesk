import 'package:flutter/material.dart';

import '../../common/widgets/overlay.dart';

class IosWindowGeometry {
  IosWindowGeometry(Map event)
      : bounds = _rect(event['bounds']),
        partiallyOpen = event['hingeStatus'] == 'partiallyOpen',
        keyboard = event['keyboard'] is Map ? _rect(event['keyboard']) : null,
        divisions = List<Map>.from(event['divisions'] ?? []);

  final Rect bounds;
  final bool partiallyOpen;
  final Rect? keyboard;
  final List<Map> divisions;

  static Rect _rect(Map value) => Rect.fromLTWH(
      (value['x'] as num).toDouble(),
      (value['y'] as num).toDouble(),
      (value['width'] as num).toDouble(),
      (value['height'] as num).toDouble());

  Rect? horizontalDivision(Size size) {
    if (!partiallyOpen ||
        (bounds.width - size.width).abs() > .5 ||
        (bounds.height - size.height).abs() > .5) {
      return null;
    }
    for (final division in divisions) {
      final frame = _rect(division['frame']);
      if (frame.left > .5 ||
          frame.right < size.width - .5 ||
          frame.top <= 0 ||
          frame.bottom >= size.height ||
          frame.width <= frame.height) {
        continue;
      }
      return frame;
    }
    return null;
  }

  double splitY(Rect frame) {
    final division = divisions.firstWhere((d) => _rect(d['frame']) == frame);
    return frame.top + (division['margins']['top'] as num).toDouble();
  }
}

class IosControlOverlay extends StatefulWidget {
  const IosControlOverlay(
      {super.key, required this.state, required this.child});

  final BlockableOverlayState state;
  final Widget child;

  @override
  State<IosControlOverlay> createState() => _IosControlOverlayState();
}

class _IosControlOverlayState extends State<IosControlOverlay> {
  late final _entry = OverlayEntry(builder: (_) => widget.child);

  @override
  void didUpdateWidget(IosControlOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    _entry.markNeedsBuild();
  }

  @override
  Widget build(BuildContext context) =>
      Overlay(key: widget.state.key, initialEntries: [_entry]);

  @override
  void dispose() {
    _entry.remove();
    _entry.dispose();
    super.dispose();
  }
}

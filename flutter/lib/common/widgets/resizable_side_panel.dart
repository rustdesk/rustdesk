import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/dragable_divider.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

// Persisted-width option keys for the resizable left panels.
const String kOptionAbTagsPanelWidth = 'ab-tags-panel-width';
const String kOptionAccessibleDevicesPanelWidth = 'accessible-devices-panel-width';

class ResizablePanelController {
  final String optionKey;
  final double defaultWidth;
  final double minWidth;
  final double maxWidth;
  late final RxDouble width;

  ResizablePanelController({
    required this.optionKey,
    required this.defaultWidth,
    this.minWidth = 120,
    this.maxWidth = 300,
  }) {
    final saved = double.tryParse(bind.mainGetLocalOption(key: optionKey));
    width =
        RxDouble((saved ?? defaultWidth).clamp(minWidth, maxWidth).toDouble());
  }

  void _onDrag(double dx) {
    width.value = (width.value + dx).clamp(minWidth, maxWidth).toDouble();
  }

  void _persist() {
    bind.mainSetLocalOption(
        key: optionKey, value: width.value.toStringAsFixed(0));
  }

  Widget buildDivider() {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onHorizontalDragUpdate: (details) => _onDrag(details.delta.dx),
      onHorizontalDragEnd: (_) => _persist(),
      onHorizontalDragCancel: _persist,
      child: DraggableDivider(
        axis: Axis.vertical,
        padding: const EdgeInsets.symmetric(horizontal: 4.0),
        color: Colors.transparent,
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/custom_scale_base.dart';

class MobileCustomScaleControls extends StatefulWidget {
  final FFI ffi;
  final ValueChanged<int>? onChanged;
  const MobileCustomScaleControls({super.key, required this.ffi, this.onChanged});

  @override
  State<MobileCustomScaleControls> createState() => _MobileCustomScaleControlsState();
}

class _MobileCustomScaleControlsState extends CustomScaleControls<MobileCustomScaleControls> {
  @override
  FFI get ffi => widget.ffi;

  @override
  ValueChanged<int>? get onScaleChanged => widget.onChanged;

  @override
  Widget build(BuildContext context) {
    // Smaller button size for mobile
    const smallBtnConstraints = BoxConstraints(minWidth: 32, minHeight: 32);

    final sliderControl = Slider(
      value: scalePos,
      min: 0.0,
      max: 1.0,
      divisions: (CustomScaleControls.maxPercent - CustomScaleControls.minPercent).round(),
      label: '$scaleValue%',
      onChanged: onSliderChanged,
    );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8.0, vertical: 8.0),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            '${translate("Scale custom")}: $scaleValue%',
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              IconButton(
                iconSize: 20,
                padding: const EdgeInsets.all(4),
                constraints: smallBtnConstraints,
                icon: const Icon(Icons.remove),
                tooltip: translate('Decrease'),
                onPressed: () => nudgeScale(-1),
              ),
              Expanded(child: sliderControl),
              IconButton(
                iconSize: 20,
                padding: const EdgeInsets.all(4),
                constraints: smallBtnConstraints,
                icon: const Icon(Icons.add),
                tooltip: translate('Increase'),
                onPressed: () => nudgeScale(1),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

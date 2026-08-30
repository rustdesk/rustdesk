// ignore_for_file: use_super_parameters

import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

import '../models/color_picker_type.dart';

/// A widget used to select the active color picker
///
/// Not library exposed, private to the library.
@immutable
class SelectPicker extends StatelessWidget {
  /// Default const constructor.
  const SelectPicker({
    Key? key,
    required this.pickers,
    required this.pickerLabels,
    required this.picker,
    required this.onPickerChanged,
    this.thumbColor,
    this.textStyle,
    this.columnSpacing = 8,
  }) : super(key: key);

  /// A map of used picker types to select which segments to show and use.
  final Map<ColorPickerType, bool> pickers;

  /// THe labels for the picker segments.
  final Map<ColorPickerType, String> pickerLabels;

  /// Current active picker.
  final ColorPickerType picker;

  /// Callback to change picker type.
  final ValueChanged<ColorPickerType> onPickerChanged;

  /// The thumb color of the selected segment.
  ///
  /// Uses cupertino default light and dark style if not provided.
  final Color? thumbColor;

  /// Text style of the text items in the picker
  ///
  /// If not provided, default to `Theme.of(context).textTheme.bodySmall`.
  final TextStyle? textStyle;

  /// The spacing after the picker. Defaults to 8.
  final double columnSpacing;

  @override
  Widget build(BuildContext context) {
    // Set default text style for the segmented slider control.
    final TextStyle segmentTextStyle = textStyle ??
        Theme.of(context).textTheme.bodySmall ??
        const TextStyle(fontSize: 12);

    final Color effectiveThumbColor = thumbColor ??
        const CupertinoDynamicColor.withBrightness(
          color: Color(0xFFFFFFFF),
          darkColor: Color(0xFF636366),
        );

    final Color? effectiveThumbOnColor = thumbColor == null
        ? null
        : ThemeData.estimateBrightnessForColor(effectiveThumbColor) ==
                Brightness.light
            ? Colors.black
            : Colors.white;

    return SizedBox(
      width: double.infinity,
      child: Padding(
        padding: EdgeInsets.only(bottom: columnSpacing),
        child: CupertinoSlidingSegmentedControl<ColorPickerType>(
          children: <ColorPickerType, Widget>{
            if (pickers[ColorPickerType.both]!)
              ColorPickerType.both: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.both] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.both
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.primary]!)
              ColorPickerType.primary: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.primary] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.primary
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.accent]!)
              ColorPickerType.accent: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.accent] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.accent
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.bw]!)
              ColorPickerType.bw: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.bw] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.bw
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.custom]!)
              ColorPickerType.custom: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.custom] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.custom
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.customSecondary]!)
              ColorPickerType.customSecondary: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.customSecondary] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.customSecondary
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
            if (pickers[ColorPickerType.wheel]!)
              ColorPickerType.wheel: Padding(
                padding: const EdgeInsets.all(5),
                child: Text(
                  pickerLabels[ColorPickerType.wheel] ?? '',
                  textAlign: TextAlign.center,
                  style: picker == ColorPickerType.wheel
                      ? segmentTextStyle.copyWith(color: effectiveThumbOnColor)
                      : segmentTextStyle,
                ),
              ),
          },
          thumbColor: thumbColor ??
              const CupertinoDynamicColor.withBrightness(
                color: Color(0xFFFFFFFF),
                darkColor: Color(0xFF636366),
              ),
          onValueChanged: (ColorPickerType? value) {
            if (value != null) onPickerChanged(value);
          },
          groupValue: picker,
        ),
      ),
    );
  }
}

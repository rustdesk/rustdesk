// ignore_for_file: use_super_parameters

import 'package:flutter/material.dart';

import '../../flex_color_picker.dart';
import '../functions/picker_functions.dart';

/// TonalPaletteColors widget.
///
/// Not library exposed, private to the library.
class TonalPaletteColors extends StatefulWidget {
  /// Default const constructor.
  const TonalPaletteColors({
    Key? key,
    required this.spacing,
    required this.runSpacing,
    required this.selectedColor,
    required this.onSelectColor,
    required this.tonalShouldUpdate,
    required this.width,
    required this.height,
    this.borderRadius,
    required this.hasBorder,
    this.borderColor,
    required this.elevation,
    required this.selectedColorIcon,
    required this.selectedRequestsFocus,
    required this.tonalPaletteFixedMinChroma,
  }) : super(key: key);

  /// The spacing between the color pick items.
  final double spacing;

  /// The run spacing between the color pick items when wrapped on several rows.
  final double runSpacing;

  /// The selected color.
  final Color selectedColor;

  /// Void callback called when a color is selected.
  final ValueChanged<Color> onSelectColor;

  /// Should the internal color to generate the Tonal Palette be updated?
  final bool tonalShouldUpdate;

  /// Width of the color pick item.
  final double width;

  /// Height of the color pick item.
  final double height;

  /// Border radius of the color pick item.
  final double? borderRadius;

  /// Set to true if pick item should have a border.
  final bool hasBorder;

  /// Color of the border when one is used.
  final Color? borderColor;

  /// Material elevation of the color pick item.
  final double elevation;

  /// Icon used to mark selected color.
  final IconData selectedColorIcon;

  /// Set to true, if a an indicator should request focus if it is selected.
  ///
  /// The indicator will always request focus when it clicked and selected,
  /// setting this value to true is to make it request focus when it is drawn.
  /// This is used to set focus to the selected color, but only when
  /// the picker is redrawn.
  ///
  /// Defaults to false.
  final bool selectedRequestsFocus;

  /// Weather to use fixed min chroma for tonal palette.
  final bool tonalPaletteFixedMinChroma;

  @override
  State<TonalPaletteColors> createState() => _TonalPaletteColorsState();
}

class _TonalPaletteColorsState extends State<TonalPaletteColors> {
  late List<Color> tonalColors;

  @override
  void initState() {
    super.initState();
    tonalColors = getTonalColors(
      widget.selectedColor,
      widget.tonalPaletteFixedMinChroma,
    );
  }

  @override
  void didUpdateWidget(TonalPaletteColors oldWidget) {
    if (widget.tonalShouldUpdate) {
      tonalColors = getTonalColors(
        widget.selectedColor,
        widget.tonalPaletteFixedMinChroma,
      );
    }
    super.didUpdateWidget(oldWidget);
  }

  @override
  Widget build(BuildContext context) {
    final double effectiveBorderRadius =
        widget.borderRadius ?? widget.width / 4.0;
    return Wrap(
      spacing: widget.spacing,
      runSpacing: widget.runSpacing,
      children: <Widget>[
        for (final Color color in tonalColors)
          ColorIndicator(
            isSelected: widget.selectedColor == color ||
                widget.selectedColor.value == color.value,
            color: color,
            width: widget.width,
            height: widget.height,
            borderRadius: effectiveBorderRadius,
            hasBorder: widget.hasBorder,
            borderColor: widget.borderColor,
            elevation: widget.elevation,
            selectedIcon: widget.selectedColorIcon,
            onSelect: () {
              widget.onSelectColor(color);
            },
            selectedRequestsFocus: widget.selectedRequestsFocus,
          ),
      ],
    );
  }
}

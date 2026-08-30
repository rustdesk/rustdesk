// ignore_for_file: use_super_parameters

import 'package:flutter/material.dart';

import '../../flex_color_picker.dart';
import '../functions/picker_functions.dart';

/// ShadeColors widget.
///
/// Not library exposed, private to the library.
class ShadeColors extends StatelessWidget {
  /// Default const constructor.
  const ShadeColors({
    Key? key,
    required this.spacing,
    required this.runSpacing,
    required this.columnSpacing,
    required this.activeSwatch,
    required this.selectedColor,
    required this.onSelectColor,
    required this.includeIndex850,
    required this.width,
    required this.height,
    this.borderRadius,
    required this.hasBorder,
    this.borderColor,
    required this.elevation,
    required this.selectedColorIcon,
    required this.selectedRequestsFocus,
  }) : super(key: key);

  /// The spacing between the color pick items.
  final double spacing;

  /// The run spacing between the color pick items when wrapped on several rows.
  final double runSpacing;

  /// The spacing after the main colors.
  final double columnSpacing;

  // /// The currently active used list of color swatches we select color from.
  // final List<ColorSwatch<Object>> activeColorSwatchList;

  /// The active Swatch in the active Color swatch List.
  final ColorSwatch<Object> activeSwatch;

  /// The selected color.
  final Color selectedColor;

  /// Void callback called when a color is selected.
  final ValueChanged<Color> onSelectColor;

  /// Set to trued if index 850 is to be included in the main shades.
  final bool includeIndex850;

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

  @override
  Widget build(BuildContext context) {
    final double effectiveBorderRadius = borderRadius ?? width / 4.0;
    return Padding(
      padding: EdgeInsets.only(bottom: columnSpacing),
      child: Wrap(
        spacing: spacing,
        runSpacing: runSpacing,
        children: <Widget>[
          for (final Color color
              in getMaterialColorShades(activeSwatch, includeIndex850))
            ColorIndicator(
              isSelected:
                  selectedColor == color || selectedColor.value == color.value,
              color: color,
              width: width,
              height: height,
              borderRadius: effectiveBorderRadius,
              hasBorder: hasBorder,
              borderColor: borderColor,
              elevation: elevation,
              selectedIcon: selectedColorIcon,
              onSelect: () {
                onSelectColor(color);
              },
              selectedRequestsFocus: selectedRequestsFocus,
            ),
        ],
      ),
    );
  }
}

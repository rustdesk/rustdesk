import 'package:flex_color_picker/flex_color_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

class WaylandScreenFrameSettings extends StatefulWidget {
  final Widget child;

  const WaylandScreenFrameSettings({super.key, required this.child});

  @override
  State<WaylandScreenFrameSettings> createState() =>
      _WaylandScreenFrameSettingsState();
}

class _WaylandScreenFrameSettingsState
    extends State<WaylandScreenFrameSettings> {
  // Probe once per settings page; a slider rebuild must not reconnect to XWayland.
  late final supported = bind.mainIsScreenFrameSupported();

  @override
  Widget build(BuildContext context) => FutureBuilder<bool>(
        future: supported,
        builder: (_, snapshot) =>
            snapshot.data == true ? widget.child : const SizedBox.shrink(),
      );
}

class ScreenFrameStyleSettings extends StatelessWidget {
  final bool enabled;

  const ScreenFrameStyleSettings({super.key, required this.enabled});

  Color _readColor(String key, Color fallback) {
    final value = bind.mainGetOptionSync(key: key);
    return value.length == 7 && RegExp(r'^#[0-9a-fA-F]{6}$').hasMatch(value)
        ? Color(0xFF000000 | int.parse(value.substring(1), radix: 16))
        : fallback;
  }

  double _readOpacity() {
    // Preserve existing transparency settings and fixed policies while the UI
    // presents opacity. Clamp before subtracting to handle out-of-range values.
    final raw = bind.mainGetOptionSync(key: kOptionScreenFrameTransparency);
    final parsed =
        RegExp(r'^[+-]?[0-9]+$').hasMatch(raw) ? int.tryParse(raw) : null;
    return (100 - (parsed ?? 20).clamp(0, 80)).toDouble();
  }

  @override
  Widget build(BuildContext context) {
    // Share the slider value so both color swatches preview opacity while dragging.
    final opacity = _readOpacity().obs;
    final wayland = bind.mainCurrentIsWayland();
    return Padding(
      padding: const EdgeInsets.only(left: 48, right: 15, bottom: 10),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        _colorPicker(
            context,
            wayland ? 'Frame color' : 'Viewed display frame color',
            kOptionScreenFrameColor,
            MyTheme.accent,
            opacity),
        if (!wayland)
          _colorPicker(
              context,
              'Unviewed display frame color',
              kOptionScreenFrameInactiveColor,
              const Color(0xFF808080),
              opacity),
        const SizedBox(height: 12),
        _slider('Width', kOptionScreenFrameWidth,
            min: 2, max: 20, fallback: 4, unit: isMacOS ? 'pt' : 'px'),
        _slider('Opacity', kOptionScreenFrameTransparency,
            min: 20, max: 100, fallback: 80, unit: '%', currentValue: opacity),
      ]),
    );
  }

  Widget _colorPicker(BuildContext context, String label, String key,
      Color fallback, RxDouble opacity) {
    final color = _readColor(key, fallback).obs;
    return Row(children: [
      Expanded(child: Text('${translate(label)}:')),
      const SizedBox(width: 16),
      Obx(() => OutlinedButton(
            style: OutlinedButton.styleFrom(
              fixedSize: const Size(36, 36),
              minimumSize: const Size(36, 36),
              padding: const EdgeInsets.all(8),
            ),
            onPressed: enabled && !isOptionFixed(key)
                ? () async {
                    final selected = await showColorPickerDialog(
                      context,
                      color.value,
                      pickersEnabled: {
                        ColorPickerType.accent: false,
                        ColorPickerType.wheel: true,
                      },
                      pickerTypeLabels: {
                        ColorPickerType.primary: translate('Primary Color'),
                        ColorPickerType.wheel: translate('HSV Color'),
                      },
                      actionButtons: ColorPickerActionButtons(
                        dialogOkButtonLabel: translate('OK'),
                        dialogCancelButtonLabel: translate('Cancel'),
                      ),
                      showColorCode: true,
                    );
                    if (!context.mounted || selected == color.value) return;
                    await bind.mainSetOption(
                      key: key,
                      value:
                          '#${(selected.value & 0xFFFFFF).toRadixString(16).padLeft(6, '0').toUpperCase()}',
                    );
                    color.value = _readColor(key, fallback);
                  }
                : null,
            child: Semantics(
              label: translate(label),
              child: Container(
                width: 20,
                height: 20,
                decoration: BoxDecoration(
                  color: color.value.withOpacity(opacity.value / 100),
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(color: Theme.of(context).dividerColor),
                ),
              ),
            ),
          )),
    ]).paddingSymmetric(vertical: 4);
  }

  Widget _slider(String label, String key,
      {required int min,
      required int max,
      required int fallback,
      required String unit,
      RxDouble? currentValue}) {
    final isOpacity = key == kOptionScreenFrameTransparency;
    double readValue() {
      if (isOpacity) return _readOpacity();
      final raw = bind.mainGetOptionSync(key: key);
      final parsed =
          RegExp(r'^[+-]?[0-9]+$').hasMatch(raw) ? int.tryParse(raw) : null;
      return (parsed ?? fallback).clamp(min, max).toDouble();
    }

    final value = currentValue ?? readValue().obs;
    final editable = enabled && !isOptionFixed(key);
    return Obx(() => Row(children: [
          SizedBox(width: 80, child: Text('${translate(label)}:')),
          Expanded(
            child: Slider(
              value: value.value,
              min: min.toDouble(),
              max: max.toDouble(),
              divisions: max - min,
              label: '${value.value.round()} $unit',
              onChanged: editable ? (next) => value.value = next : null,
              // Save once on release so dragging does not repeatedly rebuild native windows.
              onChangeEnd: editable
                  ? (next) async {
                      await bind.mainSetOption(
                          key: key,
                          value: (isOpacity ? 100 - next.round() : next.round())
                              .toString());
                      value.value = readValue();
                    }
                  : null,
            ),
          ),
          SizedBox(width: 55, child: Text('${value.value.round()} $unit')),
        ]));
  }
}

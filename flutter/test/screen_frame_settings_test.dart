import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/screen_frame_settings.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final options = <String, String>{};
  bool wayland = false;
  bool screenFrameSupported = true;

  @override
  bool mainCurrentIsWayland({dynamic hint}) => wayland;

  @override
  Future<bool> mainIsScreenFrameSupported({dynamic hint}) async =>
      screenFrameSupported;

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetOptionSync({required String key, dynamic hint}) =>
      options[key] ?? '';

  @override
  bool mainIsOptionFixed({required String key, dynamic hint}) => false;

  @override
  Future<void> mainSetOption(
      {required String key, required String value, dynamic hint}) async {
    options[key] = value;
  }
}

Color swatch(WidgetTester tester, int index) => (tester
        .widget<Container>(find.descendant(
          of: find.byType(OutlinedButton).at(index),
          matching: find.byType(Container),
        ))
        .decoration as BoxDecoration)
    .color!;

void main() {
  late _Binding binding;
  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
  });

  Future<void> open(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: ScreenFrameStyleSettings(key: UniqueKey(), enabled: true),
      ),
    ));
    await tester.pumpAndSettle();
  }

  testWidgets('labels, aligned sliders and default appearance', (tester) async {
    await open(tester);
    expect(find.text('Viewed display frame color:'), findsOneWidget);
    expect(find.text('Unviewed display frame color:'), findsOneWidget);
    expect(find.text('#0071FF'), findsNothing);
    expect(find.text('#808080'), findsNothing);
    expect(find.byType(OutlinedButton), findsNWidgets(2));
    expect(find.text('Width:'), findsOneWidget);
    expect(find.text('Opacity:'), findsOneWidget);
    expect(find.text(Platform.isMacOS ? '4 pt' : '4 px'), findsOneWidget);
    expect(find.text('80 %'), findsOneWidget);
    expect(find.textContaining('Screen frame transparency'), findsNothing);
    expect(binding.options, isEmpty);
    final width = tester.widget<Slider>(find.byType(Slider).first);
    expect([width.min, width.max], [2, 20]);
    expect(swatch(tester, 0).opacity, closeTo(0.8, 1 / 255));
    final widthSlider = tester.getRect(find.byType(Slider).first);
    final opacitySlider = tester.getRect(find.byType(Slider).last);
    expect(widthSlider.left, opacitySlider.left);
    expect(widthSlider.right, opacitySlider.right);
    expect(tester.getCenter(find.text('Width:')).dy, widthSlider.center.dy);
    expect(tester.getCenter(find.text('Opacity:')).dy, opacitySlider.center.dy);
  });

  testWidgets('Wayland shows one frame color using the saved active color',
      (tester) async {
    binding.wayland = true;
    binding.options[kOptionScreenFrameColor] = '#12AB34';
    binding.options[kOptionScreenFrameInactiveColor] = '#ABCDEF';
    await open(tester);
    expect(find.text('Frame color:'), findsOneWidget);
    expect(find.text('Viewed display frame color:'), findsNothing);
    expect(find.text('Unviewed display frame color:'), findsNothing);
    expect(find.byType(OutlinedButton), findsOneWidget);
    expect(swatch(tester, 0), const Color(0xFF12AB34).withOpacity(0.8));
    expect(binding.options[kOptionScreenFrameInactiveColor], '#ABCDEF');
  });

  testWidgets('unavailable XWayland hides the entire screen frame settings',
      (tester) async {
    binding.wayland = true;
    for (final supported in [false, true]) {
      binding.screenFrameSupported = supported;
      await tester.pumpWidget(MaterialApp(
        home: Scaffold(
          body: WaylandScreenFrameSettings(
            key: UniqueKey(),
            child: const Column(children: [
              Text('Enable screen frame'),
              ScreenFrameStyleSettings(enabled: true),
            ]),
          ),
        ),
      ));
      await tester.pumpAndSettle();
      final expected = supported ? findsOneWidget : findsNothing;
      expect(find.text('Enable screen frame'), expected);
      expect(find.text('Frame color:'), expected);
      expect(find.text('Width:'), expected);
      expect(find.text('Opacity:'), expected);
    }
  });

  testWidgets('saved transparency reads as opacity without altering settings',
      (tester) async {
    binding.options[kOptionScreenFrameColor] = '#12AB34';
    for (final entry in {
      '0': 100,
      '25': 75,
      '50': 50,
      '80': 20,
      '-10': 100,
      '99999': 20,
      'invalid': 80,
    }.entries) {
      binding.options[kOptionScreenFrameTransparency] = entry.key;
      await open(tester);
      expect(find.text('${entry.value} %'), findsOneWidget);
      expect(binding.options[kOptionScreenFrameTransparency], entry.key);
      expect(swatch(tester, 0),
          const Color(0xFF12AB34).withOpacity(entry.value / 100));
    }
  });

  testWidgets('opacity slider saves matching transparency and survives reopen',
      (tester) async {
    await open(tester);
    final slider = find.byType(Slider).at(1);
    final rect = tester.getRect(slider);
    for (final sample in [
      [rect.center.dx, 60, 40],
      [rect.left + 10, 20, 80],
      [rect.right - 10, 100, 0],
    ]) {
      await tester.tapAt(Offset(sample[0].toDouble(), rect.center.dy));
      await tester.pumpAndSettle();
      expect(find.text('${sample[1]} %'), findsOneWidget);
      expect(binding.options[kOptionScreenFrameTransparency], '${sample[2]}');
      for (final index in [0, 1]) {
        expect(
            swatch(tester, index).opacity, closeTo(sample[1] / 100, 1 / 255));
      }
      await open(tester);
      expect(find.text('${sample[1]} %'), findsOneWidget);
      expect(find.text(Platform.isMacOS ? '4 pt' : '4 px'), findsOneWidget);
    }
  });
}

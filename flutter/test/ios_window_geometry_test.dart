import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/overlay.dart';
import 'package:flutter_hbb/mobile/widgets/ios_window_geometry.dart';

void main() {
  final event = {
    'bounds': {'x': 0, 'y': 0, 'width': 669, 'height': 951},
    'hingeStatus': 'partiallyOpen',
    'divisions': [
      {
        'frame': {'x': 0, 'y': 455.5, 'width': 669, 'height': 40},
        'margins': {'top': 20, 'left': 0, 'bottom': 20, 'right': 0},
      }
    ],
  };

  test('half-fold display uses the core boundary, controls retain margins', () {
    final geometry = IosWindowGeometry(event);
    final division = geometry.horizontalDivision(const Size(669, 951))!;
    expect(geometry.splitY(division), 475.5);
    expect(division.bottom, 495.5);
    expect(geometry.horizontalDivision(const Size(951, 669)), isNull);
    expect(
        IosWindowGeometry({...event, 'hingeStatus': 'closed'})
            .horizontalDivision(const Size(669, 951)),
        isNull);
  });

  testWidgets('bounded controls update and release their overlay',
      (tester) async {
    final label = ValueNotifier('Controls');
    addTearDown(label.dispose);
    final state = BlockableOverlayState();
    var taps = 0;
    await tester.pumpWidget(MaterialApp(
        home: Stack(children: [
      ValueListenableBuilder<String>(
          valueListenable: label,
          builder: (_, value, __) => Positioned(
              top: value == 'Controls' ? 300 : 0,
              left: 0,
              right: 0,
              height: 250,
              child: IosControlOverlay(
                  state: state,
                  child: Scaffold(
                      bottomNavigationBar: BottomAppBar(
                          child: TextButton(
                              onPressed: () => taps++, child: Text(value))))))),
    ])));
    expect(find.text('Controls'), findsOneWidget);
    state.setMiddleBlocked(true);
    await tester.pump();
    await tester.tap(find.text('Controls'));
    expect(taps, 1);
    final entry = OverlayEntry(
        builder: (_) => const Positioned(
              top: 80,
              left: 20,
              width: 250,
              height: 50,
              child: Material(child: TextField()),
            ));
    state.state!.insert(entry);
    await tester.pump();
    await tester.enterText(find.byType(TextField), 'draft');
    expect(tester.getTopLeft(find.byType(TextField)).dy, 380);
    label.value = 'Keyboard';
    await tester.pump();
    expect(find.text('Keyboard'), findsOneWidget);
    expect(find.text('Controls'), findsNothing);
    expect(find.text('draft'), findsOneWidget);
    expect(tester.getTopLeft(find.byType(TextField)).dy, 80);
    entry.remove();
    entry.dispose();
    await tester.pumpWidget(const SizedBox());
    expect(tester.takeException(), isNull);
  });
}

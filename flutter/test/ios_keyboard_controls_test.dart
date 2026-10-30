import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/ios_keyboard_controls.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  Widget app({bool halfFold = true, VoidCallback? remoteTap}) => MaterialApp(
    theme: ThemeData(useMaterial3: false),
    home: MediaQuery(
      data: const MediaQueryData(
        size: Size(669, 455.5),
        padding: EdgeInsets.only(bottom: 34),
      ),
      child: Scaffold(
        body: GestureDetector(
          onTap: remoteTap,
          child: Stack(
            children: [
              const Positioned.fill(child: ColoredBox(color: Colors.black)),
              if (halfFold)
                Positioned.fill(
                  child: IosKeyboardControls(
                    auxiliaryBuilder: (requested) => requested
                        ? const SizedBox(height: 132, child: Text('Ctrl'))
                        : const SizedBox.shrink(),
                    toolbarBuilder: (collapse, toggleAuxiliary) => BottomAppBar(
                      child: Row(
                        children: [
                          const Text('Tools'),
                          IconButton(
                            tooltip: 'Auxiliary keys',
                            onPressed: toggleAuxiliary,
                            icon: const Icon(Icons.keyboard),
                          ),
                          IconButton(
                            tooltip: 'Collapse',
                            onPressed: collapse,
                            icon: const Icon(Icons.expand_more),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    ),
  );

  testWidgets('keyboard controls expand on request without forwarding taps', (
    tester,
  ) async {
    var remoteTaps = 0;
    await tester.pumpWidget(app(remoteTap: () => remoteTaps++));
    expect(find.text('Tools'), findsNothing);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('Tools'), findsOneWidget);
    expect(tester.getSize(find.byTooltip('Collapse')).height, 48);
    expect(tester.getSize(find.byType(BottomAppBar)).height, 48);
    expect(find.text('Ctrl'), findsNothing);
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    expect(find.text('Ctrl'), findsOneWidget);
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    expect(find.text('Ctrl'), findsNothing);
    await tester.tap(find.byTooltip('Collapse'));
    await tester.pump();
    expect(find.text('Tools'), findsNothing);
    expect(find.byType(FloatingActionButton), findsOneWidget);
    expect(remoteTaps, 0);
  });

  testWidgets('a new half-fold starts with only the floating button', (
    tester,
  ) async {
    await tester.pumpWidget(app());
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('Tools'), findsOneWidget);
    await tester.pumpWidget(app(halfFold: false));
    await tester.pumpWidget(app());
    expect(find.text('Tools'), findsNothing);
    expect(find.byType(FloatingActionButton), findsOneWidget);
  });
}

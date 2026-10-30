import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/ios_keyboard_controls.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  Widget app({bool halfFold = true, VoidCallback? remoteTap,
    VoidCallback? hideKeyboard}) => MaterialApp(
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
                    onHideKeyboard: hideKeyboard,
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
    await tester.tapAt(const Offset(100, 100));
    expect(remoteTaps, 1);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('Tools'), findsOneWidget);
    expect(tester.getSize(find.byTooltip('Collapse')).height, 48);
    expect(tester.getSize(find.byType(BottomAppBar)).height, 48);
    expect(find.text('Ctrl'), findsNothing);
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    expect(find.text('Ctrl'), findsOneWidget);
    expect(tester.getTopLeft(find.text('Ctrl')).dy, 0);
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    expect(find.text('Ctrl'), findsNothing);
    await tester.tap(find.byTooltip('Collapse'));
    await tester.pump();
    expect(find.text('Tools'), findsNothing);
    expect(find.byType(FloatingActionButton), findsOneWidget);
    expect(remoteTaps, 1);
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

  testWidgets('keyboard dismissal is independent of auxiliary controls', (
    tester,
  ) async {
    var hidden = 0;
    var remoteTaps = 0;
    await tester.pumpWidget(app(
      hideKeyboard: () => hidden++, remoteTap: () => remoteTaps++));
    await tester.tap(find.byIcon(Icons.keyboard_hide));
    expect(hidden, 1);
    expect(remoteTaps, 0);
    expect(find.text('Tools'), findsNothing);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    await tester.tap(find.byTooltip('Collapse'));
    await tester.pump();
    await tester.tap(find.byIcon(Icons.keyboard_hide));
    expect(hidden, 2);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('Ctrl'), findsOneWidget);
    await tester.tap(find.byTooltip('Collapse'));
    await tester.pumpWidget(app());
    expect(find.byIcon(Icons.keyboard_hide), findsNothing);
    expect(remoteTaps, 0);
  });
}

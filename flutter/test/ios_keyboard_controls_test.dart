import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/widgets/ios_keyboard_controls.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  Widget app(
          {bool halfFold = true,
          VoidCallback? remoteTap,
          bool keyboardVisible = true,
          VoidCallback? toggleKeyboard}) =>
      MaterialApp(
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
                    Positioned(
                      top: keyboardVisible ? 0 : 300,
                      left: 0,
                      right: 0,
                      height: 300,
                      child: IosKeyboardControls(
                        keyboardVisible: keyboardVisible,
                        auxiliaryBuilder: (requested) => requested
                            ? const SizedBox(height: 132, child: Text('Ctrl'))
                            : const SizedBox.shrink(),
                        toolbarBuilder: (collapse, toggleAuxiliary) =>
                            BottomAppBar(
                          child: Row(
                            children: [
                              const Text('Tools'),
                              IconButton(
                                tooltip: 'Auxiliary keys',
                                onPressed: toggleAuxiliary,
                                icon: const Icon(Icons.keyboard_command_key),
                              ),
                              if (toggleKeyboard != null)
                                IconButton(
                                  onPressed: toggleKeyboard,
                                  icon: Icon(keyboardVisible
                                      ? Icons.keyboard_hide
                                      : Icons.keyboard),
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

  testWidgets(
      'keyboard toggle moves the same controls without losing selection', (
    tester,
  ) async {
    var toggled = 0;
    var remoteTaps = 0;
    await tester.pumpWidget(app(
        keyboardVisible: false,
        toggleKeyboard: () => toggled++,
        remoteTap: () => remoteTaps++));
    expect(find.text('Tools'), findsOneWidget);
    await tester.tap(find.byTooltip('Auxiliary keys'));
    await tester.pump();
    expect(tester.getTopLeft(find.text('Ctrl')).dy, 300);
    await tester.tap(find.byIcon(Icons.keyboard));
    expect(toggled, 1);
    await tester.pumpWidget(
        app(toggleKeyboard: () => toggled++, remoteTap: () => remoteTaps++));
    expect(find.text('Tools'), findsNothing);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('Ctrl'), findsOneWidget);
    expect(tester.getTopLeft(find.text('Ctrl')).dy, 0);
    await tester.tap(find.byIcon(Icons.keyboard_hide));
    expect(toggled, 2);
    await tester.pumpWidget(app(
        keyboardVisible: false,
        toggleKeyboard: () => toggled++,
        remoteTap: () => remoteTaps++));
    expect(find.text('Tools'), findsOneWidget);
    expect(find.text('Ctrl'), findsOneWidget);
    expect(tester.getTopLeft(find.text('Ctrl')).dy, 300);
    expect(find.byIcon(Icons.keyboard_hide), findsNothing);
    expect(remoteTaps, 0);
  });
}

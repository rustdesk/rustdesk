import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/mobile/widgets/session_overlay.dart';
import 'package:flutter_hbb/mobile/services/foldable_layout.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('keyboard remains open when session content and insets update',
      (tester) async {
    final focus = FocusNode();
    addTearDown(focus.dispose);
    Widget page(bool editing, double inset) => MaterialApp(
            home: MediaQuery(
          data: MediaQueryData(viewInsets: EdgeInsets.only(bottom: inset)),
          child: Scaffold(
              resizeToAvoidBottomInset: false,
              body: SessionOverlay(
                  child: Stack(children: [
                if (editing)
                  SizedBox(
                      width: 0, height: 0, child: TextField(focusNode: focus))
              ]))),
        ));
    await tester.pumpWidget(page(false, 0));
    await tester.pumpWidget(page(true, 0));
    focus.requestFocus();
    await tester.pump();
    expect(tester.testTextInput.isVisible, isTrue);
    await tester.enterText(find.byType(TextField), 'hello');
    expect(tester.testTextInput.isVisible, isTrue);
    await tester.pumpWidget(page(true, 250));
    await tester.pump();
    expect(tester.testTextInput.isVisible, isTrue);
    expect(find.text('hello'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    expect(tester.takeException(), isNull);
  });

  testWidgets('native pose updates never replace a chosen session tree',
      (tester) async {
    final layout = FoldableSessionLayout();
    final overlayKey = GlobalKey<OverlayState>();
    final focus = FocusNode();
    addTearDown(focus.dispose);
    Widget page() => MaterialApp(
        home: !layout.ready
            ? const SizedBox()
            : layout.current.supported
                ? Scaffold(
                    body: SessionOverlay(
                        overlayKey: overlayKey,
                        child: TextField(focusNode: focus)))
                : const Scaffold(body: Text('Original mobile session')));
    await tester.pumpWidget(page());
    expect(find.byType(TextField), findsNothing);
    layout.update(const FoldableLayout(supported: true, size: Size(626, 890)));
    await tester.pumpWidget(page());
    await tester.tap(find.byType(TextField));
    await tester.enterText(find.byType(TextField), 'retained');
    final dialog = OverlayEntry(
        builder: (_) => const AlertDialog(title: Text('Connection error')));
    overlayKey.currentState!.insert(dialog);
    for (var i = 0; i < 5; i++) {
      layout.update(FoldableLayout(
          supported: i.isOdd,
          size: i.isOdd ? const Size(890, 626) : const Size(626, 890)));
      await tester.pumpWidget(page());
      await tester.pump();
      expect(find.text('retained'), findsOneWidget);
      expect(find.text('Connection error'), findsOneWidget);
    }
    dialog.remove();
    dialog.dispose();
    await tester.pumpAndSettle();
    expect(tester.testTextInput.isVisible, isTrue);
    await tester.pumpWidget(const SizedBox());
    expect(tester.takeException(), isNull);
    final ordinary = FoldableSessionLayout()..update(const FoldableLayout());
    ordinary.update(const FoldableLayout(supported: true));
    expect(ordinary.current.supported, isFalse);
  });

  testWidgets('connection dialogs use full session bounds and dismiss',
      (tester) async {
    final state = OverlayKeyState();
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: SessionOverlay(
      overlayKey: state.key,
      child: const Stack(children: [
        Positioned(
            right: 0,
            top: 0,
            bottom: 0,
            width: 48,
            child: ColoredBox(color: Colors.grey))
      ]),
    ))));
    final manager = OverlayDialogManager();
    manager.setOverlayState(state);
    for (var i = 0; i < 5; i++) {
      final result = manager.show<bool>((setState, close, context) =>
          CustomAlertDialog(
              title: const Text('Connection error'),
              content: const Text('Please log in first and try again.'),
              actions: [
                TextButton(
                    onPressed: () => close(true), child: const Text('OK'))
              ]));
      await tester.pumpAndSettle();
      expect(tester.getSize(find.byType(AlertDialog)).width, greaterThan(300));
      await tester.tap(find.text('OK'));
      await tester.pumpAndSettle();
      expect(await result, isTrue);
      expect(find.text('Connection error'), findsNothing);
    }
    await tester.pumpWidget(const SizedBox());
    expect(tester.takeException(), isNull);
  });
}

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/offstage_focus_policy.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('Tab skips buttons hidden by Offstage', (tester) async {
    final first = FocusNode();
    final hidden = FocusNode();
    final last = FocusNode();
    await tester.pumpWidget(MaterialApp(
      home: FocusTraversalGroup(
        policy: SkipOffstageTraversalPolicy(),
        child: Column(children: [
          TextButton(focusNode: first, onPressed: () {}, child: const Text('A')),
          Offstage(
            child: TextButton(
                focusNode: hidden, onPressed: () {}, child: const Text('B')),
          ),
          // Keeps C clearly below B so reading order is A, B, C.
          const SizedBox(height: 100),
          TextButton(focusNode: last, onPressed: () {}, child: const Text('C')),
        ]),
      ),
    ));
    first.requestFocus();
    await tester.pump();

    first.nextFocus();
    await tester.pump();
    expect(last.hasFocus, isTrue);
    expect(hidden.hasFocus, isFalse);

    last.previousFocus();
    await tester.pump();
    expect(first.hasFocus, isTrue);
  });

  testWidgets('Tab skips kept-alive pages that are not shown', (tester) async {
    final a1 = FocusNode();
    final a2 = FocusNode();
    final b1 = FocusNode();
    final controller = PageController();
    await tester.pumpWidget(MaterialApp(
      home: FocusTraversalGroup(
        policy: SkipOffstageTraversalPolicy(),
        child: PageView(controller: controller, children: [
          _KeptAlivePage(children: [
            TextButton(focusNode: a1, onPressed: () {}, child: const Text('A1')),
            TextButton(focusNode: a2, onPressed: () {}, child: const Text('A2')),
          ]),
          _KeptAlivePage(children: [
            TextButton(focusNode: b1, onPressed: () {}, child: const Text('B1')),
          ]),
        ]),
      ),
    ));
    // Visit page B so it is built and kept alive, then return to page A.
    controller.jumpToPage(1);
    await tester.pump();
    controller.jumpToPage(0);
    await tester.pump();
    a2.requestFocus();
    await tester.pump();

    a2.nextFocus();
    await tester.pump();
    expect(b1.hasFocus, isFalse);
    expect(a1.hasFocus, isTrue);
  });
}

class _KeptAlivePage extends StatefulWidget {
  final List<Widget> children;
  const _KeptAlivePage({required this.children});

  @override
  State<_KeptAlivePage> createState() => _KeptAlivePageState();
}

class _KeptAlivePageState extends State<_KeptAlivePage>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Column(children: widget.children);
  }
}

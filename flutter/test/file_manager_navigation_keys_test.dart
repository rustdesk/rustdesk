import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/list_search_action_listener.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_test/flutter_test.dart';

List<Entry> _entries(int count) => [
      for (var i = 0; i < count; i++)
        Entry()
          ..name = 'f$i'
          ..path = '/f$i'
    ];

List<String> _names(List<Entry> entries) =>
    entries.map((e) => e.name).toList()..sort();

void main() {
  test('navigation keys move the selection and stop at the ends', () {
    final entries = _entries(5);
    expect(_names(navigateSelection(entries, [], 1, false)), ['f0']);
    expect(_names(navigateSelection(entries, [], -1, false)), ['f4']);
    var selected = navigateSelection(entries, [entries[1]], 1, false);
    expect(_names(selected), ['f2']);
    selected = navigateSelection(entries, selected, -1, false);
    expect(_names(selected), ['f1']);
    expect(_names(navigateSelection(entries, selected, -3, false)), ['f0']);
    expect(_names(navigateSelection(entries, selected, 5, false)), ['f4']);
  });

  test('shift extends and shrinks the range from the anchor', () {
    final entries = _entries(5);
    var selected = [entries[2]];
    selected = navigateSelection(entries, selected, 1, true);
    selected = navigateSelection(entries, selected, 1, true);
    expect(_names(selected), ['f2', 'f3', 'f4']);
    selected = navigateSelection(entries, selected, -1, true);
    selected = navigateSelection(entries, selected, -1, true);
    selected = navigateSelection(entries, selected, -1, true);
    expect(_names(selected), ['f1', 'f2']);
    selected = navigateSelection(entries, selected, 1, false);
    expect(_names(selected), ['f2']);
  });

  test('page keys go to the visible edge before paging', () {
    // Rows 10..19 are fully visible.
    expect(pageDelta(12, 10, 19, true), 7);
    expect(pageDelta(19, 10, 19, true), 9);
    expect(pageDelta(12, 10, 19, false), -2);
    expect(pageDelta(10, 10, 19, false), -9);
  });

  testWidgets('Enter does not reach type-to-search', (tester) async {
    final node = FocusNode();
    addTearDown(node.dispose);
    final searched = <String>[];
    await tester.pumpWidget(MaterialApp(
      home: ListSearchActionListener(
        node: node,
        buffer: TimeoutStringBuffer(),
        onNext: searched.add,
        onSearch: searched.add,
        child: const SizedBox(),
      ),
    ));
    node.requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter, character: '\r');
    expect(searched, isEmpty);
  });
}

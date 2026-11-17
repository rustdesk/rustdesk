import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/api_server_warning.dart';

void main() {
  test('warns only for remote HTTP endpoints', () {
    final cases = <String?, bool>{
      null: false,
      '': false,
      'https://api.example.com': false,
      'http://api.example.com': true,
      'http://192.168.1.13:13000': true,
      'http://localhost:21114': false,
      'http://127.0.0.1:21114': false,
      'http://[::1]:21114': false,
    };
    for (final entry in cases.entries) {
      expect(shouldWarnApiServer(entry.key), entry.value, reason: entry.key);
    }
  });

  testWidgets('warning follows API and ID changes using the resolved endpoint',
      (tester) async {
    final api = TextEditingController();
    final id = TextEditingController(text: 'id.example.com');
    addTearDown(api.dispose);
    addTearDown(id.dispose);
    await tester.pumpWidget(MaterialApp(
      home: ApiServerWarning(
        controller: api,
        idController: id,
        message: 'HTTP warning',
        emptyMessage: 'Default HTTP warning',
        resolveApiServer: ({required api, required custom}) =>
            api.isNotEmpty ? api : 'http://$custom',
      ),
    ));
    expect(find.text('Default HTTP warning'), findsOneWidget);

    id.text = 'localhost';
    await tester.pump();
    expect(find.text('Default HTTP warning'), findsNothing);

    api.text = 'http://api.example.com';
    await tester.pump();
    expect(find.text('HTTP warning'), findsOneWidget);

    api.text = 'https://api.example.com';
    await tester.pump();
    expect(find.text('HTTP warning'), findsNothing);
  });

  testWidgets('resolver failures are reported without breaking the server form',
      (tester) async {
    final api = TextEditingController();
    final id = TextEditingController();
    addTearDown(api.dispose);
    addTearDown(id.dispose);
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Column(children: [
          TextField(controller: api),
          Expanded(
              child: ApiServerWarning(
            controller: api,
            idController: id,
            message: 'HTTP warning',
            emptyMessage: 'Default HTTP warning',
            resolveApiServer: ({required api, required custom}) =>
                throw const FormatException('API preview failed'),
          )),
        ]),
      ),
    ));
    expect(tester.takeException(), isA<FormatException>());
    expect(find.byType(ErrorWidget), findsNothing);
    expect(find.text('Default HTTP warning'), findsNothing);
    expect(find.byType(TextField), findsOneWidget);
  });
}

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Binding extends Fake implements RustdeskImpl {
  late Completer<String> printers = Completer<String>();
  final canceled = <int>[];

  @override
  bool isOutgoingOnly({dynamic hint}) => true;
  @override
  bool isIncomingOnly({dynamic hint}) => true;
  @override
  bool isDisableSettings({dynamic hint}) => false;
  @override
  bool isDisableAccount({dynamic hint}) => true;
  @override
  bool mainIsInstalled({dynamic hint}) => false;
  @override
  bool mainIsOptionFixed({required String key, dynamic hint}) => false;
  @override
  String mainGetBuildinOption({required String key, dynamic hint}) =>
      key == kOptionHideRemotePrinterSetting ? '' : 'Y';

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetLocalOption({required String key, dynamic hint}) => '';

  @override
  String mainGetPrinterNames({dynamic hint}) => '["Office"]';

  @override
  String mainGetAppNameSync({dynamic hint}) => 'RustDesk';

  @override
  Future<String> mainGetCommon({required String key, dynamic hint}) {
    if (key == 'is-pdf-printer-installed') return Future.value('false');
    if (key == 'is-remote-modify-enabled-by-control-permissions') {
      return Future.value('true');
    }
    expect(key, 'pdf-printer-names');
    return printers.future;
  }

  @override
  String mainGetCommonSync({required String key, dynamic hint}) =>
      key == 'is-support-printer-driver'
          ? 'true'
          : throw StateError('Printer discovery must not run on the UI thread');

  @override
  Future<void> sessionCancelJob({
    required UuidValue sessionId,
    required int actId,
    dynamic hint,
  }) async =>
      canceled.add(actId);
}

class _Session extends Fake implements FFI {
  @override
  final sessionId = UuidValue('00000000-0000-4000-8000-000000000001');
  @override
  final dialogManager = OverlayDialogManager();
  @override
  bool closed = false;
}

void main() {
  late _Binding binding;
  late _Session session;
  late FfiModel model;
  var taps = 0;

  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    session = _Session();
    model = FfiModel(WeakReference(session));
    taps = 0;
  });

  tearDown(() {
    session.dialogManager.dismissAll();
    model.dispose();
  });

  Future<void> openRequest(WidgetTester tester, {bool pdf = true}) async {
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      home: Scaffold(
        body:
            TextButton(onPressed: () => taps++, child: const Text('Navigate')),
      ),
    ));
    await model.startEventListener(session.sessionId, 'peer')({
      'name': 'printer_request',
      'id': 1,
      'path': 'RustDesk://FsJob//${pdf ? 'PdfPrinter' : 'Printer'}/1',
    });
    await tester.pump();
  }

  testWidgets('printer settings refresh added and removed printers',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    binding.printers.complete('{"printers":["Old printer"]}');
    await tester.pumpWidget(MaterialApp(
      home: DesktopSettingPage(initialTabkey: SettingsTabKey.printer),
    ));
    await tester.pumpAndSettle();
    expect(tester.widget<ComboBox>(find.byType(ComboBox)).keys,
        ['', 'Old printer']);

    binding.printers = Completer<String>();
    await tester.tap(find.byTooltip('Refresh'));
    await tester.pump();
    binding.printers.complete('{"printers":["New printer"]}');
    await tester.pumpAndSettle();
    expect(tester.widget<ComboBox>(find.byType(ComboBox)).keys,
        ['', 'New printer']);
  });

  testWidgets('legacy XPS requests keep their existing confirmation workflow',
      (tester) async {
    await openRequest(tester, pdf: false);
    await tester.pumpAndSettle();
    expect(find.text('Office'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(binding.canceled, isEmpty);
  });

  testWidgets('PDF discovery leaves the UI responsive before confirmation',
      (tester) async {
    await openRequest(tester);
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Navigate'));
    expect(taps, 1);
    expect(find.text('Incoming Print Job'), findsNothing);

    binding.printers.complete('{"printers":["Office"]}');
    await tester.pumpAndSettle();
    expect(find.text('Office'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(binding.canceled, [1]);
  });

  testWidgets('a closed session does not show a delayed print dialog',
      (tester) async {
    await openRequest(tester);
    session.closed = true;
    binding.printers.complete('{"printers":["Office"]}');
    await tester.pumpAndSettle();
    expect(find.text('Incoming Print Job'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('PDF enumeration failure remains visible and cancelable',
      (tester) async {
    await openRequest(tester);
    binding.printers.completeError(StateError('Print service unavailable'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Print service unavailable'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(binding.canceled, [1]);
  });
}

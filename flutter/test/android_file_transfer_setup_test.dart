import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mobile/widgets/android_file_transfer_setup.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final options = <String, String>{};

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) =>
      options[k] ?? '';

  @override
  Future<void> setLocalFlutterOption(
      {required String k, required String v, dynamic hint}) async {
    options[k] = v;
  }

  @override
  bool isDisableSettings({dynamic hint}) => false;

  @override
  bool isOutgoingOnly({dynamic hint}) => false;

  @override
  String mainGetBuildinOption({required String key, dynamic hint}) => '';
}

class _Server extends Fake implements ServerModel {
  @override
  List<Client> get clients => [];
}

class _Session extends Fake implements FFI {
  @override
  final dialogManager = OverlayDialogManager();

  @override
  final serverModel = _Server();
}

void main() {
  late _Session session;

  setUp(() {
    platformFFI.setBindingForTesting(_Binding());
    session = _Session();
  });

  Future<void> openApp(WidgetTester tester) => tester.pumpWidget(MaterialApp(
        navigatorKey: globalKey,
        home: const Scaffold(),
      ));

  testWidgets('skipping folder setup suppresses later prompts', (tester) async {
    await openApp(tester);
    final first = showAndroidFileTransferSetup(session);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Skip'));
    await tester.pumpAndSettle();
    expect(await first, isTrue);
    expect(await showAndroidFileTransferSetup(_Session()), isTrue);
    expect(find.byType(CustomAlertDialog), findsNothing);
  });

  testWidgets('canceling setup leaves it available at the next entry',
      (tester) async {
    await openApp(tester);
    final first = showAndroidFileTransferSetup(session);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(await first, isFalse);

    final retry = showAndroidFileTransferSetup(session);
    await tester.pumpAndSettle();
    expect(find.byType(CustomAlertDialog), findsOneWidget);
    await tester.tap(find.text('Skip'));
    await tester.pumpAndSettle();
    expect(await retry, isTrue);
  });
}

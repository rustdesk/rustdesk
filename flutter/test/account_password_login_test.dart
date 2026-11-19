import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  String hidePasswordLogin = '';
  String? authenticatedProvider;
  Future<String?>? loginOptionsResponse;

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name.replaceAll('{', '').replaceAll('}', '');

  @override
  String mainGetBuildinOption({required String key, dynamic hint}) =>
      key == kOptionHideAccountPasswordLogin ? hidePasswordLogin : '';

  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      key == kOptionEnableFlutterHttpOnRust ? 'Y' : '';

  @override
  String getLocalFlutterOption({required String k, dynamic hint}) => '';

  @override
  bool isDisableAb({dynamic hint}) => false;

  @override
  bool isDisableAccount({dynamic hint}) => false;

  @override
  bool isDisableGroupPanel({dynamic hint}) => false;

  @override
  Future<String> mainGetApiServer({dynamic hint}) async =>
      'https://example.com';

  @override
  Future<void> mainHttpRequest(
      {required String url,
      required String method,
      String? body,
      required String header,
      dynamic hint}) async {
    expectSync(Uri.parse(url).path, '/api/login-options');
  }

  @override
  Future<String?> mainGetHttpStatus(
          {required String url, dynamic hint}) async =>
      await loginOptionsResponse ??
      jsonEncode({'body': '["oidc/azure"]', 'headers': {}, 'status_code': 200});

  @override
  Future<void> mainAccountAuth(
      {required String op, required bool rememberMe, dynamic hint}) async {
    authenticatedProvider = op;
  }

  @override
  Future<void> mainAccountAuthCancel({dynamic hint}) async {}

  @override
  Future<String> mainAccountAuthResult({dynamic hint}) async => '';
}

void main() {
  final binding = _Binding();

  setUpAll(() async {
    isTest = true;
    platformFFI.setBindingForTesting(binding);
    await initGlobalFFI();
  });

  Future<void> openLogin(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      home: const Scaffold(),
    ));
    loginDialog();
    addTearDown(gFFI.dialogManager.dismissAll);
    await tester.pumpAndSettle();
  }

  for (final option in ['', 'N', 'Y']) {
    testWidgets('account password visibility with option "$option"',
        (tester) async {
      binding.hidePasswordLogin = option;
      binding.authenticatedProvider = null;
      await openLogin(tester);
      final hidden = option == 'Y';
      expect(find.byType(TextField), hidden ? findsNothing : findsNWidgets(2));
      expect(find.widgetWithText(ElevatedButton, 'Login'),
          hidden ? findsNothing : findsOneWidget);
      expect(find.text('or'), hidden ? findsNothing : findsOneWidget);
      expect(find.text('Continue with Microsoft'), findsOneWidget);
      final dialog =
          tester.widget<CustomAlertDialog>(find.byType(CustomAlertDialog));
      expect(dialog.onSubmit, hidden ? isNull : isNotNull);
      await tester.tap(find.text('Continue with Microsoft'));
      await tester.pump();
      expect(binding.authenticatedProvider, 'azure');
      gFFI.dialogManager.dismissAll();
      await tester.pumpAndSettle();
    });
  }

  testWidgets('OIDC-only login shows loading before an empty provider response',
      (tester) async {
    binding.hidePasswordLogin = 'Y';
    final response = Completer<String?>();
    binding.loginOptionsResponse = response.future;
    addTearDown(() async {
      binding.loginOptionsResponse = null;
      gFFI.dialogManager.dismissAll();
      await tester.pumpAndSettle();
    });
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      home: const Scaffold(),
    ));
    loginDialog();
    final unavailable = find.text('username-login-disabled-oidc-unavailable');

    // Render the first frame before the zero-delay fetch timer runs.
    await tester.pump();
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    expect(unavailable, findsNothing);

    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    expect(unavailable, findsNothing);

    response.complete(
        jsonEncode({'body': '[]', 'headers': {}, 'status_code': 200}));
    await tester.pumpAndSettle();
    expect(find.byType(LinearProgressIndicator), findsNothing);
    expect(unavailable, findsOneWidget);
    expect(find.text('Retry'), findsNothing);
  });
}

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Binding extends Fake implements RustdeskImpl {
  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  Future<void> sessionOnWaitingForImageDialogShow(
      {required UuidValue sessionId, dynamic hint}) async {}
}

class _Session extends Fake implements FFI {
  @override
  final SessionID sessionId = const Uuid().v4obj();
}

void main() {
  testWidgets('duplicate waiting dialogs are fully removed when cleared',
      (tester) async {
    platformFFI.setBindingForTesting(_Binding());
    final session = _Session();
    final model = FfiModel(WeakReference(session));
    final dialogs = OverlayDialogManager();
    await tester.pumpWidget(MaterialApp(
      navigatorKey: globalKey,
      home: const Scaffold(),
    ));

    model.showConnectedWaitingForImage(dialogs, session.sessionId, 'success',
        'Successful', kMsgboxTextWaitingForImage);
    await tester.pumpAndSettle();
    expect(find.byType(CustomAlertDialog), findsOneWidget);

    // Automatic Windows session selection repeats the PeerInfo waiting event.
    model.showConnectedWaitingForImage(dialogs, session.sessionId, 'success',
        'Successful', kMsgboxTextWaitingForImage);
    await tester.pumpAndSettle();

    clearWaitingForImage(dialogs, session.sessionId);
    model.waitForFirstImage.value = false;
    model.clear();
    await tester.pumpAndSettle(const Duration(seconds: 2));
    expect(find.byType(CustomAlertDialog), findsNothing);
  });
}

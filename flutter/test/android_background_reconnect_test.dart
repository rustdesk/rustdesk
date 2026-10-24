import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

final _sessionId = UuidValue('00000000-0000-4000-8000-000000000002');

class _Binding extends Fake implements RustdeskImpl {
  int reconnects = 0;

  @override
  Future<void> sessionReconnect(
      {required UuidValue sessionId,
      required bool forceRelay,
      dynamic hint}) async {
    reconnects++;
  }

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;
}

class _Dialogs extends OverlayDialogManager {
  int shown = 0;

  @override
  Future<T?> show<T>(DialogBuilder builder,
      {bool clickMaskDismiss = false,
      bool backDismiss = false,
      String? tag,
      bool useAnimation = true,
      bool forceGlobal = false}) {
    shown++;
    return Future.value();
  }
}

class _Input extends Fake implements InputModel {
  @override
  bool setRelativeMouseMode(bool enabled) => true;
}

class _Session extends Fake implements FFI {
  @override
  UuidValue get sessionId => _sessionId;
  @override
  bool closed = false;
  @override
  final _Dialogs dialogManager = _Dialogs();
  @override
  final _Input inputModel = _Input();
  @override
  late final ffiModel = FfiModel(WeakReference(this));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('a DNS failure in the background shows no dialog and reconnects on resume',
      () {
    final binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    final session = _Session();
    final model = session.ffiModel;

    model.onAndroidAppPaused();
    model.handleMsgBox({
      'type': 'error',
      'title': 'Connection Error',
      'text': 'failed to lookup address information: '
          'No address associated with hostname',
      'link': '',
      'hasRetry': '',
    }, _sessionId, 'peer');
    expect(session.dialogManager.shown, 0);
    expect(binding.reconnects, 0);

    model.onAndroidAppResumed();
    expect(binding.reconnects, 1);
    model.dispose();
  });
}

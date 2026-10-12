import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

final _sessionId = UuidValue('00000000-0000-0000-0000-000000000000');

class _FakeFFI implements FFI {
  @override
  UuidValue get sessionId => _sessionId;

  @override
  late final FfiModel ffiModel = FfiModel(WeakReference(this));

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  test('the quality monitor reports the current connection mode', () {
    final ffi = _FakeFFI();
    final model = QualityMonitorModel(WeakReference(ffi));

    expect(model.connectionType, isNull);

    ffi.ffiModel.setConnectionType('peer', true, true, 'UDP');
    expect(
        model.connectionType,
        '${translate("Direct and encrypted connection")} (UDP)');

    ffi.ffiModel.setConnectionType('peer', true, false, 'Relay');
    expect(
        model.connectionType,
        '${translate("Relayed and encrypted connection")} (TCP)');
  });
}

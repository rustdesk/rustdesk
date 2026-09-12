import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _FakeFFI implements FFI {
  @override
  final sessionId = UuidValue('00000000-0000-0000-0000-000000000000');

  @override
  late final FfiModel ffiModel = FfiModel(WeakReference(this));

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  test('clearing a reused session resets keyboard capture status', () {
    final ffi = _FakeFFI();
    final model = ffi.ffiModel;
    addTearDown(model.dispose);

    expect(model.keyboardGrabbed, isFalse);
    model.keyboardGrabbed = true;
    model.clear();
    expect(model.keyboardGrabbed, isFalse);
  });
}

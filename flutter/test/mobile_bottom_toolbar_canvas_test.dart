import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Binding extends Fake implements RustdeskImpl {
  @override
  Future<String?> sessionGetViewStyle(
          {required UuidValue sessionId, dynamic hint}) async =>
      kRemoteViewStyleAdaptive;
}

class _FFI extends Fake implements FFI {
  _FFI() {
    ffiModel = FfiModel(WeakReference(this));
    cursorModel = CursorModel(WeakReference(this));
    canvasModel = CanvasModel(WeakReference(this));
  }
  @override
  final SessionID sessionId = const Uuid().v4obj();
  @override
  late final FfiModel ffiModel;
  @override
  late final CursorModel cursorModel;
  @override
  late final CanvasModel canvasModel;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('adaptive scale fits the remote screen above the mobile toolbar',
      () async {
    final wasMobile = isMobile;
    isMobile = true;
    addTearDown(() => isMobile = wasMobile);
    platformFFI.setBindingForTesting(_Binding());
    final ffi = _FFI();
    final canvas = ffi.canvasModel;
    double imageBottom() =>
        canvas.y + ffi.canvasModel.getDisplayHeight() * canvas.scale;

    await canvas.updateViewStyle();
    final viewHeight = canvas.size.height;
    final bottomWithoutToolbar = imageBottom();

    const toolbarHeight = 56.0;
    const epsilon = 1e-6;
    canvas.setMobileBottomBarHeight(toolbarHeight);
    await Future<void>.delayed(Duration.zero);
    expect(
        imageBottom(), lessThanOrEqualTo(viewHeight - toolbarHeight + epsilon));

    canvas.setMobileBottomBarHeight(0);
    await Future<void>.delayed(Duration.zero);
    expect(imageBottom(), closeTo(bottomWithoutToolbar, epsilon));
  });
}

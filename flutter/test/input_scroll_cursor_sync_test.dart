import 'dart:convert';
import 'package:flutter/gestures.dart';
import 'package:flutter_hbb/common.dart' show SessionID;
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart' hide Display;
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Bind extends Fake implements RustdeskImpl {
  final sent = <Map<String, dynamic>>[];
  @override Future<void> sessionSendMouse({required SessionID sessionId, required String msg, dynamic hint}) async => sent.add(json.decode(msg) as Map<String, dynamic>);
  @override Future<void> setCurSessionId({required SessionID sessionId, dynamic hint}) async {}
  @override String mainGetInputSource({dynamic hint}) => 'flutter';
  @override Future<void> sessionEnterOrLeave({required SessionID sessionId, required bool enter, dynamic hint}) async {}
  @override bool sessionGetToggleOptionSync({required SessionID sessionId, required String arg, dynamic hint}) => false;
  @override Future<String?> sessionGetViewStyle({required SessionID sessionId, dynamic hint}) async => null;
  @override Future<void> sessionSetSize({required SessionID sessionId, required int display, required int width, required int height, dynamic hint}) async {}
}

class _Session extends Fake implements FFI {
  @override bool closed = false;
  @override final String id = 'test-peer';
  @override final SessionID sessionId = const Uuid().v4obj();
  @override ConnType get connType => ConnType.defaultConn;
  @override late final FfiModel ffiModel = FfiModel(WeakReference(this))..pi.platform = kPeerPlatformLinux..pi.displays.add(Display()..width = 1920..height = 1080);
  @override late final CanvasModel canvasModel = CanvasModel(WeakReference(this))..updateSize();
  @override late final CursorModel cursorModel = CursorModel(WeakReference(this))..gotMouseControl = true;
  @override late final InputModel inputModel = InputModel(WeakReference(this));
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('syncs cursor before initial wheel and trackpad scroll once per entry', () async {
    final testBind = _Bind();
    platformFFI.setBindingForTesting(testBind);
    final session = _Session();
    await session.ffiModel.updateCurDisplay(session.sessionId);
    session.canvasModel.update(0, 0, 1.0);
    final input = session.inputModel;
    addTearDown(() {
      session.closed = true;
      input..enterOrLeave(false)..disposeSideButtonTracking()..disposeRelativeMouseMode();
      session.ffiModel.dispose();
      platformFFI.setBindingForTesting(_Bind());
    });

    input.enterOrLeave(true);
    input.onPointerSignalImage(const PointerScrollEvent(position: Offset(640, 420), scrollDelta: Offset(0, 120)));
    expect([testBind.sent.map((e) => e['type']).toList(), testBind.sent.first['x'], testBind.sent.first['y']], [['', 'wheel'], '${(640 - CanvasModel.leftToEdge).toInt()}', '${(420 - CanvasModel.topToEdge).toInt()}']);

    testBind.sent.clear();
    input.onPointerSignalImage(const PointerScrollEvent(position: Offset(640, 420), scrollDelta: Offset(0, 120)));
    expect(testBind.sent.map((e) => e['type']).toList(), ['wheel']);

    testBind.sent.clear();
    session.canvasModel.update(0, 0, 1.0);
    input..enterOrLeave(true)..onPointerPanZoomStart(const PointerPanZoomStartEvent(position: Offset(800, 300)));
    input.onPointerPanZoomUpdate(const PointerPanZoomUpdateEvent(position: Offset(800, 300), pan: Offset(0, 40), panDelta: Offset(0, 40)));
    expect([testBind.sent.map((e) => e['type']).toList(), testBind.sent.first['x'], testBind.sent.first['y']], [['', 'trackpad'], '${(800 - CanvasModel.leftToEdge).toInt()}', '${(300 - CanvasModel.topToEdge).toInt()}']);
  });
}

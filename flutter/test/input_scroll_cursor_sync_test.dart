import 'dart:convert';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _FakeBind extends Fake implements RustdeskImpl {
  final List<Map<String, dynamic>> sentMouse = [];

  @override
  Future<void> sessionSendMouse({
    required UuidValue sessionId,
    required String msg,
    dynamic hint,
  }) async {
    sentMouse.add(json.decode(msg) as Map<String, dynamic>);
  }

  @override
  void setCurSessionId({required UuidValue sessionId, dynamic hint}) {}

  @override
  bool sessionGetToggleOptionSync({
    required UuidValue sessionId,
    required String arg,
    dynamic hint,
  }) =>
      false;
}

class _FakeCanvas extends Fake implements CanvasModel {
  @override
  final double devicePixelRatio = 1.0;
  @override
  final double scale = 1.0;
  @override
  final double x = 0.0;
  @override
  final double y = 0.0;
  @override
  final ScrollStyle scrollStyle = ScrollStyle.scrollauto;
  @override
  final Size size = const Size(1920, 1080);
  @override
  double getAdjustY() => 0.0;
  @override
  void moveDesktopMouse(double x, double y) {}
  @override
  void edgeScrollMouse(double x, double y) {}
  @override
  void updateLocalCursor(double x, double y) {}
}

class _FakeCursor extends Fake implements CursorModel {
  @override
  bool isPeerControlProtected = false;
  @override
  bool gotMouseControl = true;
}

class _FakePeer extends Fake implements FfiModel {
  @override
  final pi = PeerInfo()
    ..platform = kPeerPlatformLinux
    ..version = '1.5.3';
  @override
  bool get keyboard => true;
  @override
  bool get viewOnly => false;
  @override
  bool get showMyCursor => false;
  @override
  ui.Rect? get rect => const ui.Rect.fromLTWH(0, 0, 1920, 1080);
}

class _FakeFFI extends Fake implements FFI {
  @override
  final String id = 'test-peer';
  @override
  final SessionID sessionId = const Uuid().v4obj();
  @override
  final ConnType connType = ConnType.defaultConn;
  @override
  final bool closed = false;
  @override
  final CanvasModel canvasModel = _FakeCanvas();
  @override
  final CursorModel cursorModel = _FakeCursor();
  @override
  final FfiModel ffiModel = _FakePeer();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late _FakeBind fakeBind;
  late InputModel inputModel;

  setUp(() {
    fakeBind = _FakeBind();
    platformFFI.ffiBind = fakeBind;
    final ffi = _FakeFFI();
    inputModel = InputModel(WeakReference<FFI>(ffi));
    inputModel.enterOrLeave(true);
    fakeBind.sentMouse.clear();
  });

  test('syncs cursor position before initial wheel scroll and only once per entry',
      () {
    const pos = Offset(640, 420);
    inputModel.onPointerSignalImage(
      const PointerScrollEvent(
        position: pos,
        scrollDelta: Offset(0, 120),
      ),
    );

    expect(fakeBind.sentMouse, hasLength(2));
    expect(fakeBind.sentMouse['type'], '');
    expect(fakeBind.sentMouse['x'], '640');
    expect(fakeBind.sentMouse['y'], '420');
    expect(fakeBind.sentMouse['type'], 'wheel');
    expect(fakeBind.sentMouse['y'], '-1');

    // Subsequent scroll without leaving should not emit another synthetic move.
    fakeBind.sentMouse.clear();
    inputModel.onPointerSignalImage(
      const PointerScrollEvent(
        position: pos,
        scrollDelta: Offset(0, 120),
      ),
    );
    expect(fakeBind.sentMouse, hasLength(1));
    expect(fakeBind.sentMouse['type'], 'wheel');
  });

  test('syncs cursor position before initial precision touchpad scroll', () {
    const pos = Offset(800, 300);
    inputModel.onPointerPanZoomStart(
      const PointerPanZoomStartEvent(position: pos),
    );
    inputModel.onPointerPanZoomUpdate(
      const PointerPanZoomUpdateEvent(
        position: pos,
        pan: Offset(0, 40),
        panDelta: Offset(0, 40),
      ),
    );

    expect(fakeBind.sentMouse, hasLength(2));
    expect(fakeBind.sentMouse['type'], '');
    expect(fakeBind.sentMouse['x'], '800');
    expect(fakeBind.sentMouse['y'], '300');
    expect(fakeBind.sentMouse['type'], 'trackpad');
    expect(fakeBind.sentMouse['y'], '2');
  });
}

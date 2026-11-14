import 'dart:convert';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Bind extends Fake implements RustdeskImpl {
  final sent = <Map<String, dynamic>>[];
  @override
  Future<void> sessionSendMouse({required UuidValue sessionId, required String msg, dynamic hint}) async =>
      sent.add(json.decode(msg) as Map<String, dynamic>);
  @override
  Future<void> setCurSessionId({required UuidValue sessionId, dynamic hint}) async {}
  @override
  String mainGetInputSource({dynamic hint}) => 'flutter';
  @override
  bool mainCurrentIsWayland({dynamic hint}) => false;
  @override
  int versionToNumber({required String v, dynamic hint}) => int.tryParse(v.replaceAll('.', '')) ?? 0;
  @override
  String translate({required String name, required String locale, dynamic hint}) => name;
  @override
  Future<void> sessionEnterOrLeave({required UuidValue sessionId, required bool enter, dynamic hint}) async {}
  @override
  bool sessionGetToggleOptionSync({required UuidValue sessionId, required String arg, dynamic hint}) => false;
}

class _Canvas extends Fake implements CanvasModel {
  @override double get devicePixelRatio => 1.0;
  @override double get scale => 1.0;
  @override double get x => 0.0;
  @override double get y => 0.0;
  @override double get scrollX => 0.0;
  @override double get scrollY => 0.0;
  @override Offset get imageOffset => Offset.zero;
  @override ScrollStyle get scrollStyle => ScrollStyle.scrollauto;
  @override Size get size => const Size(1920, 1080);
  @override double getAdjustY() => 0.0;
  @override void moveDesktopMouse(double x, double y) {}
  @override void edgeScrollMouse(double x, double y) {}
  @override void updateLocalCursor(double x, double y) {}
}

class _Cursor extends Fake implements CursorModel {
  @override bool isPeerControlProtected = false;
  @override bool gotMouseControl = true;
}

class _Peer extends Fake implements FfiModel {
  @override final pi = PeerInfo()..platform = kPeerPlatformWindows..version = '1.5.3';
  @override bool get keyboard => true;
  @override bool get viewOnly => false;
  @override bool get showMyCursor => false;
  @override ui.Rect? get rect => const ui.Rect.fromLTWH(0, 0, 1920, 1080);
}

class _FFI extends Fake implements FFI {
  @override String get id => 'p';
  @override final UuidValue sessionId = const Uuid().v4obj();
  @override ConnType get connType => ConnType.defaultConn;
  @override bool get closed => false;
  @override final CanvasModel canvasModel = _Canvas();
  @override final CursorModel cursorModel = _Cursor();
  @override final FfiModel ffiModel = _Peer();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('syncs cursor before initial wheel and trackpad scroll unless relative mouse is enabled', () async {
    final bind = _Bind();
    platformFFI.setBindingForTesting(bind);
    final model = InputModel(WeakReference<FFI>(_FFI()));

    // 1. Wheel scroll syncs once upon entry.
    model.enterOrLeave(true);
    model.onPointerSignalImage(const PointerScrollEvent(position: Offset(640, 420), scrollDelta: Offset(0, 120)));
    expect(bind.sent.map((e) => e['type']).toList(), ['', 'wheel']);
    expect([bind.sent[0]['x'], bind.sent[0]['y']], ['634', '386']);

    bind.sent.clear();
    model.onPointerSignalImage(const PointerScrollEvent(position: Offset(640, 420), scrollDelta: Offset(0, 120)));
    expect(bind.sent.map((e) => e['type']).toList(), ['wheel']);

    // 2. Touchpad pan-zoom scroll syncs once upon re-entry.
    bind.sent.clear();
    model.enterOrLeave(true);
    model.onPointerPanZoomStart(const PointerPanZoomStartEvent(position: Offset(800, 300)));
    model.onPointerPanZoomUpdate(const PointerPanZoomUpdateEvent(position: Offset(800, 300), pan: Offset(0, 40), panDelta: Offset(0, 40)));
    expect(bind.sent.map((e) => e['type']).toList(), ['', 'trackpad']);
    expect([bind.sent[0]['x'], bind.sent[0]['y']], ['794', '266']);

    // 3. Relative mouse mode skips synthetic move on scroll.
    model.enterOrLeave(true);
    model.relativeMouseMode.value = true;
    bind.sent.clear();
    model.onPointerSignalImage(const PointerScrollEvent(position: Offset(640, 420), scrollDelta: Offset(0, 120)));
    expect(bind.sent.map((e) => e['type']).toList(), ['wheel']);
  });
}

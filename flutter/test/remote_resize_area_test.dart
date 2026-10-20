import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/remote_resize_area.dart';
import 'package:flutter_hbb/desktop/pages/remote_window_resize.dart';
import 'package:flutter_hbb/models/model.dart';

class _ConnectingCanvas extends ChangeNotifier implements CanvasModel {
  @override
  ViewStyle get viewStyle => ViewStyle.defaultViewStyle();
  @override
  int getDisplayWidth() => 1920;
  @override
  int getDisplayHeight() => 1080;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  const channel = MethodChannel('mixin.one/flutter_multi_window');
  String? requestedCorner;

  setUp(() {
    requestedCorner = null;
    RemoteWindowResize.enabled.value = true;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'startResizing') {
        requestedCorner = (call.arguments as Map)['resizeEdge'] as String;
      }
      return true;
    });
  });

  tearDown(() {
    RemoteWindowResize.enabled.value = false;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  Future<void> showArea(WidgetTester tester) async {
    await tester.binding.setSurfaceSize(const Size(800, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(const MaterialApp(
      home: RemoteResizeArea(
        windowId: 1,
        resizeEdgeSize: 8,
        childPadding: EdgeInsets.zero,
        child: Material(child: Center(child: TextField())),
      ),
    ));
  }

  testWidgets('each half-edge shows and starts its nearest diagonal corner',
      (tester) async {
    await showArea(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer();
    final points = <Offset, String>{
      Offset(2, 150): 'topLeft',
      Offset(2, 450): 'bottomLeft',
      Offset(798, 150): 'topRight',
      Offset(798, 450): 'bottomRight',
      Offset(200, 2): 'topLeft',
      Offset(600, 2): 'topRight',
      Offset(200, 598): 'bottomLeft',
      Offset(600, 598): 'bottomRight',
    };
    for (final point in points.entries) {
      await mouse.moveTo(point.key);
      await tester.pump();
      final cursor = point.value == 'topLeft' || point.value == 'bottomRight'
          ? SystemMouseCursors.resizeUpLeftDownRight
          : SystemMouseCursors.resizeUpRightDownLeft;
      expect(RendererBinding.instance.mouseTracker.debugDeviceActiveCursor(1),
          cursor);
      await mouse.down(point.key);
      await tester.pump();
      expect(requestedCorner, point.value);
      await mouse.up();
      await tester.pump(const Duration(milliseconds: 350));
    }
    await mouse.removePointer();
  });

  testWidgets('leaving adaptive mode preserves content and restores edge drags',
      (tester) async {
    await showArea(tester);
    await tester.enterText(find.byType(TextField), 'session content');
    RemoteWindowResize.enabled.value = false;
    await tester.pump();
    expect(find.text('session content'), findsOneWidget);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(2, 150));
    await mouse.down(const Offset(2, 150));
    await tester.pump();
    expect(requestedCorner, 'left');
    await mouse.up();
    await mouse.removePointer();
    await tester.pump(const Duration(milliseconds: 350));
  });

  testWidgets(
      'a connecting tab disables both diagonal resizing and native sizing',
      (tester) async {
    const geometryChannel =
        MethodChannel('org.rustdesk.rustdesk/remote-window-resize');
    double? ratio;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(geometryChannel, (call) async {
      ratio = (call.arguments as List).first as double;
      return null;
    });
    addTearDown(
        () => messenger.setMockMethodCallHandler(geometryChannel, null));
    final canvas = _ConnectingCanvas();
    final resize = RemoteWindowResize(canvas, () => true, null);
    addTearDown(canvas.dispose);
    addTearDown(resize.dispose);
    await tester.pump();
    expect(ratio, 0.0);
    expect(RemoteWindowResize.enabled.value, isFalse);
  }, skip: !isWindows);
}

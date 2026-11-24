import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'src/channels.dart';
import 'src/backend.dart';
import 'src/window_controller.dart';
import 'src/window_controller_impl.dart';
import 'src/window_listener.dart';

export 'src/window_listener.dart';
export 'src/window_controller.dart';
export 'src/widgets/sub_drag_to_resize_area.dart';

class DesktopMultiWindow {
  /// Create a new Window.
  ///
  /// The new window instance will call `main` method in your `main.dart` file in
  /// new flutter engine instance with some addiotonal arguments.
  /// the arguments of `main` method is a fixed length list.
  /// ---------------------------------------------------------
  /// | index |   Type   |        description                 |
  /// |-------|----------| -----------------------------------|
  /// | 0     | `String` | the value always is "multi_window".|
  /// | 1     | `int`    | the id of the window.              |
  /// | 2     | `String` | the [arguments] of the window.     |
  /// ---------------------------------------------------------
  ///
  /// You can use [WindowController] to control the window.
  ///
  /// NOTE: [createWindow] will only create a new window, you need to call
  /// [WindowController.show] to show the window.
  static Future<WindowController> createWindow([String? arguments]) async {
    if (experimentalWindowBackend != null) return experimentalWindowBackend!.createWindow(arguments);
    final windowId = await miltiWindowChannel.invokeMethod<int>(
      'createWindow',
      arguments,
    );
    assert(windowId != null, 'windowId is null');
    assert(windowId! > 0, 'id must be greater than 0');
    return WindowControllerMainImpl(windowId!);
  }

  /// Invoke method on the isolate of the window.
  ///
  /// Need use [setMethodHandler] in the target window isolate to handle the
  /// method.
  ///
  /// [targetWindowId] which window you want to invoke the method.
  static Future<dynamic> invokeMethod(int targetWindowId, String method,
      [dynamic arguments]) {
    if (experimentalWindowBackend != null) return experimentalWindowBackend!.invokeMethod(targetWindowId, method, arguments);
    return windowEventChannel.invokeMethod(method, <String, dynamic>{
      'targetWindowId': targetWindowId,
      'arguments': arguments,
    });
  }

  /// Add a method handler to the isolate of the window.
  ///
  /// NOTE: you can only handle this window event in this window engine isoalte.
  /// for example: you can not receive the method call which target window isn't
  /// main window in main window isolate.
  ///
  static void setMethodHandler(
      Future<dynamic> Function(MethodCall call, int fromWindowId)? handler) {
    if (experimentalWindowBackend != null) {
      experimentalWindowBackend!.setMethodHandler(handler);
      return;
    }
    if (handler == null) {
      windowEventChannel.setMethodCallHandler(null);
      return;
    }
    windowEventChannel.setMethodCallHandler((call) async {
      if (call.method != 'onEvent') {
        final fromWindowId = call.arguments['fromWindowId'] as int;
        final arguments = call.arguments['arguments'];
        final result =
            await handler(MethodCall(call.method, arguments), fromWindowId);
        return result;
      } else {
        // window event
        _windowMethodCallHandler(call);
      }
    });
  }

  /// Get all sub window id.
  static Future<List<int>> getAllSubWindowIds() async {
    if (experimentalWindowBackend != null) return experimentalWindowBackend!.getAllSubWindowIds();
    try {
      final result = await miltiWindowChannel
          .invokeMethod<List<dynamic>>('getAllSubWindowIds');
      final ids = result
              ?.map<int>((id) => id.toInt())
              .where((id) => id != 0)
              ?.toList() ??
          [];
      assert(ids.every((id) => id > 0), 'id must be greater than 0');
      return ids;
    } catch (e) {
      print('Unreachable, plugin exception, func getAllSubWindowIds(), $e');
      return [];
    }
  }

  static final ObserverList<MultiWindowListener> _listeners =
      ObserverList<MultiWindowListener>();

  static Future<void> _windowMethodCallHandler(MethodCall call) async {
    for (final MultiWindowListener listener in listeners) {
      if (!_listeners.contains(listener)) {
        return;
      }

      if (call.method != 'onEvent') throw UnimplementedError();
      // {'fromWindowId': xxx, arguments: {'eventName':xxx, 'windowId': xxx}}
      String eventName = call.arguments["arguments"]['eventName'].toString();
      listener.onWindowEvent(eventName);
      Map<String, Function> funcMap = {
        kWindowEventClose: listener.onWindowClose,
        kWindowEventFocus: listener.onWindowFocus,
        kWindowEventBlur: listener.onWindowBlur,
        kWindowEventMaximize: listener.onWindowMaximize,
        kWindowEventUnmaximize: listener.onWindowUnmaximize,
        kWindowEventMinimize: listener.onWindowMinimize,
        kWindowEventRestore: listener.onWindowRestore,
        kWindowEventResize: listener.onWindowResize,
        kWindowEventResized: listener.onWindowResized,
        kWindowEventMove: listener.onWindowMove,
        kWindowEventMoved: listener.onWindowMoved,
        kWindowEventEnterFullScreen: listener.onWindowEnterFullScreen,
        kWindowEventLeaveFullScreen: listener.onWindowLeaveFullScreen,
      };
      funcMap[eventName]?.call();
    }
  }

  static List<MultiWindowListener> get listeners {
    if (experimentalWindowBackend != null) return experimentalWindowBackend!.listeners;
    final List<MultiWindowListener> localListeners =
        List<MultiWindowListener>.from(_listeners);
    return localListeners;
  }

  static bool get hasListeners {
    if (experimentalWindowBackend != null) return experimentalWindowBackend!.listeners.isNotEmpty;
    return _listeners.isNotEmpty;
  }

  static void addListener(MultiWindowListener listener) {
    if (experimentalWindowBackend != null) {
      experimentalWindowBackend!.addListener(listener);
      return;
    }
    _listeners.add(listener);
  }

  static void removeListener(MultiWindowListener listener) {
    if (experimentalWindowBackend != null) {
      experimentalWindowBackend!.removeListener(listener);
      return;
    }
    _listeners.remove(listener);
  }
}

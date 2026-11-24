// ignore_for_file: implementation_imports, invalid_use_of_internal_member

import 'dart:async';
import 'dart:io';
import 'dart:ui' show AppExitType;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter/src/foundation/_features.dart' show isWindowingEnabled;
import 'package:flutter/src/widgets/_window.dart' as official;
import 'package:flutter/src/widgets/_window_win32.dart'
    show WindowControllerWin32;

import 'src/backend.dart';
import 'src/window_controller.dart';
import 'src/window_listener.dart';
import 'src/widgets/sub_drag_to_resize_area.dart';

export 'src/window_controller.dart';

/// Explicit per-window state; Dart statics and plugin instances remain shared.
class ExperimentalWindowContext {
  ExperimentalWindowContext._(this.windowId, this.arguments);
  final int windowId;
  final String? arguments;
  final Map<String, Object?> state = {};

  T run<T>(T Function() action) =>
      runZoned(action, zoneValues: {_windowZoneKey: windowId});
}

final Object _windowZoneKey = Object();
const _win32 = MethodChannel('rustdesk/desktop_multi_window/win32');
typedef ExperimentalWindowBuilder = Widget Function(
  ExperimentalWindowContext window,
);

/// Flutter 3.50.1 Windows PoC. Importing this library alone does not opt in.
class ExperimentalWindowingBackend implements DesktopWindowBackend {
  final ValueNotifier<int> _updates = ValueNotifier<int>(0);
  ExperimentalWindowingBackend._(this.builder);
  final ExperimentalWindowBuilder builder;
  final Map<int, _Entry> _entries = {};
  int _nextId = 1;
  int get currentWindowId => Zone.current[_windowZoneKey] as int? ?? 0;

  static ExperimentalWindowingBackend install({
    required ExperimentalWindowBuilder builder,
  }) {
    WidgetsFlutterBinding.ensureInitialized();
    if (!Platform.isWindows || !isWindowingEnabled) {
      throw UnsupportedError(
        'This PoC requires Windows, Flutter 3.50.1 and FLUTTER_WINDOWING=true.',
      );
    }
    if (experimentalWindowBackend != null)
      throw StateError('Backend already installed');
    final backend = ExperimentalWindowingBackend._(builder);
    backend._entries[0] = _Entry(ExperimentalWindowContext._(0, null));
    unawaited(backend._show(0));
    experimentalWindowBackend = backend;
    return backend;
  }

  /// Mount with runWidget, never runApp. Each Window supplies its own FlutterView.
  Widget get root => ListenableBuilder(
    listenable: _updates,
    builder: (context, _) => ViewCollection(
      views: [
        for (final entry in _entries.values)
          if (entry.native != null && !entry.native!.isDestroyed)
            official.Window(
              key: ValueKey(entry.context.windowId),
              controller: entry.native!,
              child: entry.context.run(() => builder(entry.context)),
            ),
      ],
    ),
  );

  ExperimentalWindowContext contextFor(int id) => _entry(id).context;
  int viewIdFor(int id) => _native(id).rootView.viewId;
  _Entry _entry(int id) =>
      _entries[id] ?? (throw StateError('Window $id is closed or unknown'));
  official.WindowController _native(int id) =>
      _entry(id).native ?? (throw StateError('Window $id has not been shown'));

  @override
  Future<WindowController> createWindow(String? arguments) async {
    final id = _nextId++;
    _entries[id] = _Entry(ExperimentalWindowContext._(id, arguments));
    return controller(id);
  }

  @override
  WindowController controller(int id) {
    _entry(id);
    return _Controller(this, id);
  }

  Future<void> _show(int id) async {
    final entry = _entry(id);
    if (entry.native == null) {
      entry.native = official.WindowController(
        size: entry.size,
        title: entry.title,
        delegate: _Delegate(this, id),
      );
      entry.native!.addListener(() => _changed(id));
      final showGeneration = ++entry.pendingShow;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!entry.native!.isDestroyed && entry.pendingShow == showGeneration) {
          unawaited(_control<void>(id, 'show'));
        }
      });
      _updates.value++;
      final frame = entry.stagedFrame;
      if (frame != null) {
        await _setFrame(id, frame);
        entry.stagedFrame = null;
      }
    } else {
      await _control<void>(id, 'show');
    }
  }

  Future<T?> _control<T>(
    int id,
    String method, [
    Map<String, int> args = const {},
  ]) {
    final native = _native(id) as WindowControllerWin32;
    return _win32.invokeMethod<T>(method, {
      'hwnd': native.windowHandle.address,
      ...args,
    });
  }

  Future<void> _setFrame(int id, Rect frame) async {
    await _control<void>(id, 'setFrame', {
      'x': frame.left.toInt(),
      'y': frame.top.toInt(),
      'width': frame.width.toInt(),
      'height': frame.height.toInt(),
    });
  }

  void _changed(int id) {
    final entry = _entries[id];
    if (entry == null || entry.native == null) return;
    final native = entry.native!;
    if (native.isDestroyed) return;
    final values = [
      native.isActivated,
      native.isMaximized,
      native.isMinimized,
      native.isFullscreen,
    ];
    final events = [
      [kWindowEventBlur, kWindowEventFocus],
      [kWindowEventUnmaximize, kWindowEventMaximize],
      [kWindowEventRestore, kWindowEventMinimize],
      [kWindowEventLeaveFullScreen, kWindowEventEnterFullScreen],
    ];
    for (var i = 0; i < values.length; i++) {
      if (values[i] != entry.flags[i]) _emit(id, events[i][values[i] ? 1 : 0]);
    }
    entry.flags = values;
    if (native.contentSize != entry.size) {
      entry.size = native.contentSize;
      _emit(id, kWindowEventResize);
    }
  }

  void _emit(int id, String event) {
    final entry = _entry(id);
    entry.context.run(() {
      for (final listener in List<MultiWindowListener>.of(entry.listeners)) {
        listener.onWindowEvent(event);
        switch (event) {
          case kWindowEventClose:
            listener.onWindowClose();
            break;
          case kWindowEventFocus:
            listener.onWindowFocus();
            break;
          case kWindowEventBlur:
            listener.onWindowBlur();
            break;
          case kWindowEventMaximize:
            listener.onWindowMaximize();
            break;
          case kWindowEventUnmaximize:
            listener.onWindowUnmaximize();
            break;
          case kWindowEventMinimize:
            listener.onWindowMinimize();
            break;
          case kWindowEventRestore:
            listener.onWindowRestore();
            break;
          case kWindowEventResize:
            listener.onWindowResize();
            break;
          case kWindowEventEnterFullScreen:
            listener.onWindowEnterFullScreen();
            break;
          case kWindowEventLeaveFullScreen:
            listener.onWindowLeaveFullScreen();
            break;
        }
      }
    });
  }

  void _close(int id) {
    final entry = _entry(id);
    _emit(id, kWindowEventClose);
    if (entry.preventClose) return;
    if (entry.native == null) {
      _destroyed(id);
    } else {
      entry.native!.destroy();
    }
  }

  void _destroyed(int id) {
    final entry = _entries.remove(id);
    if (entry == null) return;
    entry.handler = null;
    entry.listeners.clear();
    entry.context.state.clear();
    // Native destruction may be delivered inside a controller notification.
    scheduleMicrotask(() {
      entry.native?.dispose();
      _updates.value++;
      if (id == 0)
        ServicesBinding.instance.exitApplication(AppExitType.required);
    });
  }

  dynamic _copy(dynamic value) => const StandardMessageCodec().decodeMessage(
    const StandardMessageCodec().encodeMessage(value),
  );

  @override
  Future<dynamic> invokeMethod(
    int target,
    String method,
    dynamic arguments,
  ) async {
    final source = currentWindowId;
    _entry(source);
    final entry = _entry(target);
    final handler = entry.handler;
    if (handler == null)
      throw MissingPluginException('No handler for window $target');
    final result = await entry.context.run(
      () => handler(MethodCall(method, _copy(arguments)), source),
    );
    return _copy(result);
  }

  @override
  void setMethodHandler(WindowMethodHandler? handler) =>
      _entry(currentWindowId).handler = handler;
  @override
  Future<List<int>> getAllSubWindowIds() async =>
      _entries.keys.where((id) => id != 0).toList();
  @override
  List<MultiWindowListener> get listeners =>
      List.of(_entry(currentWindowId).listeners);
  @override
  void addListener(MultiWindowListener listener) =>
      _entry(currentWindowId).listeners.add(listener);
  @override
  void removeListener(MultiWindowListener listener) =>
      _entry(currentWindowId).listeners.remove(listener);
}

class _Entry {
  _Entry(this.context);
  final ExperimentalWindowContext context;
  official.WindowController? native;
  WindowMethodHandler? handler;
  final List<MultiWindowListener> listeners = [];
  bool preventClose = false;
  int pendingShow = 0;
  Rect? stagedFrame;
  String title = 'RustDesk official windowing PoC';
  Size size = const Size(640, 420);
  List<bool> flags = [false, false, false, false];
}

class _Delegate with official.WindowControllerDelegate {
  _Delegate(this.backend, this.id);
  final ExperimentalWindowingBackend backend;
  final int id;
  @override
  void onWindowCloseRequested(official.WindowController controller) =>
      backend._close(id);
  @override
  void onWindowDestroyed() => backend._destroyed(id);
}

class _Controller extends WindowController {
  _Controller(this.backend, this.windowId);
  final ExperimentalWindowingBackend backend;
  @override
  final int windowId;
  _Entry get entry => backend._entry(windowId);
  official.WindowController get native => backend._native(windowId);
  Never _unsupported(String operation) => throw UnsupportedError(
    '$operation has no equivalent in the Flutter 3.50.1 generic Windowing API',
  );
  @override
  Future<void> show() async => backend._show(windowId);
  @override
  Future<void> close() async => backend._close(windowId);
  @override
  Future<void> focus() async => backend._control<void>(windowId, 'focus');
  @override
  Future<void> maximize() async => native.setMaximized(true);
  @override
  Future<void> unmaximize() async => native.setMaximized(false);
  @override
  Future<void> minimize() async => native.setMinimized(true);
  @override
  Future<bool> isMaximized() async => native.isMaximized;
  @override
  Future<bool> isMinimized() async => native.isMinimized;
  @override
  Future<bool> isFullScreen() async => native.isFullscreen;
  @override
  Future<void> setFullscreen(bool fullscreen) async =>
      native.setFullscreen(fullscreen);
  @override
  Future<void> setTitle(String title) async {
    entry.title = title;
    entry.native?.setTitle(title);
  }

  @override
  Future<bool> isPreventClose() async => entry.preventClose;
  @override
  Future<void> setPreventClose(bool value) async => entry.preventClose = value;
  @override
  Future<void> hide() async {
    entry.pendingShow++;
    if (entry.native != null) await backend._control<void>(windowId, 'hide');
  }

  @override
  Future<bool> isHidden() async =>
      entry.native == null ||
      (await backend._control<bool>(windowId, 'isHidden'))!;
  @override
  Future<void> setFrame(Rect frame) async {
    if (![
          frame.left,
          frame.top,
          frame.width,
          frame.height,
        ].every((v) => v.isFinite && v.abs() <= 0x7fffffff) ||
        frame.width < 1 ||
        frame.height < 1) {
      throw ArgumentError.value(
        frame,
        'frame',
        'Expected a positive Win32 window rectangle',
      );
    }
    if (entry.native == null) {
      entry.stagedFrame = frame;
      return;
    }
    await backend._setFrame(windowId, frame);
  }

  @override
  Future<Rect> getFrame() async {
    if (entry.native == null && entry.stagedFrame != null)
      return entry.stagedFrame!;
    final rect = (await backend._control<Map<Object?, Object?>>(
      windowId,
      'getFrame',
    ))!;
    return Rect.fromLTWH(
      rect['x'] as double,
      rect['y'] as double,
      rect['width'] as double,
      rect['height'] as double,
    );
  }

  @override
  Future<void> center() async => _unsupported('center');
  @override
  Future<void> showTitleBar(bool show) async => _unsupported('showTitleBar');
  @override
  Future<void> startDragging() async => _unsupported('startDragging');
  @override
  Future<void> startResizing(SubWindowResizeEdge edge) async =>
      _unsupported('startResizing');
  @override
  Future<void> setMovable(bool value) async => _unsupported('setMovable');
  @override
  Future<void> setFrameAutosaveName(String name) async =>
      _unsupported('setFrameAutosaveName');
  @override
  Future<int> getXID() async => _unsupported('getXID');
  @override
  Future<void> setInitBackgroundColor(Color color) async =>
      _unsupported('setInitBackgroundColor');
}

// ignore_for_file: avoid_web_libraries_in_flutter

import 'dart:convert';
import 'dart:js_interop';
import 'dart:js_util' as js_util;
import 'dart:typed_data';
import 'dart:js';
import 'dart:html';
import 'dart:async';
import 'dart:ui' as ui;
import 'dart:ui_web' as ui_web;

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/models/state_model.dart';

import 'package:flutter_hbb/web/bridge.dart';
import 'package:flutter_hbb/common.dart';
import 'package:uuid/uuid.dart';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
final List<StreamSubscription<KeyboardEvent>> keyListeners = [];

// WebCodecs VideoFrames handed over from js/src/webcodecs.js arrive as plain
// interop objects (the package language version predates extension types).
// This side owns each frame and must close it quickly: hardware decoders
// stall once their small output frame pool is exhausted.
int _videoFrameWidth(Object frame) =>
    js_util.getProperty(frame, 'displayWidth');
int _videoFrameHeight(Object frame) =>
    js_util.getProperty(frame, 'displayHeight');
void _closeVideoFrame(Object frame) {
  try {
    js_util.callMethod(frame, 'close', []);
  } catch (_) {
    // closing twice is the only failure mode and is harmless
  }
}

typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);

class PlatformFFI {
  final _eventHandlers = <String, Map<String, HandleEvent>>{};
  final RustdeskImpl _ffiBind = RustdeskImpl();

  static String getByName(String name, [String arg = '']) {
    return context.callMethod('getByName', [name, arg]);
  }

  static void setByName(String name, [String value = '']) {
    context.callMethod('setByName', [name, value]);
  }

  PlatformFFI._() {
    window.document.addEventListener(
        'visibilitychange',
        (event) => {
              stateGlobal.isWebVisible =
                  window.document.visibilityState == 'visible'
            });
  }

  static final PlatformFFI instance = PlatformFFI._();

  static get localeName => window.navigator.language;
  RustdeskImpl get ffiBind => _ffiBind;

  static Future<String> getVersion() async {
    throw UnimplementedError();
  }

  bool registerEventHandler(
      String eventName, String handlerName, HandleEvent handler,
      {bool replace = false}) {
    debugPrint('registerEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers == null) {
      _eventHandlers[eventName] = {handlerName: handler};
      return true;
    } else {
      if (!replace && handlers.containsKey(handlerName)) {
        return false;
      } else {
        handlers[handlerName] = handler;
        return true;
      }
    }
  }

  void unregisterEventHandler(String eventName, String handlerName) {
    debugPrint('unregisterEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers != null) {
      handlers.remove(handlerName);
    }
  }

  Future<bool> tryHandle(Map<String, dynamic> evt) async {
    final name = evt['name'];
    if (name != null) {
      final handlers = _eventHandlers[name];
      if (handlers != null) {
        if (handlers.isNotEmpty) {
          for (var handler in handlers.values) {
            await handler(evt);
          }
          return true;
        }
      }
    }
    return false;
  }

  String translate(String name, String locale) =>
      _ffiBind.translate(name: name, locale: locale);

  Uint8List? getRgba(SessionID sessionId, int display, int bufSize) {
    throw UnimplementedError();
  }

  int getRgbaSize(SessionID sessionId, int display) =>
      _ffiBind.sessionGetRgbaSize(sessionId: sessionId, display: display);
  void nextRgba(SessionID sessionId, int display) =>
      _ffiBind.sessionNextRgba(sessionId: sessionId, display: display);
  void registerPixelbufferTexture(SessionID sessionId, int display, int ptr) =>
      _ffiBind.sessionRegisterPixelbufferTexture(
          sessionId: sessionId, display: display, ptr: ptr);
  void registerGpuTexture(SessionID sessionId, int display, int ptr) =>
      _ffiBind.sessionRegisterGpuTexture(
          sessionId: sessionId, display: display, ptr: ptr);

  Future<void> init(String appType) async {
    Completer completer = Completer();
    context["onInitFinished"] = () {
      completer.complete();
    };
    context['dialog'] = (type, title, text) {
      final uuid = Uuid();
      msgBox(SessionID(uuid.v4()), type, title, text, '', gFFI.dialogManager);
    };
    context['loginDialog'] = () {
      loginDialog();
    };
    context['closeConnection'] = () {
      gFFI.dialogManager.dismissAll();
      closeConnection();
    };
    context.callMethod('init');
    version = getByName('version');
    window.onContextMenu.listen((event) {
      event.preventDefault();
    });

    context['onRegisteredEvent'] = (String message) {
      try {
        Map<String, dynamic> event = json.decode(message);
        tryHandle(event);
      } catch (e) {
        print('json.decode fail(): $e');
      }
    };
    return completer.future;
  }

  void setEventCallback(void Function(Map<String, dynamic>) fun) {
    context["onGlobalEvent"] = (String message) {
      try {
        Map<String, dynamic> event = json.decode(message);
        fun(event);
      } catch (e) {
        print('json.decode fail(): $e');
      }
    };
  }

  void setRgbaCallback(void Function(int, Uint8List) fun) {
    context["onRgba"] = (int display, Uint8List? rgba) {
      if (rgba != null) {
        fun(display, rgba);
      }
    };
  }

  void Function(int, ui.Image)? _videoImageCallback;
  final Map<int, Object> _pendingVideoFrames = {};
  bool _processingVideoFrames = false;
  bool _videoFramesDisabled = false;

  // Zero-readback video path: the JS decoder hands decoded VideoFrames here
  // (checking typeof window.onVideoFrame before every frame), and the engine
  // imports them GPU-to-GPU via createImageBitmap. Unregistering the JS global
  // reverts the JS side to the RGBA readback path.
  void setVideoFrameCallback(void Function(int, ui.Image) fun) {
    _videoImageCallback = fun;
    if (_videoFramesDisabled) return;
    js_util.setProperty(js_util.globalThis, 'onVideoFrame',
        js_util.allowInterop((int display, Object frame) {
      _handleVideoFrame(display, frame);
    }));
  }

  void _handleVideoFrame(int display, Object frame) {
    if (_videoFramesDisabled || _videoImageCallback == null) {
      _closeVideoFrame(frame);
      return;
    }
    // Keep only the newest undrawn frame per display: a remote desktop wants
    // freshness, and the decoder needs its frame pool returned promptly.
    final stale = _pendingVideoFrames[display];
    if (stale != null) {
      _closeVideoFrame(stale);
    }
    _pendingVideoFrames[display] = frame;
    if (_processingVideoFrames) return;
    _processingVideoFrames = true;
    Future(() async {
      while (_pendingVideoFrames.isNotEmpty) {
        final display = _pendingVideoFrames.keys.first;
        final frame = _pendingVideoFrames.remove(display)!;
        if (_videoFramesDisabled) {
          _closeVideoFrame(frame);
          continue;
        }
        ui.Image? image;
        try {
          image = await ui_web.createImageFromTextureSource(frame as JSAny,
              width: _videoFrameWidth(frame),
              height: _videoFrameHeight(frame));
        } catch (e) {
          debugPrint(
              'createImageFromTextureSource failed, using RGBA path: $e');
          _videoFramesDisabled = true;
          js_util.setProperty(js_util.globalThis, 'onVideoFrame', null);
        } finally {
          _closeVideoFrame(frame);
        }
        if (image != null) {
          final callback = _videoImageCallback;
          if (callback == null) {
            image.dispose();
          } else {
            try {
              callback(display, image);
            } catch (e) {
              // Nothing downstream took ownership (ImageModel catches its own
              // errors before adopting the image), so free the texture here.
              image.dispose();
              debugPrint('video image callback error: $e');
            }
          }
        }
      }
      _processingVideoFrames = false;
    });
  }

  void startDesktopWebListener() {
    mouseListeners.add(
        window.document.onContextMenu.listen((evt) => evt.preventDefault()));
  }

  void stopDesktopWebListener() {
    for (var ml in mouseListeners) {
      ml.cancel();
    }
    mouseListeners.clear();
    for (var kl in keyListeners) {
      kl.cancel();
    }
    keyListeners.clear();
  }

  void setMethodCallHandler(FMethod callback) {}

  invokeMethod(String method, [dynamic arguments]) async {
    return true;
  }

  // just for compilation
  void syncAndroidServiceAppDirConfigPath() {}

  void setFullscreenCallback(void Function(bool) fun) {
    context["onFullscreenChanged"] = (bool v) {
      fun(v);
    };
  }
}

// ignore_for_file: avoid_web_libraries_in_flutter

import 'dart:convert';
import 'dart:typed_data';
import 'dart:js';
import 'dart:html';
import 'dart:async';

import 'package:flutter/material.dart';

import 'package:flutter_hbb/web/common.dart';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
final List<StreamSubscription<KeyboardEvent>> keyListeners = [];

typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);

class PlatformFFI {
  final _eventHandlers = <String, Map<String, HandleEvent>>{};

  static String getByName(String name, [String arg = '']) {
    return context.callMethod('getByName', [name, arg]);
  }

  static void setByName(String name, [String value = '']) {
    context.callMethod('setByName', [name, value]);
  }

  PlatformFFI._();
  static final PlatformFFI instance = PlatformFFI._();

  static get localeName => window.navigator.language;

  static Future<void> init(String appType) async {
    isWeb = true;
    isWebDesktop = !context.callMethod('isMobile');
    context.callMethod('init');
    version = getByName('version');
  }

  static void setEventCallback(void Function(Map<String, dynamic>) fun) {
    context["onGlobalEvent"] = (String message) {
      try {
        Map<String, dynamic> event = json.decode(message);
        fun(event);
      } catch (e) {
        print('json.decode fail(): $e');
      }
    };
  }

  static void setRgbaCallback(void Function(Uint8List) fun) {
    context["onRgba"] = (Uint8List? rgba) {
      if (rgba != null) {
        fun(rgba);
      }
    };
  }

  static void startDesktopWebListener() {
    mouseListeners.add(
        window.document.onContextMenu.listen((evt) => evt.preventDefault()));
  }

  static void stopDesktopWebListener() {
    for (var ml in mouseListeners) {
      ml.cancel();
    }
    mouseListeners.clear();
    for (var kl in keyListeners) {
      kl.cancel();
    }
    keyListeners.clear();
  }

  static void setMethodCallHandler(FMethod callback) {}

  static Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return true;
  }

  bool registerEventHandler(
      String eventName, String handlerName, HandleEvent handler) {
    debugPrint('registerEventHandler $eventName $handlerName');
    var handlers = _eventHandlers[eventName];
    if (handlers == null) {
      _eventHandlers[eventName] = {handlerName: handler};
      return true;
    } else {
      if (handlers.containsKey(handlerName)) {
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
}

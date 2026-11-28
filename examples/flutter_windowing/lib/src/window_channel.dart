import 'package:flutter/services.dart';

import 'channels.dart';

typedef MessageHandler = Future<dynamic> Function(MethodCall call);

class ClientMessageChannel {
  const ClientMessageChannel();

  Future<dynamic> invokeMethod(String method, [dynamic arguments]) {
    return windowEventChannel.invokeMethod(method, arguments);
  }

  void setMessageHandler(MessageHandler? handler) {
    windowEventChannel.setMethodCallHandler(handler);
  }
}

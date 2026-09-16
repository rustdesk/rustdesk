import 'dart:io';

import 'package:flutter/services.dart';

class Nv12GlTexture {
  static const _channel = MethodChannel('org.rustdesk.rustdesk/nv12_gl_texture');

  static bool get supported => Platform.isLinux;

  Future<int> createTexture(int key) async {
    final id = await _channel.invokeMethod<int>('createTexture', {'key': key});
    return id ?? -1;
  }

  Future<bool> closeTexture(int key) async {
    return await _channel.invokeMethod<bool>('closeTexture', {'key': key}) ??
        false;
  }

  Future<int> getTexturePtr(int key) async {
    return await _channel.invokeMethod<int>('getTexturePtr', {'key': key}) ?? 0;
  }
}

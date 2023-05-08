import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/plugin/ui_manager.dart';
import 'package:flutter_hbb/plugin/utils/dialogs.dart';

abstract class NativeHandler {
  bool onEvent(Map<String, dynamic> evt);
}

typedef OnSelectPeersCallback = Bool Function(Int returnCode,
    Pointer<Void> data, Uint64 dataLength, Pointer<Void> userData);
typedef OnSelectPeersCallbackDart = bool Function(
    int returnCode, Pointer<Void> data, int dataLength, Pointer<Void> userData);

class NativeUiHandler extends NativeHandler {
  NativeUiHandler._();

  static NativeUiHandler instance = NativeUiHandler._();

  @override
  bool onEvent(Map<String, dynamic> evt) {
    final name = evt['name'];
    final action = evt['action'];
    if (name != "native_ui") {
      return false;
    }
    switch (action) {
      case "select_peers":
        int cb = evt['cb'];
        int userData = evt['user_data'] ?? 0;
        final cbFuncNative = Pointer.fromAddress(cb)
            .cast<NativeFunction<OnSelectPeersCallback>>();
        final cbFuncDart = cbFuncNative.asFunction<OnSelectPeersCallbackDart>();
        onSelectPeers(cbFuncDart, userData);
        break;
      case "register_ui_entry":
        int cb = evt['on_tap_cb'];
        int userData = evt['user_data'] ?? 0;
        String title = evt['title'] ?? "";
        final cbFuncNative = Pointer.fromAddress(cb)
            .cast<NativeFunction<OnSelectPeersCallback>>();
        final cbFuncDart = cbFuncNative.asFunction<OnSelectPeersCallbackDart>();
        onRegisterUiEntry(title, cbFuncDart, userData);
        break;
      default:
        return false;
    }
    return true;
  }

  void onSelectPeers(OnSelectPeersCallbackDart cb, int userData) async {
    showPeerSelectionDialog(onPeersCallback: (peers) {
      String json = jsonEncode(<String, dynamic> {
        "peers": peers
      });
      final native = json.toNativeUtf8();
      cb(0, native.cast(), native.length, Pointer.fromAddress(userData));
      malloc.free(native);
    });
  }
  
  void onRegisterUiEntry(String title, OnSelectPeersCallbackDart cbFuncDart, int userData) {
    Widget widget = InkWell(
      child: Container(
        height: 25.0,
        child: Row(
          children: [
            Expanded(child: Text(title)),
            Icon(Icons.chevron_right_rounded, size: 12.0,)
          ],
        ),
      ),
    );
    PluginUiManager.instance.registerEntry(title, widget);
  }
}

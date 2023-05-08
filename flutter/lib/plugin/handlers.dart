import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';
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
}

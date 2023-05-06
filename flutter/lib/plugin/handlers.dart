import 'dart:ffi';

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
    // TODO: design a UI interface to pick peers.
    cb(0, Pointer.fromAddress(0), 0, Pointer.fromAddress(userData));
  }
}

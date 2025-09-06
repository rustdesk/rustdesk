abstract class NativeHandler {
  bool onEvent(Map<String, dynamic> evt);
}

class NativeUiHandler extends NativeHandler {
  NativeUiHandler._();

  static NativeUiHandler instance = NativeUiHandler._();

  @override
  bool onEvent(Map<String, dynamic> evt) {
      throw UnimplementedError();
  }
}

import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';

RustdeskImpl get bind => platformFFI.ffiBind;

String mainGetLocalOption({required String key, dynamic hint}) {
  return bind.mainGetLocalOption(key: key, hint: hint);
}

Future<void> mainSetLocalOption(
    {required String key, required String value, dynamic hint}) {
  return bind.mainSetLocalOption(key: key, value: value, hint: hint);
}

Future<void> mainChangeTheme({required String dark, dynamic hint}) {
  return bind.mainChangeTheme(dark: dark, hint: hint);
}

String mainGetLoginDeviceInfo() {
  return bind.mainGetLoginDeviceInfo();
}

mainGetApiServer() {
  return bind.mainGetApiServer();
}

mainGetMyId() {
  return bind.mainGetMyId();
}

mainGetUuid() {
  return bind.mainGetUuid();
}

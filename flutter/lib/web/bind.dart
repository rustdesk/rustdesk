import 'package:flutter_hbb/web/common.dart';

String mainGetLocalOption({required String key, dynamic hint}) {
  return "";
}

Future<void> mainSetLocalOption(
    {required String key, required String value, dynamic hint}) {
  return Future.value();
}

Future<void> mainChangeTheme({required String dark, dynamic hint}) {
  return Future.value();
}

String mainGetLoginDeviceInfo() {
  return "";
}

Future<String> mainGetApiServer() {
  return Future.value("");
}

Future<String> mainGetMyId() {
  return Future.value("");
}

Future<String> mainGetUuid() {
  return Future.value("");
}

Future<void> mainSetOption(
    {required String key, required String value, dynamic hint}) {
  return Future.value();
}

Future<void> sessionSetRemember({required SessionID sessionId, required bool value}) {
  return Future.value();
}

Future<bool?> sessionGetRemember({required SessionID sessionId}) {
  return Future.value(false);
}

Future<void> sessionPeerOption({required SessionID sessionId, required String name, required String value}) {
  return Future.value();
}

Future<void> sessionElevateWithLogon({required SessionID sessionId, required String username, required String password}) {
  return Future.value();
}

Future<void> sessionElevateDirect({required SessionID sessionId}) {
  return Future.value();
}

Future<void> sessionRestartRemoteDevice({required SessionID sessionId}) {
  return Future.value();
}

Future<String?> sessionGetOption({required SessionID sessionId, required String arg, dynamic hint}) {
  return Future.value("");
}

Future<void> sessionInputOsPassword({required SessionID sessionId, required String value, dynamic hint}) {
  return Future.value();
}

Future<void> sessionSendNote({required SessionID sessionId, required String note}) {
  return Future.value();
}

Future<bool> mainIsUsingPublicServer() {
  return Future.value(false);
}

Future<void> sessionSetCustomImageQuality({required SessionID sessionId, required int value}) {
  return Future.value();
}

Future<void> sessionSetCustomFps({required SessionID sessionId, required int fps}) {
  return Future.value();
}

sessionGetCustomImageQuality({required SessionID sessionId}) {
  return Future.value(0);
}

mainHasValid2FaSync() {
  return Future.value(false);
}

mainGenerate2Fa() {
  return Future.value("");
}

mainVerify2Fa({required String code}) {
  return Future.value();
}

sessionSendSelectedSessionId({required SessionID sessionId, required String sid}) {
  return Future.value();
}

versionCmp(String a, String b) {
  return 0;
}

sessionGetViewStyle({required SessionID sessionId}) {
  return Future.value("");
}

sessionGetScrollStyle({required SessionID sessionId}) {
  return Future.value("");
}

sessionSetViewStyle({required SessionID sessionId, required String value}) {
  return Future.value();
}

sessionChangePreferCodec({required SessionID sessionId}) {
  return Future.value();
}

sessionAlternativeCodecs({required SessionID sessionId}) {
  return Future.value();
}

sessionGetImageQuality({required SessionID sessionId}) {
  return Future.value(0);
}

sessionSetImageQuality({required SessionID sessionId, required String value}) {
  return Future.value();
}

mainClearAb() {
  return Future.value();
}

getLocalFlutterOption({required String k}) {
  return '';
}

mainPeerExists({required String id}) {
  return Future.value(false);
}

mainGetNewStoredPeers() {
  return Future.value([]);
}

mainLoadRecentPeersForAb({required String filter}) {
  return Future.value([]);
}

mainSaveAb({required String json}) {
  return Future.value();
}

mainLoadAb() {
  return Future.value("");
}

mainSaveGroup({required String json}) {
  return Future.value();
}

mainLoadGroup() {
  return Future.value("");
}

mainClearGroup() {
  return Future.value();
}

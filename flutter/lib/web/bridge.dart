import 'dart:async';
import 'dart:js' as js;
import 'dart:convert';
import 'dart:typed_data';
import 'package:uuid/uuid.dart';

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models');

mixin _$EventToUI {
  Object get field0 => throw _privateConstructorUsedError;
}

sealed class EventToUI {
  const factory EventToUI.event(
    String field0,
  ) = EventToUI_Event;
  const factory EventToUI.rgba(
    int field0,
  ) = EventToUI_Rgba;
  const factory EventToUI.texture(
    int field0,
  ) = EventToUI_Texture;
}

class EventToUI_Event implements EventToUI {
  const EventToUI_Event(final String field0) : this.field = field0;
  final String field;
  String get field0 => field;
}

class EventToUI_Rgba implements EventToUI {
  const EventToUI_Rgba(final int field0) : this.field = field0;
  final int field;
  int get field0 => field;
}

class EventToUI_Texture implements EventToUI {
  const EventToUI_Texture(final int field0) : this.field = field0;
  final int field;
  int get field0 => field;
}

class RustdeskImpl {
  Future<void> stopGlobalEventStream({required String appType, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> hostStopSystemKeyPropagate(
      {required bool stopped, dynamic hint}) {
    throw UnimplementedError();
  }

  int peerGetDefaultSessionsCount({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  String sessionAddExistedSync(
      {required String id, required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  void sessionTryAddDisplay(
      {required UuidValue sessionId,
      required Int32List displays,
      dynamic hint}) {
    throw UnimplementedError();
  }

  String sessionAddSync(
      {required UuidValue sessionId,
      required String id,
      required bool isFileTransfer,
      required bool isPortForward,
      required bool isRdp,
      required String switchUuid,
      required bool forceRelay,
      required String password,
      required bool isSharedPassword,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Stream<EventToUI> sessionStart(
      {required UuidValue sessionId, required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool?> sessionGetRemember(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool?> sessionGetToggleOption(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    throw UnimplementedError();
  }

  bool sessionGetToggleOptionSync(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetOption(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionLogin(
      {required UuidValue sessionId,
      required String osUsername,
      required String osPassword,
      required String password,
      required bool remember,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSend2Fa(
      {required UuidValue sessionId, required String code, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionClose({required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRefresh(
      {required UuidValue sessionId, required int display, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRecordScreen(
      {required UuidValue sessionId,
      required bool start,
      required int display,
      required int width,
      required int height,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRecordStatus(
      {required UuidValue sessionId, required bool status, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionReconnect(
      {required UuidValue sessionId, required bool forceRelay, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionToggleOption(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionTogglePrivacyMode(
      {required UuidValue sessionId,
      required String implKey,
      required bool on,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetFlutterOption(
      {required UuidValue sessionId, required String k, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetFlutterOption(
      {required UuidValue sessionId,
      required String k,
      required String v,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetFlutterOptionByPeerId(
      {required String id, required String k, dynamic hint}) {
    return Future.value(null);
  }

  int getNextTextureKey({dynamic hint}) {
    return 0;
  }

  String getLocalFlutterOption({required String k, dynamic hint}) {
    return js.context.callMethod('getByName', ['option:flutter:local', k]);
  }

  Future<void> setLocalFlutterOption(
      {required String k, required String v, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:flutter:local',
          jsonEncode({'name': k, 'value': v})
        ]));
  }

  String getLocalKbLayoutType({dynamic hint}) {
    throw js.context.callMethod('getByName', ['option:local', 'kb_layout']);
  }

  Future<void> setLocalKbLayoutType(
      {required String kbLayoutType, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:local',
          jsonEncode({'name': 'kb_layout', 'value': kbLayoutType})
        ]));
  }

  Future<String?> sessionGetViewStyle(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetViewStyle(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetScrollStyle(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetScrollStyle(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetImageQuality(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetImageQuality(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> sessionGetKeyboardMode(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetKeyboardMode(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  String? sessionGetReverseMouseWheelSync(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetReverseMouseWheel(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  String? sessionGetDisplaysAsIndividualWindows(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetDisplaysAsIndividualWindows(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  String? sessionGetUseAllMyDisplaysForTheRemoteSession(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetUseAllMyDisplaysForTheRemoteSession(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<Int32List?> sessionGetCustomImageQuality(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  bool sessionIsKeyboardModeSupported(
      {required UuidValue sessionId, required String mode, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetCustomImageQuality(
      {required UuidValue sessionId, required int value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetCustomFps(
      {required UuidValue sessionId, required int fps, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionLockScreen({required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionCtrlAltDel({required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSwitchDisplay(
      {required bool isDesktop,
      required UuidValue sessionId,
      required Int32List value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionHandleFlutterKeyEvent(
      {required UuidValue sessionId,
      required String name,
      required int platformCode,
      required int positionCode,
      required int lockModes,
      required bool downOrUp,
      dynamic hint}) {
    throw UnimplementedError();
  }

  void sessionEnterOrLeave(
      {required UuidValue sessionId, required bool enter, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionInputKey(
      {required UuidValue sessionId,
      required String name,
      required bool down,
      required bool press,
      required bool alt,
      required bool ctrl,
      required bool shift,
      required bool command,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionInputString(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendChat(
      {required UuidValue sessionId, required String text, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionPeerOption(
      {required UuidValue sessionId,
      required String name,
      required String value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> sessionGetPeerOption(
      {required UuidValue sessionId, required String name, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionInputOsPassword(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionReadRemoteDir(
      {required UuidValue sessionId,
      required String path,
      required bool includeHidden,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendFiles(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required String to,
      required int fileNum,
      required bool includeHidden,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetConfirmOverrideFile(
      {required UuidValue sessionId,
      required int actId,
      required int fileNum,
      required bool needOverride,
      required bool remember,
      required bool isUpload,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRemoveFile(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required int fileNum,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionReadDirRecursive(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required bool isRemote,
      required bool showHidden,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRemoveAllEmptyDirs(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionCancelJob(
      {required UuidValue sessionId, required int actId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionCreateDir(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> sessionReadLocalDirSync(
      {required UuidValue sessionId,
      required String path,
      required bool showHidden,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> sessionGetPlatform(
      {required UuidValue sessionId, required bool isRemote, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionLoadLastTransferJobs(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionAddJob(
      {required UuidValue sessionId,
      required int actId,
      required String path,
      required String to,
      required int fileNum,
      required bool includeHidden,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionResumeJob(
      {required UuidValue sessionId,
      required int actId,
      required bool isRemote,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionElevateDirect(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionElevateWithLogon(
      {required UuidValue sessionId,
      required String username,
      required String password,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSwitchSides(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionChangeResolution(
      {required UuidValue sessionId,
      required int display,
      required int width,
      required int height,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSetSize(
      {required UuidValue sessionId,
      required int display,
      required int width,
      required int height,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendSelectedSessionId(
      {required UuidValue sessionId, required String sid, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<List<String>> mainGetSoundInputs({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> mainGetDefaultSoundInput({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetLoginDeviceInfo({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainChangeId({required String newId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetAsyncStatus({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetOption({required String key, dynamic hint}) {
    return Future.value(mainGetOptionSync(key: key));
  }

  String mainGetOptionSync({required String key, dynamic hint}) {
    return js.context.callMethod('getByName', ['option', key]);
  }

  Future<String> mainGetError({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainShowOption({required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetOption(
      {required String key, required String value, dynamic hint}) {
    return js.context.callMethod('setByName', [
      'option',
      jsonEncode({'name': key, 'value': value})
    ]);
  }

  Future<String> mainGetOptions({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetOptionsSync({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetOptions({required String json, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainTestIfValidServer({required String server, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetSocks(
      {required String proxy,
      required String username,
      required String password,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<List<String>> mainGetSocks({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetAppName({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetAppNameSync({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainUriPrefixSync({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetLicense({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetVersion({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<List<String>> mainGetFav({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStoreFav({required List<String> favs, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetPeerSync({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetLanPeers({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetConnectStatus({dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ["get_conn_status"]));
  }

  Future<void> mainCheckConnectStatus({dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ["check_conn_status"]));
  }

  Future<bool> mainIsUsingPublicServer({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainDiscover({dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['discover']));
  }

  Future<String> mainGetApiServer({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainPostRequest(
      {required String url,
      required String body,
      required String header,
      dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetLocalOption({required String key, dynamic hint}) {
    return js.context.callMethod('getByName', ['option:local', key]);
  }

  String mainGetEnv({required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetLocalOption(
      {required String key, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:local',
          jsonEncode({'name': key, 'value': value})
        ]));
  }

  String mainGetInputSource({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetInputSource(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetMyId({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetUuid({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetPeerOption(
      {required String id, required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetPeerOptionSync(
      {required String id, required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetPeerFlutterOptionSync(
      {required String id, required String k, dynamic hint}) {
    throw UnimplementedError();
  }

  void mainSetPeerFlutterOptionSync(
      {required String id,
      required String k,
      required String v,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetPeerOption(
      {required String id,
      required String key,
      required String value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainSetPeerOptionSync(
      {required String id,
      required String key,
      required String value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetPeerAlias(
      {required String id, required String alias, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetNewStoredPeers({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainForgetPassword({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainPeerHasPassword({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainPeerExists({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainLoadRecentPeers({dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['load_recent_peers']));
  }

  String mainLoadRecentPeersSync({dynamic hint}) {
    return js.context.callMethod('getByName', ['load_recent_peers_sync']);
  }

  String mainLoadLanPeersSync({dynamic hint}) {
    return js.context.callMethod('getByName', ['load_lan_peers_sync']);
  }

  Future<String> mainLoadRecentPeersForAb(
      {required String filter, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainLoadFavPeers({dynamic hint}) {
    return Future(() => js.context.callMethod('getByName', ['load_fav_peers']));
  }

  Future<void> mainLoadLanPeers({dynamic hint}) {
    return Future(() => js.context.callMethod('getByName', ['load_lan_peers']));
  }

  Future<void> mainRemoveDiscovered({required String id, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['remove_discovered']));
  }

  Future<void> mainChangeTheme({required String dark, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainChangeLanguage({required String lang, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainDefaultVideoSaveDirectory({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetUserDefaultOption(
      {required String key, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetUserDefaultOption({required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainHandleRelayId({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetMainDisplay({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetDisplays({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionAddPortForward(
      {required UuidValue sessionId,
      required int localPort,
      required String remoteHost,
      required int remotePort,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRemovePortForward(
      {required UuidValue sessionId, required int localPort, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionNewRdp({required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRequestVoiceCall(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionCloseVoiceCall(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmHandleIncomingVoiceCall(
      {required int id, required bool accept, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmCloseVoiceCall({required int id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetLastRemoteId({dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['option', 'last_remote_id']));
  }

  Future<String> mainGetSoftwareUpdateUrl({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetHomeDir({dynamic hint}) {
    return Future.value('');
  }

  Future<String> mainGetLangs({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetTemporaryPassword({dynamic hint}) {
    return Future.value('');
  }

  Future<String> mainGetPermanentPassword({dynamic hint}) {
    return Future.value('');
  }

  Future<String> mainGetFingerprint({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> cmGetClientsState({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String?> cmCheckClientsLength({required int length, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<int> cmGetClientsLength({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainInit({required String appDir, dynamic hint}) {
    return Future.value();
  }

  Future<void> mainDeviceId({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainDeviceName({required String name, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainRemovePeer({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasHwcodec({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasGpucodec({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainSupportedHwdecodings({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainIsRoot({dynamic hint}) {
    throw UnimplementedError();
  }

  int getDoubleClickTime({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStartDbusServer({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSaveAb({required String json, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainClearAb({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainLoadAb({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSaveGroup({required String json, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainClearGroup({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainLoadGroup({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendPointer(
      {required UuidValue sessionId, required String msg, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendMouse(
      {required UuidValue sessionId, required String msg, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRestartRemoteDevice(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  String sessionGetAuditServerSync(
      {required UuidValue sessionId, required String typ, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionSendNote(
      {required UuidValue sessionId, required String note, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> sessionAlternativeCodecs(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionChangePreferCodec(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionOnWaitingForImageDialogShow(
      {required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionToggleVirtualDisplay(
      {required UuidValue sessionId,
      required int index,
      required bool on,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetHomeDir({required String home, dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetDataDirIos({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStopService({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStartService({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainUpdateTemporaryPassword({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetPermanentPassword(
      {required String password, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainCheckSuperUserPermission({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainCheckMouseTime({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<double> mainGetMouseTime({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainWol({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainCreateShortcut({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmSendChat(
      {required int connId, required String msg, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmLoginRes(
      {required int connId, required bool res, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmCloseConnection({required int connId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmRemoveDisconnectedConnection(
      {required int connId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmCheckClickTime({required int connId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<double> cmGetClickTime({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmSwitchPermission(
      {required int connId,
      required String name,
      required bool enabled,
      dynamic hint}) {
    throw UnimplementedError();
  }

  bool cmCanElevate({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmElevatePortable({required int connId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmSwitchBack({required int connId, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> cmGetConfig({required String name, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetBuildDate({dynamic hint}) {
    throw UnimplementedError();
  }

  String translate(
      {required String name, required String locale, dynamic hint}) {
    return js.context.callMethod('getByName', [
      'translate',
      jsonEncode({'locale': locale, 'text': name})
    ]);
  }

  int sessionGetRgbaSize(
      {required UuidValue sessionId, required int display, dynamic hint}) {
    throw UnimplementedError();
  }

  void sessionNextRgba(
      {required UuidValue sessionId, required int display, dynamic hint}) {
    throw UnimplementedError();
  }

  void sessionRegisterPixelbufferTexture(
      {required UuidValue sessionId,
      required int display,
      required int ptr,
      dynamic hint}) {
    throw UnimplementedError();
  }

  void sessionRegisterGpuTexture(
      {required UuidValue sessionId,
      required int display,
      required int ptr,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> queryOnlines({required List<String> ids, dynamic hint}) {
    throw UnimplementedError();
  }

  int versionToNumber({required String v, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> optionSynced({dynamic hint}) {
    return Future.value(true);
  }

  bool mainIsInstalled({dynamic hint}) {
    throw UnimplementedError();
  }

  void mainInitInputSource({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsInstalledLowerVersion({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsInstalledDaemon({required bool prompt, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsProcessTrusted({required bool prompt, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsCanScreenRecording({required bool prompt, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsCanInputMonitoring({required bool prompt, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsShareRdp({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetShareRdp({required bool enable, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainGotoInstall({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetNewVersion({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainUpdateMe({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> setCurSessionId({required UuidValue sessionId, dynamic hint}) {
    throw UnimplementedError();
  }

  bool installShowRunWithoutInstall({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> installRunWithoutInstall({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> installInstallMe(
      {required String options, required String path, dynamic hint}) {
    throw UnimplementedError();
  }

  String installInstallPath({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainAccountAuth(
      {required String op, required bool rememberMe, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainAccountAuthCancel({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainAccountAuthResult({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainOnMainWindowClose({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainCurrentIsWayland({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsLoginWayland({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStartPa({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHideDocker({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasPixelbufferTextureRender({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasFileClipboard({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasGpuTextureRender({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> cmInit({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainStartIpcUrlServer({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainTestWallpaper({required int second, dynamic hint}) {
    // TODO: implement mainTestWallpaper
    return Future.value();
  }

  Future<bool> mainSupportRemoveWallpaper({dynamic hint}) {
    // TODO: implement mainSupportRemoveWallpaper
    return Future.value(false);
  }

  bool isIncomingOnly({dynamic hint}) {
    return false;
  }

  bool isOutgoingOnly({dynamic hint}) {
    return false;
  }

  bool isCustomClient({dynamic hint}) {
    return false;
  }

  bool isDisableSettings({dynamic hint}) {
    return false;
  }

  bool isDisableAb({dynamic hint}) {
    return false;
  }

  bool isDisableAccount({dynamic hint}) {
    return false;
  }

  bool isDisableInstallation({dynamic hint}) {
    return false;
  }

  Future<bool> isPresetPassword({dynamic hint}) {
    return Future.value(false);
  }

  Future<void> sendUrlScheme({required String url, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginEvent(
      {required String id,
      required String peer,
      required Uint8List event,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Stream<EventToUI> pluginRegisterEventStream(
      {required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  String? pluginGetSessionOption(
      {required String id,
      required String peer,
      required String key,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginSetSessionOption(
      {required String id,
      required String peer,
      required String key,
      required String value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  String? pluginGetSharedOption(
      {required String id, required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginSetSharedOption(
      {required String id,
      required String key,
      required String value,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginReload({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  void pluginEnable({required String id, required bool v, dynamic hint}) {
    throw UnimplementedError();
  }

  bool pluginIsEnabled({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  bool pluginFeatureIsEnabled({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginSyncUi({required String syncTo, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginListReload({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> pluginInstall(
      {required String id, required bool b, dynamic hint}) {
    throw UnimplementedError();
  }

  bool isSupportMultiUiSession({required String version, dynamic hint}) {
    throw UnimplementedError();
  }

  bool isSelinuxEnforcing({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainDefaultPrivacyModeImpl({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainSupportedPrivacyModeImpls({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainSupportedInputSource({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGenerate2Fa({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainVerify2Fa({required String code, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasValid2FaSync({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainGetHardOption({required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  void dispose() {
    throw UnimplementedError();
  }
}

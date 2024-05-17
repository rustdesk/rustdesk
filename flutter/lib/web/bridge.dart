import 'dart:async';
import 'dart:js' as js;
import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

import 'package:flutter_hbb/consts.dart';

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
    return 0;
  }

  String sessionAddExistedSync(
      {required String id, required UuidValue sessionId, dynamic hint}) {
    return '';
  }

  void sessionTryAddDisplay(
      {required UuidValue sessionId,
      required Int32List displays,
      dynamic hint}) {}

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
    return js.context.callMethod('setByName', [
      'session_add_sync',
      jsonEncode({'id': id, 'password': password})
    ]);
  }

  Stream<EventToUI> sessionStart(
      {required UuidValue sessionId, required String id, dynamic hint}) {
    js.context.callMethod('setByName', [
      'session_start',
      jsonEncode({'id': id})
    ]);
    return Stream.empty();
  }

  Future<bool?> sessionGetRemember(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['remember']) == 'true');
  }

  Future<bool?> sessionGetToggleOption(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    return Future(
        () => sessionGetToggleOptionSync(sessionId: sessionId, arg: arg));
  }

  bool sessionGetToggleOptionSync(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    return 'true' == js.context.callMethod('getByName', ['option:toggle', arg]);
  }

  Future<String?> sessionGetOption(
      {required UuidValue sessionId, required String arg, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['option:session', arg]));
  }

  Future<void> sessionLogin(
      {required UuidValue sessionId,
      required String osUsername,
      required String osPassword,
      required String password,
      required bool remember,
      dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'login',
          jsonEncode({
            'os_username': osUsername,
            'os_password': osPassword,
            'password': password,
            'remember': remember
          })
        ]));
  }

  Future<void> sessionSend2Fa(
      {required UuidValue sessionId, required String code, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['send_2fa', code]));
  }

  Future<void> sessionClose({required UuidValue sessionId, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['session_close']));
  }

  Future<void> sessionRefresh(
      {required UuidValue sessionId, required int display, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['refresh']));
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
    return Future(() => js.context.callMethod('setByName', ['reconnect']));
  }

  Future<void> sessionToggleOption(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ['toggle_option', value]));
  }

  Future<void> sessionTogglePrivacyMode(
      {required UuidValue sessionId,
      required String implKey,
      required bool on,
      dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'toggle_option',
          jsonEncode({implKey, on})
        ]));
  }

  Future<String?> sessionGetFlutterOption(
      {required UuidValue sessionId, required String k, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['option:flutter:peer', k]));
  }

  Future<void> sessionSetFlutterOption(
      {required UuidValue sessionId,
      required String k,
      required String v,
      dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:flutter:peer',
          jsonEncode({'name': k, 'value': v})
        ]));
  }

  Future<String?> sessionGetFlutterOptionByPeerId(
      {required String id, required String k, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['option:flutter:peer', k]));
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
    return Future(() =>
        js.context.callMethod('getByName', ['option:session', 'view_style']));
  }

  Future<void> sessionSetViewStyle(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'view_style', 'value': value})
        ]));
  }

  Future<String?> sessionGetScrollStyle(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(() =>
        js.context.callMethod('getByName', ['option:session', 'scroll_style']));
  }

  Future<void> sessionSetScrollStyle(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'scroll_style', 'value': value})
        ]));
  }

  Future<String?> sessionGetImageQuality(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(() => js.context
        .callMethod('getByName', ['option:session', 'image_quality']));
  }

  Future<void> sessionSetImageQuality(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'image_quality', 'value': value})
        ]));
  }

  Future<String?> sessionGetKeyboardMode(
      {required UuidValue sessionId, dynamic hint}) {
    final mode =
        js.context.callMethod('getByName', ['option:session', 'keyboard_mode']);
    return Future(() => mode == '' ? null : mode);
  }

  Future<void> sessionSetKeyboardMode(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'keyboard_mode', 'value': value})
        ]));
  }

  String? sessionGetReverseMouseWheelSync(
      {required UuidValue sessionId, dynamic hint}) {
    return js.context
        .callMethod('getByName', ['option:session', 'reverse_mouse_wheel']);
  }

  Future<void> sessionSetReverseMouseWheel(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'reverse_mouse_wheel', 'value': value})
        ]));
  }

  String? sessionGetDisplaysAsIndividualWindows(
      {required UuidValue sessionId, dynamic hint}) {
    return js.context.callMethod(
        'getByName', ['option:session', 'displays_as_individual_windows']);
  }

  Future<void> sessionSetDisplaysAsIndividualWindows(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future.value();
  }

  String? sessionGetUseAllMyDisplaysForTheRemoteSession(
      {required UuidValue sessionId, dynamic hint}) {
    return '';
  }

  Future<void> sessionSetUseAllMyDisplaysForTheRemoteSession(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future.value();
  }

  Future<Int32List?> sessionGetCustomImageQuality(
      {required UuidValue sessionId, dynamic hint}) {
    try {
      return Future(() => Int32List.fromList([
            int.parse(js.context.callMethod(
                'getByName', ['option:session', 'custom_image_quality']))
          ]));
    } catch (e) {
      return Future.value(null);
    }
  }

  bool sessionIsKeyboardModeSupported(
      {required UuidValue sessionId, required String mode, dynamic hint}) {
    return mode == kKeyLegacyMode;
  }

  bool sessionIsMultiUiSession({required UuidValue sessionId, dynamic hint}) {
    return false;
  }

  Future<void> sessionSetCustomImageQuality(
      {required UuidValue sessionId, required int value, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'custom_image_quality', 'value': value})
        ]));
  }

  Future<void> sessionSetCustomFps(
      {required UuidValue sessionId, required int fps, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'option:session',
          jsonEncode({'name': 'custom_fps', 'value': fps})
        ]));
  }

  Future<void> sessionLockScreen({required UuidValue sessionId, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['lock_screen']));
  }

  Future<void> sessionCtrlAltDel({required UuidValue sessionId, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['ctrl_alt_del']));
  }

  Future<void> sessionSwitchDisplay(
      {required bool isDesktop,
      required UuidValue sessionId,
      required Int32List value,
      dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', [
          'switch_display',
          jsonEncode({
            isDesktop: isDesktop,
            sessionId: sessionId.toString(),
            value: value
          })
        ]));
  }

  Future<void> sessionHandleFlutterKeyEvent(
      {required UuidValue sessionId,
      required String name,
      required int platformCode,
      required int positionCode,
      required int lockModes,
      required bool downOrUp,
      dynamic hint}) {
    // TODO: map mode
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
    return Future(() => js.context.callMethod('setByName', [
          'input_key',
          jsonEncode({
            'name': name,
            if (down) 'down': 'true',
            if (press) 'press': 'true',
            if (alt) 'alt': 'true',
            if (ctrl) 'ctrl': 'true',
            if (shift) 'shift': 'true',
            if (command) 'command': 'true'
          })
        ]));
  }

  Future<void> sessionInputString(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ['input_string', value]));
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
    return Future(() => js.context.callMethod('SetByName', [
          'option:session',
          jsonEncode({'name': name, 'value': value})
        ]));
  }

  Future<String> sessionGetPeerOption(
      {required UuidValue sessionId, required String name, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['option:session', name]));
  }

  Future<void> sessionInputOsPassword(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ['input_os_password', value]));
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
    return Future(() => js.context.callMethod('setByName', [
          'elevate_with_logon',
          jsonEncode({username, password})
        ]));
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
    // note: restore on disconnected
    throw UnimplementedError();
  }

  Future<void> sessionSetSize(
      {required UuidValue sessionId,
      required int display,
      required int width,
      required int height,
      dynamic hint}) {
    return Future.value();
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
    js.context.callMethod('setByName', [
      'option',
      jsonEncode({'name': key, 'value': value})
    ]);
    return Future.value();
  }

  // get server settings
  Future<String> mainGetOptions({dynamic hint}) {
    return Future(() => mainGetOptionsSync());
  }

  // get server settings
  String mainGetOptionsSync({dynamic hint}) {
    return js.context.callMethod('getByName', ['options']);
  }

  Future<void> mainSetOptions({required String json, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['options', json]));
  }

  Future<String> mainTestIfValidServer(
      {required String server, required bool testWithProxy, dynamic hint}) {
    // TODO: implement
    return Future.value('');
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
    return Future(() => js.context.callMethod('getByName', ['version']));
  }

  Future<List<String>> mainGetFav({dynamic hint}) {
    List<String> favs = [];
    try {
      favs = (jsonDecode(js.context.callMethod('getByName', ['fav']))
              as List<dynamic>)
          .map((e) => e.toString())
          .toList();
    } catch (e) {
      debugPrint('Failed to load favs: $e');
    }
    return Future.value(favs);
  }

  Future<void> mainStoreFav({required List<String> favs, dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ['fav', jsonEncode(favs)]));
  }

  String mainGetPeerSync({required String id, dynamic hint}) {
    // TODO:
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
    throw UnimplementedError();
  }

  Future<bool> mainIsUsingPublicServer({dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ["is_using_public_server"]));
  }

  Future<void> mainDiscover({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetApiServer({dynamic hint}) {
    return Future(() => js.context.callMethod('getByName', ['api_server']));
  }

  Future<void> mainPostRequest(
      {required String url,
      required String body,
      required String header,
      dynamic hint}) {
    throw UnimplementedError();
  }

  Future<bool> mainGetProxyStatus({dynamic hint}) {
    return Future(() => false);
  }

  Future<void> mainHttpRequest({
    required String url,
    required String method,
    String? body,
    required String header,
    dynamic hint,
  }) {
    throw UnimplementedError();
  }

  Future<String?> mainGetHttpStatus({required String url, dynamic hint}) {
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
    // // rdev grab mode
    // const CONFIG_INPUT_SOURCE_1 = "Input source 1";
    // // flutter grab mode
    // const CONFIG_INPUT_SOURCE_2 = "Input source 2";
    return 'Input source 2';
  }

  Future<void> mainSetInputSource(
      {required UuidValue sessionId, required String value, dynamic hint}) {
    return Future.value();
  }

  Future<String> mainGetMyId({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetUuid({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainGetPeerOption(
      {required String id, required String key, dynamic hint}) {
    return Future(() => mainGetPeerOptionSync(id: id, key: key, hint: hint));
  }

  String mainGetPeerOptionSync(
      {required String id, required String key, dynamic hint}) {
    return js.context.callMethod('getByName', [
      'option:peer',
      jsonEncode({'id': id, 'name': key})
    ]);
  }

  String mainGetPeerFlutterOptionSync(
      {required String id, required String k, dynamic hint}) {
    return js.context.callMethod('getByName', ['option:flutter:peer', k]);
  }

  void mainSetPeerFlutterOptionSync(
      {required String id,
      required String k,
      required String v,
      dynamic hint}) {
    js.context.callMethod('setByName', [
      'option:flutter:peer',
      jsonEncode({'name': k, 'value': v})
    ]);
  }

  Future<void> mainSetPeerOption(
      {required String id,
      required String key,
      required String value,
      dynamic hint}) {
    mainSetPeerOptionSync(id: id, key: key, value: value, hint: hint);
    return Future.value();
  }

  bool mainSetPeerOptionSync(
      {required String id,
      required String key,
      required String value,
      dynamic hint}) {
    js.context.callMethod('setByName', [
      'option:peer',
      jsonEncode({'id': id, 'name': key, 'value': value})
    ]);
    return true;
  }

  Future<void> mainSetPeerAlias(
      {required String id, required String alias, dynamic hint}) {
    mainSetPeerOptionSync(id: id, key: 'alias', value: alias, hint: hint);
    return Future.value();
  }

  Future<String> mainGetNewStoredPeers({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainForgetPassword({required String id, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['forget']));
  }

  Future<bool> mainPeerHasPassword({required String id, dynamic hint}) {
    return Future(() =>
        js.context.callMethod('getByName', ['peer_has_password', id]) ==
        'true');
  }

  Future<bool> mainPeerExists({required String id, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['peer_exists', id]));
  }

  Future<void> mainLoadRecentPeers({dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['load_recent_peers']));
  }

  String mainLoadRecentPeersSync({dynamic hint}) {
    return js.context.callMethod('getByName', ['load_recent_peers_sync']);
  }

  String mainLoadLanPeersSync({dynamic hint}) {
    return '{}';
  }

  Future<String> mainLoadRecentPeersForAb(
      {required String filter, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainLoadFavPeers({dynamic hint}) {
    return Future(() => js.context.callMethod('getByName', ['load_fav_peers']));
  }

  Future<void> mainLoadLanPeers({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainRemoveDiscovered({required String id, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainChangeTheme({required String dark, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainChangeLanguage({required String lang, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainVideoSaveDirectory({required bool root, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> mainSetUserDefaultOption(
      {required String key, required String value, dynamic hint}) {
    return js.context.callMethod('getByName', [
      'option:user:default',
      jsonEncode({'name': key, 'value': value})
    ]);
  }

  String mainGetUserDefaultOption({required String key, dynamic hint}) {
    return js.context.callMethod('getByName', ['option:user:default', key]);
  }

  Future<String> mainHandleRelayId({required String id, dynamic hint}) {
    var newId = id;
    if (id.endsWith("\\r") || id.endsWith("/r")) {
      newId = id.substring(0, id.length - 2);
    }
    return Future.value(newId);
  }

  String mainGetMainDisplay({dynamic hint}) {
    return js.context.callMethod('getByName', ['main_display']);
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
    // TODO: ?
    throw UnimplementedError();
  }

  Future<void> mainDeviceName({required String name, dynamic hint}) {
    // TODO: ?
    throw UnimplementedError();
  }

  Future<void> mainRemovePeer({required String id, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['remove', id]));
  }

  bool mainHasHwcodec({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasVram({dynamic hint}) {
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
    return Future(
        () => js.context.callMethod('setByName', ['send_mouse', msg]));
  }

  Future<void> sessionRestartRemoteDevice(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(() => js.context.callMethod('setByName', ['restart']));
  }

  String sessionGetAuditServerSync(
      {required UuidValue sessionId, required String typ, dynamic hint}) {
    return js.context.callMethod('getByName', ['audit_server', typ]);
  }

  Future<void> sessionSendNote(
      {required UuidValue sessionId, required String note, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> sessionAlternativeCodecs(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(
        () => js.context.callMethod('getByName', ['alternative_codecs']));
  }

  Future<void> sessionChangePreferCodec(
      {required UuidValue sessionId, dynamic hint}) {
    return Future(
        () => js.context.callMethod('setByName', ['change_prefer_codec']));
  }

  Future<void> sessionOnWaitingForImageDialogShow(
      {required UuidValue sessionId, dynamic hint}) {
    return Future.value();
  }

  Future<void> sessionToggleVirtualDisplay(
      {required UuidValue sessionId,
      required int index,
      required bool on,
      dynamic hint}) {
    // TODO
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
    // TODO:
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
    // TODO
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
    return 0;
  }

  void sessionNextRgba(
      {required UuidValue sessionId, required int display, dynamic hint}) {}

  void sessionRegisterPixelbufferTexture(
      {required UuidValue sessionId,
      required int display,
      required int ptr,
      dynamic hint}) {}

  void sessionRegisterGpuTexture(
      {required UuidValue sessionId,
      required int display,
      required int ptr,
      dynamic hint}) {}

  Future<void> queryOnlines({required List<String> ids, dynamic hint}) {
    return Future(() =>
        js.context.callMethod('setByName', ['query_onlines', jsonEncode(ids)]));
  }

  // Dup to the function in hbb_common, lib.rs
  // Maybe we need to move this function to js part.
  int versionToNumber({required String v, dynamic hint}) {
    return int.tryParse(
            js.context.callMethod('getByName', ['get_version_number', v])) ??
        0;
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
    return false;
  }

  bool mainIsLoginWayland({dynamic hint}) {
    return false;
  }

  Future<void> mainStartPa({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHideDocker({dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainHasPixelbufferTextureRender({dynamic hint}) {
    return false;
  }

  bool mainHasFileClipboard({dynamic hint}) {
    return false;
  }

  bool mainHasGpuTextureRender({dynamic hint}) {
    return false;
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
    return versionToNumber(v: version) > versionToNumber(v: '1.2.4');
  }

  bool isSelinuxEnforcing({dynamic hint}) {
    return false;
  }

  String mainDefaultPrivacyModeImpl({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainSupportedPrivacyModeImpls({dynamic hint}) {
    throw UnimplementedError();
  }

  String mainSupportedInputSource({dynamic hint}) {
    return jsonEncode([
      ['Input source 2', 'input_source_2_tip']
    ]);
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

  Future<void> mainCheckHwcodec({dynamic hint}) {
    throw UnimplementedError();
  }

  Future<void> sessionRequestNewDisplayInitMsgs(
      {required UuidValue sessionId, required int display, dynamic hint}) {
    throw UnimplementedError();
  }

  Future<String> mainHandleWaylandScreencastRestoreToken(
      {required String key, required String value, dynamic hint}) {
    throw UnimplementedError();
  }

  bool mainIsOptionFixed({required String key, dynamic hint}) {
    throw UnimplementedError();
  }

  void dispose() {}
}

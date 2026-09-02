import 'dart:async';
import 'dart:convert';

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:get/get.dart';

import '../common.dart';
import '../utils/http_service.dart' as http;
import 'model.dart';
import 'platform_model.dart';

bool refreshingUser = false;

class UserModel {
  final RxString userName = ''.obs;
  final RxString displayName = ''.obs;
  final RxString avatar = ''.obs;
  final RxBool isAdmin = false.obs;
  final RxString networkError = ''.obs;
  // True when networkError carries a server-reported error rather than a
  // connectivity failure; netWorkErrorWidget hides the network tip then.
  final RxBool networkErrorFromServer = false.obs;
  bool get isLogin => userName.isNotEmpty;
  String get displayNameOrUserName =>
      displayName.value.trim().isEmpty ? userName.value : displayName.value;
  String get accountLabelWithHandle {
    final username = userName.value.trim();
    if (username.isEmpty) {
      return '';
    }
    final preferred = displayName.value.trim();
    if (preferred.isEmpty || preferred == username) {
      return username;
    }
    return '$preferred (@$username)';
  }

  WeakReference<FFI> parent;

  UserModel(this.parent) {
    userName.listen((p0) {
      // When user name becomes empty, show login button
      // When user name becomes non-empty:
      //  For _updateLocalUserInfo, network error will be set later
      //  For login success, should clear network error
      networkError.value = '';
    });
  }

  // Bumped on every server switch, so a refresh that was in flight against
  // the previous server cannot apply its late response to the new session.
  int _sessionEpoch = 0;

  void refreshCurrentUser() async {
    if (bind.isDisableAccount()) return;
    // Wait out a server switch in progress, so that the token read below is
    // never the previous server's while the API server is already the new one.
    await _sessionWrites;
    final epoch = _sessionEpoch;
    networkError.value = '';
    networkErrorFromServer.value = false;
    final token = bind.mainGetLocalOption(key: 'access_token');
    if (token == '') {
      await updateOtherModels();
      return;
    }
    _updateLocalUserInfo();
    final url = await bind.mainGetApiServer();
    final body = {
      'id': await bind.mainGetMyId(),
      'uuid': await bind.mainGetUuid()
    };
    if (refreshingUser) return;
    try {
      refreshingUser = true;
      final http.Response response;
      try {
        response = await http.post(Uri.parse('$url/api/currentUser'),
            headers: {
              'Content-Type': 'application/json',
              'Authorization': 'Bearer $token'
            },
            body: json.encode(body));
      } catch (e) {
        if (epoch == _sessionEpoch) {
          networkError.value = e.toString();
        }
        rethrow;
      }
      refreshingUser = false;
      if (epoch != _sessionEpoch) {
        // The response belongs to the session this refresh started with,
        // not to the one now active.
        return;
      }
      final status = response.statusCode;
      if (status == 401 || status == 400) {
        // Queued behind a server switch in progress and re-checked, so a
        // reset that began before the switch cannot clear the session the
        // switch restores.
        _serializeSessionWrite(() async {
          if (epoch != _sessionEpoch) return;
          await reset(resetOther: status == 401);
        });
        return;
      }
      final data = json.decode(decode_http_response(response));
      final error = data['error'];
      if (error != null) {
        // The only failure known to come from the server itself, so the
        // check-your-network tip does not apply. Flag before the message is
        // set in the catch below so rebuilds read a consistent pair.
        networkErrorFromServer.value = true;
        throw error;
      }

      final user = UserPayload.fromJson(data);
      _parseAndUpdateUser(user);
    } catch (e) {
      debugPrint('Failed to refreshCurrentUser: $e');
      // Surface failures in the address book / group tabs, which offer a
      // retry. Anything not flagged above -- transport errors, non-JSON or
      // unexpected-schema bodies (e.g. a filter's block page) -- keeps the
      // check-your-network tip.
      if (epoch == _sessionEpoch && networkError.value.isEmpty) {
        networkError.value = e.toString();
      }
    } finally {
      refreshingUser = false;
      if (epoch != _sessionEpoch) {
        // A refresh for the session that took over meanwhile was suppressed
        // by refreshingUser while this one ran, so issue it now.
        refreshCurrentUser();
      }
      await updateOtherModels();
    }
  }

  static Map<String, dynamic>? getLocalUserInfo() {
    final userInfo = bind.mainGetLocalOption(key: 'user_info');
    if (userInfo == '') {
      return null;
    }
    try {
      return json.decode(userInfo);
    } catch (e) {
      debugPrint('Failed to get local user info "$userInfo": $e');
    }
    return null;
  }

  _updateLocalUserInfo() {
    final userInfo = getLocalUserInfo();
    if (userInfo != null) {
      userName.value = (userInfo['name'] ?? '').toString();
      displayName.value = (userInfo['display_name'] ?? '').toString();
      avatar.value = (userInfo['avatar'] ?? '').toString();
    }
  }

  Future<void> reset({bool resetOther = false}) async {
    await bind.mainSetLocalOption(key: 'access_token', value: '');
    await bind.mainSetLocalOption(key: 'user_info', value: '');
    if (resetOther) {
      await gFFI.abModel.reset();
      await gFFI.groupModel.reset();
    }
    userName.value = '';
    displayName.value = '';
    avatar.value = '';
  }

  _parseAndUpdateUser(UserPayload user) {
    userName.value = user.name;
    displayName.value = user.displayName;
    avatar.value = user.avatar;
    isAdmin.value = user.isAdmin;
    bind.mainSetLocalOption(key: 'user_info', value: jsonEncode(user));
    if (isWeb) {
      // ugly here, tmp solution
      bind.mainSetLocalOption(key: 'verifier', value: user.verifier ?? '');
    }
  }

  // update ab and group status
  static Future<void> updateOtherModels() async {
    await Future.wait([
      gFFI.abModel.pullAb(force: ForcePullAb.listAndCurrent, quiet: false),
      gFFI.groupModel.pull()
    ]);
  }

  // Sessions parked per API server while another server is in use, so that
  // switching back restores the login instead of requiring a new one.
  static const String _kParkedSessions = 'parked_sessions';

  // Writes to the active session are serialized, so that a switch and a
  // 401 reset cannot interleave their multi-step storage updates.
  Future<void> _sessionWrites = Future.value();

  Future<T> _serializeSessionWrite<T>(Future<T> Function() write) {
    final result = _sessionWrites.then((_) => write());
    _sessionWrites = result.then((_) {}, onError: (_) {});
    return result;
  }

  /// Called when the API server changes: park the session of the server being
  /// left (the token is not invalidated, unlike [logOut]) and restore the one
  /// parked for the server being switched to, if any.
  Future<void> switchApiServer(String oldApiServer, String newApiServer) =>
      _serializeSessionWrite(
          () => _switchApiServer(oldApiServer, newApiServer));

  Future<void> _switchApiServer(
      String oldApiServer, String newApiServer) async {
    _sessionEpoch++;
    Map<String, dynamic> parked = {};
    final raw = bind.mainGetLocalOption(key: _kParkedSessions);
    if (raw.isNotEmpty) {
      try {
        parked = json.decode(raw);
      } catch (e) {
        debugPrint('Failed to decode parked sessions: $e');
      }
    }
    final token = bind.mainGetLocalOption(key: 'access_token');
    if (token.isNotEmpty) {
      parked[oldApiServer] = {
        'access_token': token,
        'user_info': bind.mainGetLocalOption(key: 'user_info'),
      };
    } else {
      parked.remove(oldApiServer);
    }
    final restored = parked[newApiServer];
    // Park before clearing the active session, and unpark only after the
    // restored one is active again, so a crash in between cannot leave
    // either session nowhere.
    await bind.mainSetLocalOption(
        key: _kParkedSessions, value: json.encode(parked));
    await reset(resetOther: true);
    // Bump again now that the old token is gone: a refresh that a stale one
    // re-issued while this was awaiting saw the new server with the old token,
    // and its 401 must not reset the session restored below. Bumping before
    // the restore, rather than after, leaves no moment in which a refresh can
    // hold the final epoch together with that token.
    _sessionEpoch++;
    if (restored is Map) {
      await bind.mainSetLocalOption(
          key: 'access_token',
          value: (restored['access_token'] ?? '').toString());
      await bind.mainSetLocalOption(
          key: 'user_info', value: (restored['user_info'] ?? '').toString());
      parked.remove(newApiServer);
      await bind.mainSetLocalOption(
          key: _kParkedSessions, value: json.encode(parked));
      // An expired or revoked token comes back 401 here, which resets the
      // session, so a stale parked login cleans itself up.
      refreshCurrentUser();
    }
  }

  Future<void> logOut({String? apiServer}) async {
    final tag = gFFI.dialogManager.showLoading(translate('Waiting'));
    try {
      final url = apiServer ?? await bind.mainGetApiServer();
      final authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      await http
          .post(Uri.parse('$url/api/logout'),
              body: jsonEncode({
                'id': await bind.mainGetMyId(),
                'uuid': await bind.mainGetUuid(),
              }),
              headers: authHeaders)
          .timeout(Duration(seconds: 2));
    } catch (e) {
      debugPrint("request /api/logout failed: err=$e");
    } finally {
      await reset(resetOther: true);
      gFFI.dialogManager.dismissByTag(tag);
    }
  }

  /// throw [RequestException]
  Future<LoginResponse> login(LoginRequest loginRequest) async {
    final url = await bind.mainGetApiServer();
    final resp = await http.post(Uri.parse('$url/api/login'),
        body: jsonEncode(loginRequest.toJson()));

    final Map<String, dynamic> body;
    try {
      body = jsonDecode(decode_http_response(resp));
    } catch (e) {
      debugPrint("login: jsonDecode resp body failed: ${e.toString()}");
      if (resp.statusCode != 200) {
        BotToast.showText(
            contentColor: Colors.red, text: 'HTTP ${resp.statusCode}');
      }
      rethrow;
    }
    if (resp.statusCode != 200) {
      throw RequestException(resp.statusCode, body['error'] ?? '');
    }
    if (body['error'] != null) {
      throw RequestException(0, body['error']);
    }

    return getLoginResponseFromAuthBody(body);
  }

  LoginResponse getLoginResponseFromAuthBody(Map<String, dynamic> body) {
    final LoginResponse loginResponse;
    try {
      loginResponse = LoginResponse.fromJson(body);
    } catch (e) {
      debugPrint("login: jsonDecode LoginResponse failed: ${e.toString()}");
      rethrow;
    }

    final isLogInDone = loginResponse.type == HttpType.kAuthResTypeToken &&
        loginResponse.access_token != null;
    if (isLogInDone && loginResponse.user != null) {
      _parseAndUpdateUser(loginResponse.user!);
    }

    return loginResponse;
  }

  /// Throws on network failures, non-success responses, and invalid response
  /// data. Returns an empty list when no API server is configured or a
  /// successful response contains no third-party login options.
  static Future<List<dynamic>> queryOidcLoginOptions() async {
    final url = await bind.mainGetApiServer();
    if (url.trim().isEmpty) return [];
    final resp = await http.get(Uri.parse('$url/api/login-options'));
    const successStatusCodeStart = 200;
    const successStatusCodeEnd = 300;
    if (resp.statusCode < successStatusCodeStart ||
        resp.statusCode >= successStatusCodeEnd) {
      throw RequestException(
          resp.statusCode, resp.reasonPhrase ?? 'Request failed');
    }
    final List<String> ops = [];
    for (final item in jsonDecode(resp.body)) {
      ops.add(item as String);
    }
    for (final item in ops) {
      if (item.startsWith('common-oidc/')) {
        return jsonDecode(item.substring('common-oidc/'.length));
      }
    }
    return ops
        .where((item) => item.startsWith('oidc/'))
        .map((item) => {'name': item.substring('oidc/'.length)})
        .toList();
  }
}

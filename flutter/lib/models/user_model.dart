import 'dart:async';
import 'dart:convert';

import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/common/widgets/peer_tab_page.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

import '../common.dart';
import 'model.dart';
import 'platform_model.dart';

class UserModel {
  final RxString userName = ''.obs;
  final RxString groupName = ''.obs;
  final RxBool isAdmin = false.obs;
  WeakReference<FFI> parent;

  UserModel(this.parent);

  void refreshCurrentUser() async {
    final token = bind.mainGetLocalOption(key: 'access_token');
    if (token == '') {
      await _updateOtherModels();
      return;
    }
    final url = await bind.mainGetApiServer();
    final body = {
      'id': await bind.mainGetMyId(),
      'uuid': await bind.mainGetUuid()
    };
    try {
      final response = await http.post(Uri.parse('$url/api/currentUser'),
          headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Bearer $token'
          },
          body: json.encode(body));
      final status = response.statusCode;
      if (status == 401 || status == 400) {
        reset();
        return;
      }
      final data = json.decode(response.body);
      final error = data['error'];
      if (error != null) {
        throw error;
      }
      await _parseUserInfo(data);
    } catch (e) {
      print('Failed to refreshCurrentUser: $e');
    } finally {
      await _updateOtherModels();
    }
  }

  Future<void> reset() async {
    await bind.mainSetLocalOption(key: 'access_token', value: '');
    await bind.mainSetLocalOption(key: 'user_info', value: '');
    await gFFI.abModel.reset();
    await gFFI.groupModel.reset();
    userName.value = '';
    groupName.value = '';
    statePeerTab.check();
  }

  Future<void> _parseUserInfo(dynamic userinfo) async {
    bind.mainSetLocalOption(key: 'user_info', value: jsonEncode(userinfo));
    userName.value = userinfo['name'] ?? '';
    groupName.value = userinfo['grp'] ?? '';
    isAdmin.value = userinfo['is_admin'] == true;
  }

  Future<void> _updateOtherModels() async {
    await gFFI.abModel.pullAb();
    await gFFI.groupModel.pull();
  }

  Future<void> logOut() async {
    final tag = gFFI.dialogManager.showLoading(translate('Waiting'));
    final url = await bind.mainGetApiServer();
    final _ = await http.post(Uri.parse('$url/api/logout'),
        body: {
          'id': await bind.mainGetMyId(),
          'uuid': await bind.mainGetUuid(),
        },
        headers: await getHttpHeaders());
    await reset();
    gFFI.dialogManager.dismissByTag(tag);
  }

  Future<Map<String, dynamic>> login(String userName, String pass) async {
    final url = await bind.mainGetApiServer();
    try {
      final resp = await http.post(Uri.parse('$url/api/login'),
          headers: {'Content-Type': 'application/json'},
          body: jsonEncode({
            'username': userName,
            'password': pass,
            'id': await bind.mainGetMyId(),
            'uuid': await bind.mainGetUuid()
          }));
      final body = jsonDecode(resp.body);
      bind.mainSetLocalOption(
          key: 'access_token', value: body['access_token'] ?? '');
      await _parseUserInfo(body['user']);
      return body;
    } catch (err) {
      return {'error': '$err'};
    } finally {
      await _updateOtherModels();
    }
  }
}

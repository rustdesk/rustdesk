import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

import 'model.dart';
import 'platform_model.dart';

class UserModel extends ChangeNotifier {
  var userName = "".obs;
  WeakReference<FFI> parent;

  UserModel(this.parent);

  Future<String> getUserName() async {
    if (userName.isNotEmpty) {
      return userName.value;
    }
    final userInfo = await bind.mainGetLocalOption(key: 'user_info');
    if (userInfo.trim().isEmpty) {
      return "";
    }
    final m = jsonDecode(userInfo);
    userName.value = m['name'] ?? '';
    return userName.value;
  }

  Future<void> logOut() async {
    debugPrint("start logout");
    final url = await bind.mainGetApiServer();
    final _ = await http.post(Uri.parse("$url/api/logout"),
        body: {
          "id": await bind.mainGetMyId(),
          "uuid": await bind.mainGetUuid(),
        },
        headers: await _getHeaders());
    await Future.wait([
      bind.mainSetLocalOption(key: 'access_token', value: ''),
      bind.mainSetLocalOption(key: 'user_info', value: ''),
      bind.mainSetLocalOption(key: 'selected-tags', value: ''),
    ]);
    parent.target?.abModel.clear();
    userName.value = "";
    notifyListeners();
  }

  Future<Map<String, String>>? _getHeaders() {
    return parent.target?.getHttpHeaders();
  }

  Future<Map<String, dynamic>> login(String userName, String pass) async {
    final url = await bind.mainGetApiServer();
    try {
      final resp = await http.post(Uri.parse("$url/api/login"),
          headers: {"Content-Type": "application/json"},
          body: jsonEncode({
            "username": userName,
            "password": pass,
            "id": await bind.mainGetMyId(),
            "uuid": await bind.mainGetUuid()
          }));
      final body = jsonDecode(resp.body);
      bind.mainSetLocalOption(
          key: "access_token", value: body['access_token'] ?? "");
      bind.mainSetLocalOption(
          key: "user_info", value: jsonEncode(body['user']));
      this.userName.value = body['user']?['name'] ?? "";
      return body;
    } catch (err) {
      return {"error": "$err"};
    }
  }
}

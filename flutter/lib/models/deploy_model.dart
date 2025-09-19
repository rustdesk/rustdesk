import 'dart:convert';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get_rx/src/rx_types/rx_types.dart';
import '../../utils/http_service.dart' as http;

class DeployModel {
  final RxBool showDeployPage = false.obs;
  final RxBool isDeployed = false.obs;
  final RxBool checking = false.obs;
  final RxBool deploying = false.obs;
  final RxString error = ''.obs;
  final RxString team = ''.obs;
  final RxString group = ''.obs;
  final RxString user = ''.obs;

  Future<void> checkDeploy() async {
    if (!withPublic()) {
      return;
    }
    try {
      checking.value = true;
      error.value = '';
      isDeployed.value = false;
      team.value = '';
      group.value = '';
      final api = "${await bind.mainGetApiServer()}/api/deploy/state";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode(
          {'id': await bind.mainGetMyId(), "uuid": await bind.mainGetUuid()});
      final resp =
          await http.post(Uri.parse(api), headers: headers, body: body);
      Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
      if (json['team'] != null) {
        team.value = json['team'];
      }
      if (json['group'] != null) {
        group.value = json['group'];
      }
      if (json['user'] != null) {
        user.value = json['user'];
      }
      isDeployed.value = team.isNotEmpty;
    } catch (e) {
      error.value = e.toString();
    } finally {
      checking.value = false;
    }
  }

  Future<DeployWithCodeResponse?> deployWithCodeRequest(String code) async {
    if (!withPublic()) {
      return null;
    }
    try {
      deploying.value = true;
      error.value = '';
      final api = "${await bind.mainGetApiServer()}/api/deploy/code/request";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        'id': await bind.mainGetMyId(),
        "uuid": await bind.mainGetUuid(),
        'code': code,
      });
      final resp =
          await http.post(Uri.parse(api), headers: headers, body: body);
      if (resp.body.isEmpty) {
        throw "Empty response";
      }
      Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
      final codeResp = DeployWithCodeResponse.fromJson(json);
      deploying.value = false;
      return codeResp;
    } catch (e) {
      error.value = e.toString();
    } finally {
      deploying.value = false;
    }
    return null;
  }

  Future<void> deployWithCode(String code) async {
    if (!withPublic()) {
      return;
    }
    try {
      deploying.value = true;
      error.value = '';
      final api = "${await bind.mainGetApiServer()}/api/deploy/code";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        'id': await bind.mainGetMyId(),
        "uuid": await bind.mainGetUuid(),
        'code': code,
        'type': deployType,
      });
      final resp =
          await http.post(Uri.parse(api), headers: headers, body: body);
      if (resp.body.isNotEmpty) {
        Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
        if (json.containsKey('error')) {
          throw json['error'];
        }
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
    } catch (e) {
      error.value = e.toString();
    } finally {
      deploying.value = false;
    }
  }

  Future<void> deployWithAccount(String email, String password) async {
    if (!withPublic()) {
      return;
    }
    try {
      deploying.value = true;
      error.value = '';
      final api = "${await bind.mainGetApiServer()}/api/deploy/account";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        'id': await bind.mainGetMyId(),
        "uuid": await bind.mainGetUuid(),
        'email': email,
        'password': password,
        'type': deployType,
      });
      final resp =
          await http.post(Uri.parse(api), headers: headers, body: body);
      if (resp.body.isNotEmpty) {
        Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
        if (json.containsKey('error')) {
          throw json['error'];
        }
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
    } catch (e) {
      error.value = e.toString();
    } finally {
      deploying.value = false;
    }
  }

  Future<void> deployToLoginUser() async {
    if (!withPublic()) {
      return;
    }
    try {
      deploying.value = true;
      error.value = '';
      final api = "${await bind.mainGetApiServer()}/api/deploy/login-account";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        'id': await bind.mainGetMyId(),
        "uuid": await bind.mainGetUuid(),
      });
      final resp =
          await http.post(Uri.parse(api), headers: headers, body: body);
      if (resp.body.isNotEmpty) {
        Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
        if (json.containsKey('error')) {
          throw json['error'];
        }
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
    } catch (e) {
      error.value = e.toString();
    } finally {
      deploying.value = false;
    }
  }

  int get deployType {
    if (bind.isStandard()) {
      return 1;
    } else if (bind.isHost()) {
      return 2;
    } else {
      return 0;
    }
  }
}

class DeployWithCodeResponse {
  final String team;
  final String email;
  final String group;

  DeployWithCodeResponse({
    required this.team,
    required this.email,
    required this.group,
  });

  factory DeployWithCodeResponse.fromJson(Map<String, dynamic> json) {
    return DeployWithCodeResponse(
      team: json['team'] ?? '',
      email: json['email'] ?? '',
      group: json['group'] ?? '',
    );
  }
}

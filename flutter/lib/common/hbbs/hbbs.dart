import 'package:flutter_hbb/models/peer_model.dart';

class HttpType {
  static const kAuthReqTypeAccount = "account";
  static const kAuthReqTypeMobile = "mobile";
  static const kAuthReqTypeSMSCode = "sms_code";
  static const kAuthReqTypeEmailCode = "email_code";

  static const kAuthResTypeToken = "access_token";
  static const kAuthResTypeEmailCheck = "email_check";
}

class UserPayload {
  String name = '';
  String email = '';
  String note = '';
  int? status;
  String grp = '';
  bool isAdmin = false;

  UserPayload.fromJson(Map<String, dynamic> json)
      : name = json['name'] ?? '',
        email = json['email'] ?? '',
        note = json['note'] ?? '',
        status = json['status'],
        grp = json['grp'] ?? '',
        isAdmin = json['is_admin'] == true;
}

class PeerPayload {
  String id = '';
  String info = '';
  int? status;
  String user = '';
  String user_name = '';
  String note = '';

  PeerPayload.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        info = json['info'] ?? '',
        status = json['status'],
        user = json['user'] ?? '',
        user_name = json['user_name'] ?? '',
        note = json['note'] ?? '';

  static Peer toPeer(PeerPayload p) {
    return Peer.fromJson({"id": p.id});
  }
}

class LoginRequest {
  String? username;
  String? password;
  String? id;
  String? uuid;
  bool? autoLogin;
  String? type;
  String? verificationCode;
  String? deviceInfo;

  LoginRequest(
      {this.username,
      this.password,
      this.id,
      this.uuid,
      this.autoLogin,
      this.type,
      this.verificationCode,
      this.deviceInfo});

  LoginRequest.fromJson(Map<String, dynamic> json) {
    username = json['username'];
    password = json['password'];
    id = json['id'];
    uuid = json['uuid'];
    autoLogin = json['autoLogin'];
    type = json['type'];
    verificationCode = json['verificationCode'];
    deviceInfo = json['deviceInfo'];
  }

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> data = <String, dynamic>{};
    data['username'] = username ?? '';
    data['password'] = password ?? '';
    data['id'] = id ?? '';
    data['uuid'] = uuid ?? '';
    data['autoLogin'] = autoLogin ?? '';
    data['type'] = type ?? '';
    data['verificationCode'] = verificationCode ?? '';
    data['deviceInfo'] = deviceInfo ?? '';
    return data;
  }
}

class LoginResponse {
  String? access_token;
  String? type;
  UserPayload? user;

  LoginResponse({this.access_token, this.type, this.user});

  LoginResponse.fromJson(Map<String, dynamic> json) {
    access_token = json['access_token'];
    type = json['type'];
    print("user: ${json['user']}");
    print("user id: ${json['user']['id']}");
    print("user name: ${json['user']['name']}");
    print("user email: ${json['user']['id']}");
    print("user note: ${json['user']['note']}");
    print("user status: ${json['user']['status']}");
    print("user grp: ${json['user']['grp']}");
    print("user is_admin: ${json['user']['is_admin']}");
    user = json['user'] != null ? UserPayload.fromJson(json['user']) : null;
  }
}

class RequestException implements Exception {
  int statusCode;
  String cause;
  RequestException(this.statusCode, this.cause);

  @override
  String toString() {
    return "RequestException, statusCode: $statusCode, error: $cause";
  }
}

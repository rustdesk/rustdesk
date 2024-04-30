import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';

import 'package:flutter_hbb/models/peer_model.dart';

import '../../models/platform_model.dart';

class HttpType {
  static const kAuthReqTypeAccount = "account";
  static const kAuthReqTypeMobile = "mobile";
  static const kAuthReqTypeSMSCode = "sms_code";
  static const kAuthReqTypeEmailCode = "email_code";
  static const kAuthReqTypeTfaCode = "tfa_code";

  static const kAuthResTypeToken = "access_token";
  static const kAuthResTypeEmailCheck = "email_check";
  static const kAuthResTypeTfaCheck = "tfa_check";
}

enum UserStatus { kDisabled, kNormal, kUnverified }

// to-do: The UserPayload does not contain all the fields of the user.
// Is all the fields of the user needed?
class UserPayload {
  String name = '';
  String email = '';
  String note = '';
  UserStatus status;
  bool isAdmin = false;

  UserPayload.fromJson(Map<String, dynamic> json)
      : name = json['name'] ?? '',
        email = json['email'] ?? '',
        note = json['note'] ?? '',
        status = json['status'] == 0
            ? UserStatus.kDisabled
            : json['status'] == -1
                ? UserStatus.kUnverified
                : UserStatus.kNormal,
        isAdmin = json['is_admin'] == true;

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> map = {
      'name': name,
      'status': status == UserStatus.kDisabled
          ? 0
          : status == UserStatus.kUnverified
              ? -1
              : 1,
    };
    return map;
  }

  Map<String, dynamic> toGroupCacheJson() {
    final Map<String, dynamic> map = {
      'name': name,
    };
    return map;
  }
}

class PeerPayload {
  String id = '';
  Map<String, dynamic> info = {};
  int? status;
  String user = '';
  String user_name = '';
  String note = '';

  PeerPayload.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        info = (json['info'] is Map<String, dynamic>) ? json['info'] : {},
        status = json['status'],
        user = json['user'] ?? '',
        user_name = json['user_name'] ?? '',
        note = json['note'] ?? '';

  static Peer toPeer(PeerPayload p) {
    return Peer.fromJson({
      "id": p.id,
      'loginName': p.user_name,
      "username": p.info['username'] ?? '',
      "platform": _platform(p.info['os']),
      "hostname": p.info['device_name'],
    });
  }

  static String? _platform(dynamic field) {
    if (field == null) {
      return null;
    }
    final fieldStr = field.toString();
    List<String> list = fieldStr.split(' / ');
    if (list.isEmpty) return null;
    final os = list[0];
    switch (os.toLowerCase()) {
      case 'windows':
        return kPeerPlatformWindows;
      case 'linux':
        return kPeerPlatformLinux;
      case 'macos':
        return kPeerPlatformMacOS;
      case 'android':
        return kPeerPlatformAndroid;
      default:
        if (fieldStr.toLowerCase().contains('linux')) {
          return kPeerPlatformLinux;
        }
        return null;
    }
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
  String? tfaCode;
  String? secret;

  LoginRequest(
      {this.username,
      this.password,
      this.id,
      this.uuid,
      this.autoLogin,
      this.type,
      this.verificationCode,
      this.tfaCode,
      this.secret});

  Map<String, dynamic> toJson() {
    final Map<String, dynamic> data = <String, dynamic>{};
    if (username != null) data['username'] = username;
    if (password != null) data['password'] = password;
    if (id != null) data['id'] = id;
    if (uuid != null) data['uuid'] = uuid;
    if (autoLogin != null) data['autoLogin'] = autoLogin;
    if (type != null) data['type'] = type;
    if (verificationCode != null) {
      data['verificationCode'] = verificationCode;
    }
    if (tfaCode != null) data['tfaCode'] = tfaCode;
    if (secret != null) data['secret'] = secret;

    Map<String, dynamic> deviceInfo = {};
    try {
      deviceInfo = jsonDecode(bind.mainGetLoginDeviceInfo());
    } catch (e) {
      debugPrint('Failed to decode get device info: $e');
    }
    data['deviceInfo'] = deviceInfo;
    return data;
  }
}

class LoginResponse {
  String? access_token;
  String? type;
  String? tfa_type;
  String? secret;
  UserPayload? user;

  LoginResponse(
      {this.access_token, this.type, this.tfa_type, this.secret, this.user});

  LoginResponse.fromJson(Map<String, dynamic> json) {
    access_token = json['access_token'];
    type = json['type'];
    tfa_type = json['tfa_type'];
    secret = json['secret'];
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

enum ShareRule {
  read(1),
  readWrite(2),
  fullControl(3);

  const ShareRule(this.value);
  final int value;

  static String desc(int v) {
    if (v == ShareRule.read.value) {
      return translate('Read-only');
    }
    if (v == ShareRule.readWrite.value) {
      return translate('Read/Write');
    }
    if (v == ShareRule.fullControl.value) {
      return translate('Full Control');
    }
    return v.toString();
  }

  static String shortDesc(int v) {
    if (v == ShareRule.read.value) {
      return 'R';
    }
    if (v == ShareRule.readWrite.value) {
      return 'RW';
    }
    if (v == ShareRule.fullControl.value) {
      return 'F';
    }
    return v.toString();
  }

  static ShareRule? fromValue(int v) {
    if (v == ShareRule.read.value) {
      return ShareRule.read;
    }
    if (v == ShareRule.readWrite.value) {
      return ShareRule.readWrite;
    }
    if (v == ShareRule.fullControl.value) {
      return ShareRule.fullControl;
    }
    return null;
  }
}

class AbProfile {
  String guid;
  String name;
  String owner;
  String? note;
  int rule;

  AbProfile(this.guid, this.name, this.owner, this.note, this.rule);

  AbProfile.fromJson(Map<String, dynamic> json)
      : guid = json['guid'] ?? '',
        name = json['name'] ?? '',
        owner = json['owner'] ?? '',
        note = json['note'] ?? '',
        rule = json['rule'] ?? 0;
}

class AbTag {
  String name;
  int color;

  AbTag(this.name, this.color);

  AbTag.fromJson(Map<String, dynamic> json)
      : name = json['name'] ?? '',
        color = json['color'] ?? '';
}

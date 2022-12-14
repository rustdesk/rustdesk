import 'package:flutter_hbb/models/peer_model.dart';

class UserPayload {
  String name = '';
  String email = '';
  String note = '';
  int? status;
  String grp = '';
  bool is_admin = false;

  UserPayload.fromJson(Map<String, dynamic> json)
      : name = json['name'] ?? '',
        email = json['email'] ?? '',
        note = json['note'] ?? '',
        status = json['status'],
        grp = json['grp'] ?? '',
        is_admin = json['is_admin'] == true;
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

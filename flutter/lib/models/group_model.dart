import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'dart:convert';
import 'package:http/http.dart' as http;

class GroupModel {
  final RxBool groupLoading = false.obs;
  final RxString groupLoadError = "".obs;
  final RxBool peerLoading = false.obs; //to-do: not used
  final RxString peerLoadError = "".obs;
  final RxString groupName = ''.obs;
  final RxString groupId = ''.obs;
  final RxList<UserPayload> users = RxList.empty(growable: true);
  final RxList<PeerPayload> peerPayloads = RxList.empty(growable: true);
  final RxList<Peer> peersShow = RxList.empty(growable: true);
  WeakReference<FFI> parent;

  GroupModel(this.parent);

  Future<void> reset() async {
    groupLoading.value = false;
    groupLoadError.value = "";
    peerLoading.value = false;
    peerLoadError.value = "";
    groupName.value = '';
    groupId.value = '';
    users.clear();
    peerPayloads.clear();
    peersShow.clear();
  }

  Future<void> pull() async {
    await reset();
    if (bind.mainGetLocalOption(key: 'access_token') == '') {
      return;
    }
    try {
      if (!await _getGroup()) {
        reset();
        return;
      }
    } catch (e) {
      debugPrintStack(label: '$e');
      reset();
      return;
    }
    if (gFFI.userModel.userName.isEmpty ||
        (gFFI.userModel.isAdmin.isFalse && groupName.isEmpty)) {
      gFFI.peerTabModel.check_dynamic_tabs();
      return;
    }
    groupLoading.value = true;
    groupLoadError.value = "";
    final api = "${await bind.mainGetApiServer()}/api/users";
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 20;
      var total = 0;
      int current = 0;
      do {
        current += 1;
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: {
              'current': current.toString(),
              'pageSize': pageSize.toString(),
              if (gFFI.userModel.isAdmin.isFalse) 'grp': groupId.value,
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
          Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
          if (json.containsKey('error')) {
            throw json['error'];
          } else {
            if (json.containsKey('total')) {
              if (total == 0) total = json['total'];
              if (json.containsKey('data')) {
                final data = json['data'];
                if (data is List) {
                  for (final user in data) {
                    final u = UserPayload.fromJson(user);
                    if (!users.any((e) => e.name == u.name)) {
                      users.add(u);
                    }
                  }
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
    } catch (err) {
      debugPrintStack(label: '$err');
      groupLoadError.value = err.toString();
    } finally {
      groupLoading.value = false;
      gFFI.peerTabModel.check_dynamic_tabs();
    }
  }

  Future<bool> _getGroup() async {
    final url = await bind.mainGetApiServer();
    final body = {
      'id': await bind.mainGetMyId(),
      'uuid': await bind.mainGetUuid()
    };
    try {
      final response = await http.post(Uri.parse('$url/api/currentGroup'),
          headers: getHttpHeaders(), body: json.encode(body));
      final status = response.statusCode;
      if (status == 401 || status == 400) {
        return false;
      }
      final data = json.decode(utf8.decode(response.bodyBytes));
      final error = data['error'];
      if (error != null) {
        throw error;
      }
      groupName.value = data['name'] ?? '';
      groupId.value = data['id'] ?? '';
      return groupId.value.isNotEmpty && groupName.isNotEmpty;
    } catch (e) {
      debugPrintStack(label: '$e');
    } finally {}

    return false;
  }

  Future<void> pullUserPeers(UserPayload user) async {
    peerPayloads.clear();
    peersShow.clear();
    peerLoading.value = true;
    peerLoadError.value = "";
    final api = "${await bind.mainGetApiServer()}/api/peers";
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 20;
      var total = 0;
      int current = 0;
      do {
        current += 1;
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: {
              'current': current.toString(),
              'pageSize': pageSize.toString(),
              'target_user': user.id,
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
          Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
          if (json.containsKey('error')) {
            throw json['error'];
          } else {
            if (json.containsKey('total')) {
              if (total == 0) total = json['total'];
              if (json.containsKey('data')) {
                final data = json['data'];
                if (data is List) {
                  for (final p in data) {
                    final peer = PeerPayload.fromJson(p);
                    peerPayloads.add(peer);
                    peersShow.add(PeerPayload.toPeer(peer));
                  }
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
    } catch (err) {
      debugPrintStack(label: '$err');
      peerLoadError.value = err.toString();
    } finally {
      peerLoading.value = false;
    }
  }
}

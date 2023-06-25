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
  final RxString groupId = ''.obs;
  RxString groupName = ''.obs;
  final RxList<UserPayload> users = RxList.empty(growable: true);
  final RxList<Peer> peersShow = RxList.empty(growable: true);
  final RxString selectedUser = ''.obs;
  final RxString searchUserText = ''.obs;
  WeakReference<FFI> parent;
  var initialized = false;

  GroupModel(this.parent);

  reset() {
    groupName.value = '';
    groupId.value = '';
    users.clear();
    peersShow.clear();
    initialized = false;
  }

  Future<void> pull({force = true, quiet = false}) async {
    /*
    if (!force && initialized) return;
    if (!quiet) {
      groupLoading.value = true;
      groupLoadError.value = "";
    }
    await _pull();
    groupLoading.value = false;
    initialized = true;
    */
  }

  Future<void> _pull() async {
    reset();
    if (bind.mainGetLocalOption(key: 'access_token') == '') {
      return;
    }
    try {
      if (!await _getGroup()) {
        reset();
        return;
      }
    } catch (e) {
      debugPrint('$e');
      reset();
      return;
    }
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
      debugPrint('$err');
      groupLoadError.value = err.toString();
    } finally {
      _pullUserPeers();
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
      debugPrint('$e');
      groupLoadError.value = e.toString();
    } finally {}

    return false;
  }

  Future<void> _pullUserPeers() async {
    peersShow.clear();
    final api = "${await bind.mainGetApiServer()}/api/peers";
    try {
      var uri0 = Uri.parse(api);
      final pageSize =
          20; // ????????????????????????????????????????????????????? stupid stupis, how about >20 peers
      var total = 0;
      int current = 0;
      var queryParameters = {
        'current': current.toString(),
        'pageSize': pageSize.toString(),
      };
      if (!gFFI.userModel.isAdmin.value) {
        queryParameters.addAll({'grp': groupId.value});
      }
      do {
        current += 1;
        var uri = Uri(
            scheme: uri0.scheme,
            host: uri0.host,
            path: uri0.path,
            port: uri0.port,
            queryParameters: queryParameters);
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
                    final peerPayload = PeerPayload.fromJson(p);
                    final peer = PeerPayload.toPeer(peerPayload);
                    if (!peersShow.any((e) => e.id == peer.id)) {
                      peersShow.add(peer);
                    }
                  }
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
    } catch (err) {
      debugPrint('$err');
      groupLoadError.value = err.toString();
    } finally {}
  }
}

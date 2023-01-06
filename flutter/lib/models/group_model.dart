import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/peer_tab_page.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'dart:convert';
import 'package:http/http.dart' as http;

class GroupModel {
  final RxBool userLoading = false.obs;
  final RxString userLoadError = "".obs;
  final RxBool peerLoading = false.obs; //to-do: not used
  final RxString peerLoadError = "".obs;
  final RxList<UserPayload> users = RxList.empty(growable: true);
  final RxList<PeerPayload> peerPayloads = RxList.empty(growable: true);
  final RxList<Peer> peersShow = RxList.empty(growable: true);
  WeakReference<FFI> parent;

  GroupModel(this.parent);

  Future<void> reset() async {
    userLoading.value = false;
    userLoadError.value = "";
    peerLoading.value = false;
    peerLoadError.value = "";
    users.clear();
    peerPayloads.clear();
    peersShow.clear();
  }

  Future<void> pull() async {
    await reset();
    if (gFFI.userModel.userName.isEmpty ||
        (gFFI.userModel.isAdmin.isFalse && gFFI.userModel.groupName.isEmpty)) {
      statePeerTab.check();
      return;
    }
    userLoading.value = true;
    userLoadError.value = "";
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
              if (gFFI.userModel.isAdmin.isFalse)
                'grp': gFFI.userModel.groupName.value,
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
          Map<String, dynamic> json = jsonDecode(resp.body);
          if (json.containsKey('error')) {
            throw json['error'];
          } else {
            if (total == 0) total = json['total'];
            if (json.containsKey('data')) {
              final data = json['data'];
              if (data is List) {
                for (final user in data) {
                  users.add(UserPayload.fromJson(user));
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
    } catch (err) {
      debugPrint('$err');
      userLoadError.value = err.toString();
    } finally {
      userLoading.value = false;
      statePeerTab.check();
    }
  }

  Future<void> pullUserPeers(String username) async {
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
              'grp': gFFI.userModel.groupName.value,
              'target_user': username
            });
        final resp = await http.get(uri, headers: getHttpHeaders());
        if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
          Map<String, dynamic> json = jsonDecode(resp.body);
          if (json.containsKey('error')) {
            throw json['error'];
          } else {
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
      } while (current * pageSize < total);
    } catch (err) {
      debugPrint('$err');
      peerLoadError.value = err.toString();
    } finally {
      peerLoading.value = false;
    }
  }
}

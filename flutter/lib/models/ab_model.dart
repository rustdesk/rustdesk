import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

import '../common.dart';

class AbModel {
  var abLoading = false.obs;
  var abError = "".obs;
  var tags = [].obs;
  var peers = List<Peer>.empty(growable: true).obs;

  var selectedTags = List<String>.empty(growable: true).obs;

  WeakReference<FFI> parent;

  AbModel(this.parent);

  FFI? get _ffi => parent.target;

  Future<dynamic> pullAb() async {
    if (_ffi!.userModel.userName.isEmpty) return;
    abLoading.value = true;
    abError.value = "";
    final api = "${await bind.mainGetApiServer()}/api/ab/get";
    try {
      final resp =
          await http.post(Uri.parse(api), headers: await getHttpHeaders());
      if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
        Map<String, dynamic> json = jsonDecode(resp.body);
        if (json.containsKey('error')) {
          abError.value = json['error'];
        } else if (json.containsKey('data')) {
          final data = jsonDecode(json['data']);
          if (data != null) {
            tags.clear();
            peers.clear();
            if (data['tags'] is List) {
              tags.value = data['tags'];
            }
            if (data['peers'] is List) {
              for (final peer in data['peers']) {
                peers.add(Peer.fromJson(peer));
              }
            }
          }
        }
        return resp.body;
      } else {
        return "";
      }
    } catch (err) {
      err.printError();
      abError.value = err.toString();
    } finally {
      abLoading.value = false;
    }
    return null;
  }

  void reset() {
    tags.clear();
    peers.clear();
  }

  void addId(String id) async {
    if (idContainBy(id)) {
      return;
    }
    peers.add(Peer.fromJson({"id": id}));
  }

  void addTag(String tag) async {
    if (tagContainBy(tag)) {
      return;
    }
    tags.add(tag);
  }

  void changeTagForPeer(String id, List<dynamic> tags) {
    final it = peers.where((element) => element.id == id);
    if (it.isEmpty) {
      return;
    }
    it.first.tags = tags;
  }

  Future<void> pushAb() async {
    abLoading.value = true;
    final api = "${await bind.mainGetApiServer()}/api/ab";
    var authHeaders = await getHttpHeaders();
    authHeaders['Content-Type'] = "application/json";
    final peersJsonData = peers.map((e) => e.toJson()).toList();
    final body = jsonEncode({
      "data": jsonEncode({"tags": tags, "peers": peersJsonData})
    });
    try {
      final resp =
          await http.post(Uri.parse(api), headers: authHeaders, body: body);
      abError.value = "";
      await pullAb();
      debugPrint("resp: ${resp.body}");
    } catch (e) {
      abError.value = e.toString();
    } finally {
      abLoading.value = false;
    }
  }

  bool idContainBy(String id) {
    return peers.where((element) => element.id == id).isNotEmpty;
  }

  bool tagContainBy(String tag) {
    return tags.where((element) => element == tag).isNotEmpty;
  }

  void deletePeer(String id) {
    peers.removeWhere((element) => element.id == id);
  }

  void deleteTag(String tag) {
    gFFI.abModel.selectedTags.remove(tag);
    tags.removeWhere((element) => element == tag);
    for (var peer in peers) {
      if (peer.tags.isEmpty) {
        continue;
      }
      if (peer.tags.contains(tag)) {
        ((peer.tags)).remove(tag);
      }
    }
  }

  void unsetSelectedTags() {
    selectedTags.clear();
  }

  List<dynamic> getPeerTags(String id) {
    final it = peers.where((p0) => p0.id == id);
    if (it.isEmpty) {
      return [];
    } else {
      return it.first.tags;
    }
  }

  void setPeerAlias(String id, String value) {
    final it = peers.where((p0) => p0.id == id);
    if (it.isEmpty) {
      debugPrint("$id is not exists");
      return;
    } else {
      it.first.alias = value;
    }
  }

  void clear() {
    peers.clear();
    tags.clear();
  }
}

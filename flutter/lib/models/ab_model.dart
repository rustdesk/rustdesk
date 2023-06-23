import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';
import 'package:http/http.dart' as http;

import '../common.dart';

class AbModel {
  final abLoading = false.obs;
  final abError = "".obs;
  final tags = [].obs;
  final peers = List<Peer>.empty(growable: true).obs;

  final selectedTags = List<String>.empty(growable: true).obs;
  var initialized = false;

  WeakReference<FFI> parent;

  AbModel(this.parent);

  Future<void> pullAb({force = true, quiet = false}) async {
    if (gFFI.userModel.userName.isEmpty) return;
    if (abLoading.value) return;
    if (!force && initialized) return;
    if (!quiet) {
      abLoading.value = true;
      abError.value = "";
    }
    final api = "${await bind.mainGetApiServer()}/api/ab/get";
    try {
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      final resp = await http.post(Uri.parse(api), headers: authHeaders);
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
      }
    } catch (err) {
      abError.value = err.toString();
    } finally {
      abLoading.value = false;
      initialized = true;
    }
  }

  Future<void> reset() async {
    await bind.mainSetLocalOption(key: "selected-tags", value: '');
    tags.clear();
    peers.clear();
    initialized = false;
  }

  void addId(String id, String alias, List<dynamic> tags) {
    if (idContainBy(id)) {
      return;
    }
    final peer = Peer.fromJson({
      'id': id,
      'alias': alias,
      'tags': tags,
    });
    peers.add(peer);
  }

  void addPeer(Peer peer) {
    peers.removeWhere((e) => e.id == peer.id);
    peers.add(peer);
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
    final api = "${await bind.mainGetApiServer()}/api/ab";
    var authHeaders = getHttpHeaders();
    authHeaders['Content-Type'] = "application/json";
    final peersJsonData = peers.map((e) => e.toJson()).toList();
    final body = jsonEncode({
      "data": jsonEncode({"tags": tags, "peers": peersJsonData})
    });
    try {
      await http.post(Uri.parse(api), headers: authHeaders, body: body);
      await pullAb(quiet: true);
    } catch (e) {
      BotToast.showText(contentColor: Colors.red, text: e.toString());
    } finally {}
  }

  Peer? find(String id) {
    return peers.firstWhereOrNull((e) => e.id == id);
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

  Future<void> setPeerAlias(String id, String value) async {
    final it = peers.where((p0) => p0.id == id);
    if (it.isNotEmpty) {
      it.first.alias = value;
      await pushAb();
    }
  }

  Future<void> setPeerForceAlwaysRelay(String id, bool value) async {
    final it = peers.where((p0) => p0.id == id);
    if (it.isNotEmpty) {
      it.first.forceAlwaysRelay = value;
      await pushAb();
    }
  }

  Future<void> setRdp(String id, String port, String username) async {
    final it = peers.where((p0) => p0.id == id);
    if (it.isNotEmpty) {
      it.first.rdpPort = port;
      it.first.rdpUsername = username;
      await pushAb();
    }
  }
}

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

import '../common.dart';

class AbModel with ChangeNotifier {
  var abLoading = false;
  var abError = "";
  var tags = [].obs;
  var peers = List<Peer>.empty(growable: true).obs;

  var selectedTags = List<String>.empty(growable: true).obs;

  WeakReference<FFI> parent;

  AbModel(this.parent);

  FFI? get _ffi => parent.target;

  Future<dynamic> getAb() async {
    abLoading = true;
    notifyListeners();
    // request
    final api = "${await bind.mainGetApiServer()}/api/ab/get";
    try {
      final resp =
          await http.post(Uri.parse(api), headers: await getHttpHeaders());
      if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
        Map<String, dynamic> json = jsonDecode(resp.body);
        if (json.containsKey('error')) {
          abError = json['error'];
        } else if (json.containsKey('data')) {
          final data = jsonDecode(json['data']);
          tags.value = data['tags'];
          peers.clear();
          for (final peer in data['peers']) {
            peers.add(Peer.fromJson(peer));
          }
        }
        notifyListeners();
        return resp.body;
      } else {
        return "";
      }
    } catch (err) {
      err.printError();
      abError = err.toString();
    } finally {
      abLoading = false;
      notifyListeners();
    }
    return null;
  }

  void reset() {
    tags.clear();
    peers.clear();
    notifyListeners();
  }

  void addId(String id) async {
    if (idContainBy(id)) {
      return;
    }
    peers.add(Peer.fromJson({"id": id}));
    notifyListeners();
  }

  void addTag(String tag) async {
    if (tagContainBy(tag)) {
      return;
    }
    tags.add(tag);
    notifyListeners();
  }

  void changeTagForPeer(String id, List<dynamic> tags) {
    final it = peers.where((element) => element.id == id);
    if (it.isEmpty) {
      return;
    }
    it.first.tags = tags;
  }

  Future<void> updateAb() async {
    abLoading = true;
    notifyListeners();
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
      abError = "";
      await getAb();
      debugPrint("resp: ${resp.body}");
    } catch (e) {
      abError = e.toString();
    } finally {
      abLoading = false;
    }
    notifyListeners();
  }

  bool idContainBy(String id) {
    return peers.where((element) => element.id == id).isNotEmpty;
  }

  bool tagContainBy(String tag) {
    return tags.where((element) => element == tag).isNotEmpty;
  }

  void deletePeer(String id) {
    peers.removeWhere((element) => element.id == id);
    notifyListeners();
  }

  void deleteTag(String tag) {
    tags.removeWhere((element) => element == tag);
    for (var peer in peers) {
      if (peer.tags.isEmpty) {
        continue;
      }
      if (peer.tags.contains(tag)) {
        ((peer.tags)).remove(tag);
      }
    }
    notifyListeners();
  }

  void unsetSelectedTags() {
    selectedTags.clear();
    notifyListeners();
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
    notifyListeners();
  }
}

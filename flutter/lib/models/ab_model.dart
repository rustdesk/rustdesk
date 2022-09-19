import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:http/http.dart' as http;

class AbModel with ChangeNotifier {
  var abLoading = false;
  var abError = "";
  var tags = [].obs;
  var peers = [].obs;

  var selectedTags = List<String>.empty(growable: true).obs;

  WeakReference<FFI> parent;

  AbModel(this.parent);

  FFI? get _ffi => parent.target;

  Future<dynamic> getAb() async {
    abLoading = true;
    notifyListeners();
    // request
    final api = "${await getApiServer()}/api/ab/get";
    try {
      final resp =
          await http.post(Uri.parse(api), headers: await _getHeaders());
      if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
        Map<String, dynamic> json = jsonDecode(resp.body);
        if (json.containsKey('error')) {
          abError = json['error'];
        } else if (json.containsKey('data')) {
          final data = jsonDecode(json['data']);
          tags.value = data['tags'];
          peers.value = data['peers'];
        }
        notifyListeners();
        return resp.body;
      } else {
        return "";
      }
    } catch (err) {
      abError = err.toString();
    } finally {
      abLoading = false;
      notifyListeners();
    }
    return null;
  }

  Future<String> getApiServer() async {
    return await bind.mainGetApiServer();
  }

  void reset() {
    tags.clear();
    peers.clear();
    notifyListeners();
  }

  Future<Map<String, String>>? _getHeaders() {
    return _ffi?.getHttpHeaders();
  }

  void addId(String id) async {
    if (idContainBy(id)) {
      return;
    }
    peers.add({"id": id});
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
    final it = peers.where((element) => element['id'] == id);
    if (it.isEmpty) {
      return;
    }
    it.first['tags'] = tags;
  }

  Future<void> updateAb() async {
    abLoading = true;
    notifyListeners();
    final api = "${await getApiServer()}/api/ab";
    var authHeaders = await _getHeaders() ?? Map<String, String>();
    authHeaders['Content-Type'] = "application/json";
    final body = jsonEncode({
      "data": jsonEncode({"tags": tags, "peers": peers})
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
    return peers.where((element) => element['id'] == id).isNotEmpty;
  }

  bool tagContainBy(String tag) {
    return tags.where((element) => element == tag).isNotEmpty;
  }

  void deletePeer(String id) {
    peers.removeWhere((element) => element['id'] == id);
    notifyListeners();
  }

  void deleteTag(String tag) {
    tags.removeWhere((element) => element == tag);
    for (var peer in peers) {
      if (peer['tags'] == null) {
        continue;
      }
      if (((peer['tags']) as List<dynamic>).contains(tag)) {
        ((peer['tags']) as List<dynamic>).remove(tag);
      }
    }
    notifyListeners();
  }

  void unsetSelectedTags() {
    selectedTags.clear();
    notifyListeners();
  }

  List<dynamic> getPeerTags(String id) {
    final it = peers.where((p0) => p0['id'] == id);
    if (it.isEmpty) {
      return [];
    } else {
      return it.first['tags'] ?? [];
    }
  }

  void setPeerOption(String id, String key, String value) {
    final it = peers.where((p0) => p0['id'] == id);
    if (it.isEmpty) {
      debugPrint("${id} is not exists");
      return;
    } else {
      it.first[key] = value;
    }
  }

  void clear() {
    peers.clear();
    tags.clear();
    notifyListeners();
  }
}

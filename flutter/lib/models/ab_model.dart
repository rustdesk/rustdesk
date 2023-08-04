import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';
import 'package:http/http.dart' as http;

import '../common.dart';

final syncAbOption = 'sync-ab-with-recent-sessions';
bool shouldSyncAb() {
  return bind.mainGetLocalOption(key: syncAbOption).isNotEmpty;
}

final sortAbTagsOption = 'sync-ab-tags';
bool shouldSortTags() {
  return bind.mainGetLocalOption(key: sortAbTagsOption).isNotEmpty;
}

class AbModel {
  final abLoading = false.obs;
  final abError = "".obs;
  final tags = [].obs;
  final peers = List<Peer>.empty(growable: true).obs;
  final sortTags = shouldSortTags().obs;

  final selectedTags = List<String>.empty(growable: true).obs;
  var initialized = false;
  var licensedDevices = 0;
  var sync_all_from_recent = true;
  var _timerCounter = 0;

  WeakReference<FFI> parent;

  AbModel(this.parent) {
    if (desktopType == DesktopType.main) {
      Timer.periodic(Duration(milliseconds: 500), (timer) async {
        if (_timerCounter++ % 6 == 0) syncFromRecent();
      });
    }
  }

  Future<void> pullAb({force = true, quiet = false}) async {
    debugPrint("pullAb, force:$force, quite:$quiet");
    if (gFFI.userModel.userName.isEmpty) return;
    if (abLoading.value) return;
    if (!force && initialized) return;
    if (!quiet) {
      abLoading.value = true;
      abError.value = "";
    }
    final api = "${await bind.mainGetApiServer()}/api/ab";
    try {
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      authHeaders['Accept-Encoding'] = "gzip";
      final resp = await http.get(Uri.parse(api), headers: authHeaders);
      if (resp.body.isNotEmpty && resp.body.toLowerCase() != "null") {
        Map<String, dynamic> json = jsonDecode(utf8.decode(resp.bodyBytes));
        if (json.containsKey('error')) {
          abError.value = json['error'];
        } else if (json.containsKey('data')) {
          try {
            gFFI.abModel.licensedDevices = json['licensed_devices'];
            // ignore: empty_catches
          } catch (e) {}
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
      reset();
      abError.value = err.toString();
    } finally {
      abLoading.value = false;
      initialized = true;
      sync_all_from_recent = true;
      _timerCounter = 0;
      save();
    }
  }

  Future<void> reset() async {
    abError.value = '';
    await bind.mainSetLocalOption(key: "selected-tags", value: '');
    tags.clear();
    peers.clear();
    initialized = false;
    await bind.mainClearAb();
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

  bool isFull(bool warn) {
    final res = licensedDevices > 0 && peers.length >= licensedDevices;
    if (res && warn) {
      BotToast.showText(
          contentColor: Colors.red, text: translate("exceed_max_devices"));
    }
    return res;
  }

  void addPeer(Peer peer) {
    peers.removeWhere((e) => e.id == peer.id);
    peers.add(peer);
  }

  void addPeers(List<Peer> ps) {
    for (var p in ps) {
      addPeer(p);
    }
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

  void changeTagForPeers(List<String> ids, List<dynamic> tags) {
    peers.map((e) {
      if (ids.contains(e.id)) {
        e.tags = tags;
      }
    }).toList();
  }

  Future<void> pushAb() async {
    debugPrint("pushAb");
    final api = "${await bind.mainGetApiServer()}/api/ab";
    var authHeaders = getHttpHeaders();
    authHeaders['Content-Type'] = "application/json";
    final peersJsonData = peers.map((e) => e.toAbUploadJson()).toList();
    final body = jsonEncode({
      "data": jsonEncode({"tags": tags, "peers": peersJsonData})
    });
    var request = http.Request('POST', Uri.parse(api));
    // support compression
    if (licensedDevices > 0 && body.length > 1024) {
      authHeaders['Content-Encoding'] = "gzip";
      request.bodyBytes = GZipCodec().encode(utf8.encode(body));
    } else {
      request.body = body;
    }
    request.headers.addAll(authHeaders);
    try {
      await http.Client().send(request);
      // await pullAb(quiet: true);
    } catch (e) {
      BotToast.showText(contentColor: Colors.red, text: e.toString());
    } finally {
      sync_all_from_recent = true;
      _timerCounter = 0;
      save();
    }
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

  void deletePeers(List<String> ids) {
    peers.removeWhere((e) => ids.contains(e.id));
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

  void syncFromRecent() async {
    Peer merge(Peer r, Peer p) {
      return Peer(
          id: p.id,
          hash: r.hash.isEmpty ? p.hash : r.hash,
          username: r.username.isEmpty ? p.username : r.username,
          hostname: r.hostname.isEmpty ? p.hostname : r.hostname,
          platform: r.platform.isEmpty ? p.platform : r.platform,
          alias: r.alias,
          tags: p.tags,
          forceAlwaysRelay: r.forceAlwaysRelay,
          rdpPort: r.rdpPort,
          rdpUsername: r.rdpUsername);
    }

    bool shouldSync(Peer a, Peer b) {
      return a.hash != b.hash ||
          a.username != b.username ||
          a.platform != b.platform ||
          a.hostname != b.hostname;
    }

    Future<List<Peer>> getRecentPeers() async {
      try {
        if (peers.isEmpty) [];
        List<String> filteredPeerIDs;
        if (sync_all_from_recent) {
          sync_all_from_recent = false;
          filteredPeerIDs = peers.map((e) => e.id).toList();
        } else {
          final new_stored_str = await bind.mainGetNewStoredPeers();
          if (new_stored_str.isEmpty) return [];
          List<String> new_stores =
              (jsonDecode(new_stored_str) as List<dynamic>)
                  .map((e) => e.toString())
                  .toList();
          final abPeerIds = peers.map((e) => e.id).toList();
          filteredPeerIDs =
              new_stores.where((e) => abPeerIds.contains(e)).toList();
        }
        if (filteredPeerIDs.isEmpty) return [];
        final loadStr = await bind.mainLoadRecentPeersForAb(
            filter: jsonEncode(filteredPeerIDs));
        if (loadStr.isEmpty) {
          return [];
        }
        List<dynamic> mapPeers = jsonDecode(loadStr);
        List<Peer> recents = List.empty(growable: true);
        for (var m in mapPeers) {
          if (m is Map<String, dynamic>) {
            recents.add(Peer.fromJson(m));
          }
        }
        return recents;
      } catch (e) {
        debugPrint('getRecentPeers:$e');
      }
      return [];
    }

    try {
      if (!shouldSyncAb()) return;
      final oldPeers = peers.toList();
      final recents = await getRecentPeers();
      if (recents.isEmpty) return;
      for (var i = 0; i < peers.length; i++) {
        var p = peers[i];
        var r = recents.firstWhereOrNull((r) => p.id == r.id);
        if (r != null) {
          peers[i] = merge(r, p);
        }
      }
      bool changed = false;
      for (var i = 0; i < peers.length; i++) {
        final o = oldPeers[i];
        final p = peers[i];
        if (shouldSync(o, p)) {
          changed = true;
          break;
        }
      }
      // Be careful with loop calls
      if (changed) {
        pushAb();
      }
    } catch (e) {
      debugPrint('syncFromRecent:$e');
    }
  }

  save() {
    try {
      final infos = peers
          .map((e) => (<String, dynamic>{
                "id": e.id,
                "hash": e.hash,
              }))
          .toList();
      final m = <String, dynamic>{
        "access_token": bind.mainGetLocalOption(key: 'access_token'),
        "peers": infos,
      };
      bind.mainSaveAb(json: jsonEncode(m));
    } catch (e) {
      debugPrint('ab save:$e');
    }
  }
}

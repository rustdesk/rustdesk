import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
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
  final pullError = "".obs;
  final pushError = "".obs;
  final tags = [].obs;
  final peers = List<Peer>.empty(growable: true).obs;
  final sortTags = shouldSortTags().obs;
  bool get emtpy => peers.isEmpty && tags.isEmpty;

  final selectedTags = List<String>.empty(growable: true).obs;
  var initialized = false;
  var licensedDevices = 0;
  var _syncAllFromRecent = true;
  var _syncFromRecentLock = false;
  var _timerCounter = 0;
  var _cacheLoadOnceFlag = false;

  WeakReference<FFI> parent;

  AbModel(this.parent) {
    if (desktopType == DesktopType.main) {
      Timer.periodic(Duration(milliseconds: 500), (timer) async {
        if (_timerCounter++ % 6 == 0) syncFromRecent();
      });
    }
  }

  Future<void> pullAb({force = true, quiet = false}) async {
    debugPrint("pullAb, force:$force, quiet:$quiet");
    if (gFFI.userModel.userName.isEmpty) return;
    if (abLoading.value) return;
    if (!force && initialized) return;
    if (pushError.isNotEmpty) {
      try {
        // push to retry
        pushAb(toast: false);
      } catch (_) {}
    }
    if (!quiet) {
      abLoading.value = true;
      pullError.value = "";
    }
    final api = "${await bind.mainGetApiServer()}/api/ab";
    int? statusCode;
    try {
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      authHeaders['Accept-Encoding'] = "gzip";
      final resp = await http.get(Uri.parse(api), headers: authHeaders);
      statusCode = resp.statusCode;
      if (resp.body.toLowerCase() == "null") {
        // normal reply, emtpy ab return null
        tags.clear();
        peers.clear();
      } else if (resp.body.isNotEmpty) {
        Map<String, dynamic> json =
            _jsonDecode(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
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
            _saveCache(); // save on success
          }
        }
      }
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
        if (gFFI.peerTabModel.currentTab != PeerTabIndex.ab.index) {
          BotToast.showText(contentColor: Colors.red, text: pullError.value);
        }
      }
    } finally {
      if (initialized) {
        // make loading effect obvious
        Future.delayed(Duration(milliseconds: 300), () {
          abLoading.value = false;
        });
      } else {
        abLoading.value = false;
      }
      initialized = true;
      _syncAllFromRecent = true;
      _timerCounter = 0;
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(clearAbCache: true);
        }
      }
    }
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
    final index = peers.indexWhere((e) => e.id == peer.id);
    if (index >= 0) {
      peers[index] = merge(peer, peers[index]);
    } else {
      peers.add(peer);
    }
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

  void changeAlias({required String id, required String alias}) {
    final it = peers.where((element) => element.id == id);
    if (it.isEmpty) {
      return;
    }
    it.first.alias = alias;
  }

  Future<void> pushAb({bool toast = true}) async {
    debugPrint("pushAb");
    pushError.value = '';
    try {
      // avoid double pushes in a row
      _syncAllFromRecent = true;
      syncFromRecent(push: false);
      final api = "${await bind.mainGetApiServer()}/api/ab";
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      final peersJsonData = peers.map((e) => e.toAbUploadJson()).toList();
      final body = jsonEncode({
        "data": jsonEncode({"tags": tags, "peers": peersJsonData})
      });
      http.Response resp;
      // support compression
      if (licensedDevices > 0 && body.length > 1024) {
        authHeaders['Content-Encoding'] = "gzip";
        resp = await http.post(Uri.parse(api),
            headers: authHeaders, body: GZipCodec().encode(utf8.encode(body)));
      } else {
        resp =
            await http.post(Uri.parse(api), headers: authHeaders, body: body);
      }
      if (resp.statusCode == 200 &&
          (resp.body.isEmpty || resp.body.toLowerCase() == 'null')) {
        _saveCache();
      } else {
        Map<String, dynamic> json = _jsonDecode(resp.body, resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        } else if (resp.statusCode == 200) {
          _saveCache();
        } else {
          throw 'HTTP ${resp.statusCode}';
        }
      }
    } catch (e) {
      pushError.value =
          '${translate('push_ab_failed_tip')}: ${translate(e.toString())}';
      if (toast && gFFI.peerTabModel.currentTab != PeerTabIndex.ab.index) {
        BotToast.showText(contentColor: Colors.red, text: pushError.value);
      }
    } finally {
      _syncAllFromRecent = true;
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

  void renameTag(String oldTag, String newTag) {
    if (tags.contains(newTag)) return;
    tags.value = tags.map((e) {
      if (e == oldTag) {
        return newTag;
      } else {
        return e;
      }
    }).toList();
    selectedTags.value = selectedTags.map((e) {
      if (e == oldTag) {
        return newTag;
      } else {
        return e;
      }
    }).toList();
    for (var peer in peers) {
      peer.tags = peer.tags.map((e) {
        if (e == oldTag) {
          return newTag;
        } else {
          return e;
        }
      }).toList();
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

  Peer merge(Peer r, Peer p) {
    return Peer(
        id: p.id,
        hash: r.hash.isEmpty ? p.hash : r.hash,
        username: r.username.isEmpty ? p.username : r.username,
        hostname: r.hostname.isEmpty ? p.hostname : r.hostname,
        platform: r.platform.isEmpty ? p.platform : r.platform,
        alias: p.alias.isEmpty ? r.alias : p.alias,
        tags: p.tags,
        forceAlwaysRelay: r.forceAlwaysRelay,
        rdpPort: r.rdpPort,
        rdpUsername: r.rdpUsername);
  }

  void syncFromRecent({bool push = true}) async {
    if (!_syncFromRecentLock) {
      _syncFromRecentLock = true;
      _syncFromRecentWithoutLock(push: push);
      _syncFromRecentLock = false;
    }
  }

  void _syncFromRecentWithoutLock({bool push = true}) async {
    bool shouldSync(Peer a, Peer b) {
      return a.hash != b.hash ||
          a.username != b.username ||
          a.platform != b.platform ||
          a.hostname != b.hostname ||
          a.alias != b.alias;
    }

    Future<List<Peer>> getRecentPeers() async {
      try {
        if (peers.isEmpty) [];
        List<String> filteredPeerIDs;
        if (_syncAllFromRecent) {
          _syncAllFromRecent = false;
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
      if (changed && push) {
        pushAb();
      }
    } catch (e) {
      debugPrint('syncFromRecent:$e');
    }
  }

  _saveCache() {
    try {
      final peersJsonData = peers.map((e) => e.toAbUploadJson()).toList();
      final m = <String, dynamic>{
        "access_token": bind.mainGetLocalOption(key: 'access_token'),
        "peers": peersJsonData,
        "tags": tags.map((e) => e.toString()).toList(),
      };
      bind.mainSaveAb(json: jsonEncode(m));
    } catch (e) {
      debugPrint('ab save:$e');
    }
  }

  loadCache() async {
    try {
      if (_cacheLoadOnceFlag || abLoading.value) return;
      _cacheLoadOnceFlag = true;
      final access_token = bind.mainGetLocalOption(key: 'access_token');
      if (access_token.isEmpty) return;
      final cache = await bind.mainLoadAb();
      final data = jsonDecode(cache);
      if (data == null || data['access_token'] != access_token) return;
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
    } catch (e) {
      debugPrint("load ab cache: $e");
    }
  }

  Map<String, dynamic> _jsonDecode(String body, int statusCode) {
    try {
      Map<String, dynamic> json = jsonDecode(body);
      return json;
    } catch (e) {
      final err = body.isNotEmpty && body.length < 128 ? body : e.toString();
      if (statusCode != 200) {
        throw 'HTTP $statusCode, $err';
      }
      throw err;
    }
  }
}

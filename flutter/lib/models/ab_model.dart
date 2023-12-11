import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
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

final filterAbTagOption = 'filter-ab-by-intersection';
bool filterAbTagByIntersection() {
  return bind.mainGetLocalOption(key: filterAbTagOption).isNotEmpty;
}

class AbModel {
  final abLoading = false.obs;
  final pullError = "".obs;
  final pushError = "".obs;
  final tags = [].obs;
  final RxMap<String, int> tagColors = Map<String, int>.fromEntries([]).obs;
  final peers = List<Peer>.empty(growable: true).obs;
  final sortTags = shouldSortTags().obs;
  final filterByIntersection = filterAbTagByIntersection().obs;
  final retrying = false.obs;
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
        if (_timerCounter++ % 6 == 0) {
          if (!gFFI.userModel.isLogin) return;
          syncFromRecent();
        }
      });
    }
  }

  Future<void> pullAb({force = true, quiet = false}) async {
    debugPrint("pullAb, force:$force, quiet:$quiet");
    if (!gFFI.userModel.isLogin) return;
    if (abLoading.value) return;
    if (!force && initialized) return;
    if (pushError.isNotEmpty) {
      try {
        // push to retry
        await pushAb(toastIfFail: false, toastIfSucc: false);
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
        tagColors.clear();
        peers.clear();
      } else if (resp.body.isNotEmpty) {
        Map<String, dynamic> json =
            _jsonDecodeResp(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        } else if (json.containsKey('data')) {
          try {
            gFFI.abModel.licensedDevices = json['licensed_devices'];
            // ignore: empty_catches
          } catch (e) {}
          final data = jsonDecode(json['data']);
          if (data != null) {
            _deserialize(data);
            _saveCache(); // save on success
          }
        }
      }
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      abLoading.value = false;
      initialized = true;
      _syncAllFromRecent = true;
      _timerCounter = 0;
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
      platformFFI.tryHandle({'name': LoadEvent.addressBook});
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
    _mergePeerFromGroup(peer);
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
      merge(peer, peers[index]);
    } else {
      peers.add(peer);
    }
  }

  bool addPeers(List<Peer> ps) {
    bool allAdded = true;
    for (var p in ps) {
      if (!isFull(false)) {
        addPeer(p);
      } else {
        allAdded = false;
      }
    }
    return allAdded;
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

  bool changePassword(String id, String hash) {
    final it = peers.where((element) => element.id == id);
    if (it.isNotEmpty) {
      if (it.first.hash != hash) {
        it.first.hash = hash;
        return true;
      }
    }
    return false;
  }

  Future<bool> pushAb(
      {bool toastIfFail = true,
      bool toastIfSucc = true,
      bool isRetry = false}) async {
    debugPrint(
        "pushAb: toastIfFail:$toastIfFail, toastIfSucc:$toastIfSucc, isRetry:$isRetry");
    if (!gFFI.userModel.isLogin) return false;
    pushError.value = '';
    if (isRetry) retrying.value = true;
    DateTime startTime = DateTime.now();
    bool ret = false;
    try {
      // avoid double pushes in a row
      _syncAllFromRecent = true;
      await syncFromRecent(push: false);
      //https: //stackoverflow.com/questions/68249333/flutter-getx-updating-item-in-children-list-is-not-reactive
      peers.refresh();
      final api = "${await bind.mainGetApiServer()}/api/ab";
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      final body = jsonEncode({"data": jsonEncode(_serialize())});
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
        ret = true;
        _saveCache();
      } else {
        Map<String, dynamic> json =
            _jsonDecodeResp(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        } else if (resp.statusCode == 200) {
          ret = true;
          _saveCache();
        } else {
          throw 'HTTP ${resp.statusCode}';
        }
      }
    } catch (e) {
      pushError.value =
          '${translate('push_ab_failed_tip')}: ${translate(e.toString())}';
    }
    _syncAllFromRecent = true;
    if (isRetry) {
      var ms =
          (Duration(milliseconds: 200) - DateTime.now().difference(startTime))
              .inMilliseconds;
      ms = ms > 0 ? ms : 0;
      Future.delayed(Duration(milliseconds: ms), () {
        retrying.value = false;
      });
    }

    if (!ret && toastIfFail) {
      BotToast.showText(contentColor: Colors.red, text: pushError.value);
    }
    if (ret && toastIfSucc) {
      showToast(translate('Successful'));
    }
    return ret;
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
    tagColors.remove(tag);
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
    int? oldColor = tagColors[oldTag];
    if (oldColor != null) {
      tagColors.remove(oldTag);
      tagColors.addAll({newTag: oldColor});
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

  Color getTagColor(String tag) {
    int? colorValue = tagColors[tag];
    if (colorValue != null) {
      return Color(colorValue);
    }
    return str2color2(tag, existing: tagColors.values.toList());
  }

  setTagColor(String tag, Color color) {
    if (tags.contains(tag)) {
      tagColors[tag] = color.value;
    }
  }

  void merge(Peer r, Peer p) {
    p.hash = r.hash.isEmpty ? p.hash : r.hash;
    p.username = r.username.isEmpty ? p.username : r.username;
    p.hostname = r.hostname.isEmpty ? p.hostname : r.hostname;
    p.alias = p.alias.isEmpty ? r.alias : p.alias;
    p.forceAlwaysRelay = r.forceAlwaysRelay;
    p.rdpPort = r.rdpPort;
    p.rdpUsername = r.rdpUsername;
  }

  Future<void> syncFromRecent({bool push = true}) async {
    if (!_syncFromRecentLock) {
      _syncFromRecentLock = true;
      await _syncFromRecentWithoutLock(push: push);
      _syncFromRecentLock = false;
    }
  }

  Future<void> _syncFromRecentWithoutLock({bool push = true}) async {
    bool peerSyncEqual(Peer a, Peer b) {
      return a.hash == b.hash &&
          a.username == b.username &&
          a.platform == b.platform &&
          a.hostname == b.hostname &&
          a.alias == b.alias;
    }

    Future<List<Peer>> getRecentPeers() async {
      try {
        List<String> filteredPeerIDs;
        if (_syncAllFromRecent) {
          _syncAllFromRecent = false;
          filteredPeerIDs = [];
        } else {
          final new_stored_str = await bind.mainGetNewStoredPeers();
          if (new_stored_str.isEmpty) return [];
          filteredPeerIDs = (jsonDecode(new_stored_str) as List<dynamic>)
              .map((e) => e.toString())
              .toList();
          if (filteredPeerIDs.isEmpty) return [];
        }
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
      final recents = await getRecentPeers();
      if (recents.isEmpty) return;
      bool uiChanged = false;
      bool needSync = false;
      for (var i = 0; i < recents.length; i++) {
        var r = recents[i];
        var index = peers.indexWhere((e) => e.id == r.id);
        if (index < 0) {
          if (!isFull(false)) {
            peers.add(r);
            uiChanged = true;
            needSync = true;
          }
        } else {
          Peer old = Peer.copy(peers[index]);
          merge(r, peers[index]);
          if (!peerSyncEqual(peers[index], old)) {
            needSync = true;
          }
          if (!old.equal(peers[index])) {
            uiChanged = true;
          }
        }
      }
      // Be careful with loop calls
      if (needSync && push) {
        pushAb(toastIfSucc: false, toastIfFail: false);
      } else if (uiChanged) {
        peers.refresh();
      }
    } catch (e) {
      debugPrint('syncFromRecent:$e');
    }
  }

  _saveCache() {
    try {
      var m = _serialize();
      m.addAll(<String, dynamic>{
        "access_token": bind.mainGetLocalOption(key: 'access_token'),
      });
      bind.mainSaveAb(json: jsonEncode(m));
    } catch (e) {
      debugPrint('ab save:$e');
    }
  }

  Future<void> loadCache() async {
    try {
      if (_cacheLoadOnceFlag || abLoading.value || initialized) return;
      _cacheLoadOnceFlag = true;
      final access_token = bind.mainGetLocalOption(key: 'access_token');
      if (access_token.isEmpty) return;
      final cache = await bind.mainLoadAb();
      if (abLoading.value) return;
      final data = jsonDecode(cache);
      if (data == null || data['access_token'] != access_token) return;
      _deserialize(data);
    } catch (e) {
      debugPrint("load ab cache: $e");
    }
  }

  Map<String, dynamic> _jsonDecodeResp(String body, int statusCode) {
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

  Map<String, dynamic> _serialize() {
    final peersJsonData = peers.map((e) => e.toAbUploadJson()).toList();
    final tagColorJsonData = jsonEncode(tagColors);
    return {
      "tags": tags,
      "peers": peersJsonData,
      "tag_colors": tagColorJsonData
    };
  }

  _deserialize(dynamic data) {
    if (data == null) return;
    final oldOnlineIDs = peers.where((e) => e.online).map((e) => e.id).toList();
    tags.clear();
    tagColors.clear();
    peers.clear();
    if (data['tags'] is List) {
      tags.value = data['tags'];
    }
    if (data['peers'] is List) {
      for (final peer in data['peers']) {
        peers.add(Peer.fromJson(peer));
      }
    }
    if (isFull(false)) {
      peers.removeRange(licensedDevices, peers.length);
    }
    // restore online
    peers
        .where((e) => oldOnlineIDs.contains(e.id))
        .map((e) => e.online = true)
        .toList();
    if (data['tag_colors'] is String) {
      Map<String, dynamic> map = jsonDecode(data['tag_colors']);
      tagColors.value = Map<String, int>.from(map);
    }
    // add color to tag
    final tagsWithoutColor =
        tags.toList().where((e) => !tagColors.containsKey(e)).toList();
    for (var t in tagsWithoutColor) {
      tagColors[t] = str2color2(t, existing: tagColors.values.toList()).value;
    }
  }

  reSyncToast(Future<bool> future) {
    if (!shouldSyncAb()) return;
    Future.delayed(Duration.zero, () async {
      final succ = await future;
      if (succ) {
        await Future.delayed(Duration(seconds: 2)); // success msg
        BotToast.showText(
            contentColor: Colors.lightBlue,
            text: translate('synced_peer_readded_tip'));
      }
    });
  }

  reset() async {
    pullError.value = '';
    pushError.value = '';
    tags.clear();
    peers.clear();
    await bind.mainClearAb();
  }

  _mergePeerFromGroup(Peer p) {
    final g = gFFI.groupModel.peers.firstWhereOrNull((e) => p.id == e.id);
    if (g == null) return;
    if (p.username.isEmpty) {
      p.username = g.username;
    }
    if (p.hostname.isEmpty) {
      p.hostname = g.hostname;
    }
    if (p.platform.isEmpty) {
      p.platform = g.platform;
    }
  }
}

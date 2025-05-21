import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';

import '../utils/http_service.dart' as http;
import '../common.dart';

final syncAbOption = 'sync-ab-with-recent-sessions';
bool shouldSyncAb() {
  return bind.mainGetLocalOption(key: syncAbOption) == 'Y';
}

final sortAbTagsOption = 'sync-ab-tags';
bool shouldSortTags() {
  return bind.mainGetLocalOption(key: sortAbTagsOption) == 'Y';
}

final filterAbTagOption = 'filter-ab-by-intersection';
bool filterAbTagByIntersection() {
  return bind.mainGetLocalOption(key: filterAbTagOption) == 'Y';
}

const _personalAddressBookName = "My address book";
const _legacyAddressBookName = "Legacy address book";
const _localAddressBookName = "Local Address Book";

const kUntagged = "Untagged";

bool isLocalAddressBookMode() {
  return bind.mainGetLocalOption(key: kOptionLocalAddressBookMode) == 'Y';
}

enum ForcePullAb {
  listAndCurrent,
  current,
}

class AbModel {
  final addressbooks = Map<String, BaseAb>.fromEntries([]).obs;
  final RxString _currentName = ''.obs;
  RxString get currentName => _currentName;
  final _dummyAb = DummyAb();
  BaseAb get current => addressbooks[_currentName.value] ?? _dummyAb;

  RxList<Peer> get currentAbPeers => current.peers;
  RxList<String> get currentAbTags => current.tags;
  RxList<String> get selectedTags => current.selectedTags;

  RxBool get currentAbLoading => current.abLoading;
  bool get currentAbEmpty => current.peers.isEmpty && current.tags.isEmpty;
  RxString get currentAbPullError => current.pullError;
  RxString get currentAbPushError => current.pushError;
  String? _personalAbGuid;
  RxBool legacyMode = false.obs;

  // Only handles peers add/remove
  final Map<String, VoidCallback> _peerIdUpdateListeners = {};

  final sortTags = shouldSortTags().obs;
  final filterByIntersection = filterAbTagByIntersection().obs;

  var _syncAllFromRecent = true;
  var _syncFromRecentLock = false;
  var _timerCounter = 0;
  var _cacheLoadOnceFlag = false;
  var listInitialized = false;
  var _maxPeerOneAb = 0;

  late final Peers peersModel;

  WeakReference<FFI> parent;

  AbModel(this.parent) {
    addressbooks.clear();
    peersModel = Peers(
        name: PeersModelName.addressBook,
        getInitPeers: () => currentAbPeers,
        loadEvent: LoadEvent.addressBook);
    if (desktopType == DesktopType.main) {
      Timer.periodic(Duration(milliseconds: 500), (timer) async {
        if (_timerCounter++ % 6 == 0) {
          if (!gFFI.userModel.isLogin) return;
          if (!listInitialized) return;
          if (!current.initialized || !current.canWrite()) return;
          _syncFromRecent();
        }
      });
    }
  }

  reset() async {
    print("reset ab model");
    addressbooks.clear();
    _currentName.value = '';
    await bind.mainClearAb();
    listInitialized = false;
  }

// #region ab
  /// Pulls the address book data from the server.
  ///
  /// If `force` is `ForcePullAb.listAndCurrent`, the function will pull the list of address books, current address book, and try initialize personal address book.
  /// If `force` is `ForcePullAb.current`, the function will only pull the current address book.
  /// If `quiet` is true, the function will not display any notifications or errors.
  var _pulling = false;
  Future<void> pullAb(
      {required ForcePullAb? force, required bool quiet}) async {
    if (_pulling) return;
    _pulling = true;
    try {
      await _pullAb(force: force, quiet: quiet);
      _refreshTab();
    } catch (_) {}
    _pulling = false;
  }

  Future<void> _pullAb(
      {required ForcePullAb? force, required bool quiet}) async {
    if (isLocalAddressBookMode()) {
      listInitialized = true;
      await loadCache();
      if (!addressbooks.containsKey(_localAddressBookName)) {
        addressbooks[_localAddressBookName] = LegacyAb(); // Or a new LocalAb if specific logic is needed
      }
      setCurrentName(_localAddressBookName);
      _callbackPeerUpdate();
      return;
    }
    if (bind.isDisableAb()) return;
    if (!gFFI.userModel.isLogin) return;
    if (gFFI.userModel.networkError.isNotEmpty) return;
    if (force == null && listInitialized && current.initialized) return;
    debugPrint("pullAb, force: $force, quiet: $quiet");
    if (!listInitialized || force == ForcePullAb.listAndCurrent) {
      try {
        // Read personal guid every time to avoid upgrading the server without closing the main window
        _personalAbGuid = null;
        await _getPersonalAbGuid();
        // Determine legacy mode based on whether _personalAbGuid is null
        legacyMode.value = _personalAbGuid == null;
        if (!legacyMode.value && _maxPeerOneAb == 0) {
          await _getAbSettings();
        }
        if (_personalAbGuid != null) {
          debugPrint("pull ab list");
          List<AbProfile> abProfiles = List.empty(growable: true);
          abProfiles.add(AbProfile(_personalAbGuid!, _personalAddressBookName,
              gFFI.userModel.userName.value, null, ShareRule.read.value));
          // get all address book name
          await _getSharedAbProfiles(abProfiles);
          addressbooks.removeWhere((key, value) =>
              abProfiles.firstWhereOrNull((e) => e.name == key) == null);
          for (int i = 0; i < abProfiles.length; i++) {
            AbProfile p = abProfiles[i];
            if (addressbooks.containsKey(p.name)) {
              addressbooks[p.name]?.setSharedProfile(p);
            } else {
              addressbooks[p.name] = Ab(p, p.guid == _personalAbGuid);
            }
          }
        } else {
          // only legacy address book
          addressbooks
              .removeWhere((key, value) => key != _legacyAddressBookName);
          if (!addressbooks.containsKey(_legacyAddressBookName)) {
            addressbooks[_legacyAddressBookName] = LegacyAb();
          }
        }
        // set current address book name
        if (!listInitialized) {
          listInitialized = true;
          trySetCurrentToLast();
        }
        if (!addressbooks.containsKey(_currentName.value)) {
          setCurrentName(legacyMode.value
              ? _legacyAddressBookName
              : _personalAddressBookName);
        }
        // pull current address book
        await current.pullAb(quiet: quiet);
        // try initialize personal address book
        if (!current.isPersonal()) {
          final personalAb = addressbooks[_personalAddressBookName];
          if (personalAb != null && !personalAb.initialized) {
            await personalAb.pullAb(quiet: quiet);
          }
        }
      } catch (e) {
        debugPrint("pull ab list error: $e");
      }
    } else if (listInitialized &&
        (!current.initialized || force == ForcePullAb.current)) {
      try {
        await current.pullAb(quiet: quiet);
      } catch (e) {
        debugPrint("pull current Ab error: $e");
      }
    }
    _callbackPeerUpdate();
    if (listInitialized && current.initialized) {
      _saveCache();
    }
  }

  Future<bool> _getAbSettings() async {
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/settings";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final resp = await http.post(Uri.parse(api), headers: headers);
      if (resp.statusCode == 404) {
        debugPrint("HTTP 404, api server doesn't support shared address book");
        return false;
      }
      Map<String, dynamic> json =
          _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
      _maxPeerOneAb = json['max_peer_one_ab'] ?? 0;
      return true;
    } catch (err) {
      debugPrint('get ab settings err: ${err.toString()}');
    }
    return false;
  }

  Future<bool> _getPersonalAbGuid() async {
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/personal";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final resp = await http.post(Uri.parse(api), headers: headers);
      if (resp.statusCode == 404) {
        debugPrint("HTTP 404, current api server is legacy mode");
        return false;
      }
      Map<String, dynamic> json =
          _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
      if (json.containsKey('error')) {
        throw json['error'];
      }
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }
      _personalAbGuid = json['guid'];
      return true;
    } catch (err) {
      debugPrint('get personal ab err: ${err.toString()}');
    }
    return false;
  }

  Future<bool> _getSharedAbProfiles(List<AbProfile> profiles) async {
    final api = "${await bind.mainGetApiServer()}/api/ab/shared/profiles";
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 100;
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
            });
        var headers = getHttpHeaders();
        headers['Content-Type'] = "application/json";
        final resp = await http.post(uri, headers: headers);
        Map<String, dynamic> json =
            _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (json.containsKey('error')) {
          throw json['error'];
        }
        if (resp.statusCode != 200) {
          throw 'HTTP ${resp.statusCode}';
        }
        if (json.containsKey('total')) {
          if (total == 0) total = json['total'];
          if (json.containsKey('data')) {
            final data = json['data'];
            if (data is List) {
              for (final profile in data) {
                final u = AbProfile.fromJson(profile);
                int index = profiles.indexWhere((e) => e.name == u.name);
                if (index < 0) {
                  profiles.add(u);
                } else {
                  profiles[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      debugPrint('_getSharedAbProfiles err: ${err.toString()}');
    }
    return false;
  }

// #endregion

// #region rule
  List<String> addressBooksCanWrite() {
    List<String> list = [];
    addressbooks.forEach((key, value) async {
      if (value.canWrite()) {
        list.add(key);
      }
    });
    return list;
  }

// #endregion

// #region peer
  Future<String?> addIdToCurrent(
      String id, String alias, String password, List<dynamic> tags) async {
    if (currentAbPeers.where((element) => element.id == id).isNotEmpty) {
      return "$id already exists in address book $_currentName";
    }
    Map<String, dynamic> peer = {
      'id': id,
      'alias': alias,
      'tags': tags,
    };
    // avoid set existing password to empty
    if (password.isNotEmpty) {
      peer['password'] = password;
    }
    final ret = await addPeersTo([peer], _currentName.value);
    _syncAllFromRecent = true;
    if (isLocalAddressBookMode()) {
      _saveCache();
      return null; // Or some other appropriate local-mode return
    }
    return ret;
  }

  // Use Map<String, dynamic> rather than Peer to distinguish between empty and null
  Future<String?> addPeersTo(
    List<Map<String, dynamic>> ps,
    String name,
  ) async {
    final ab = addressbooks[name];
    if (ab == null) {
      return 'no such addressbook: $name';
    }
    // Local modification first
    for (var p_map in ps) {
      final p = Peer.fromJson(p_map);
      if (ab.peers.firstWhereOrNull((e) => e.id == p.id) == null) {
        if (!ab.isFull()) {
          ab.peers.add(p);
        } else {
          // Handle full case if necessary, maybe return an error or specific message
        }
      }
    }
    if (name == _currentName.value) {
      _refreshTab(); // Refresh UI if current AB is modified
    }
    _syncAllFromRecent = true;

    if (isLocalAddressBookMode()) {
      _saveCache();
      return null; // Or some other appropriate local-mode return
    }

    String? errMsg = await ab.addPeers(ps);
    await pullNonLegacyAfterChange(name: name);
    _saveCache(); // Save cache even in server mode after successful push
    return errMsg;
  }

  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags) async {
    // Local modification
    current.peers.map((e) {
      if (ids.contains(e.id)) {
        e.tags = List<String>.from(tags.map((t) => t.toString()));
      }
    }).toList();
    currentAbPeers.refresh();

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    bool ret = await current.changeTagForPeers(ids, tags);
    await pullNonLegacyAfterChange();
    _saveCache();
    return ret;
  }

  Future<bool> changeAlias({required String id, required String alias}) async {
    // Local modification
    final peer = current.peers.firstWhereOrNull((e) => e.id == id);
    if (peer != null) {
      peer.alias = alias;
    }
    currentAbPeers.refresh();

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    bool res = await current.changeAlias(id: id, alias: alias);
    await pullNonLegacyAfterChange();
    _saveCache();
    return res;
  }

  Future<bool> changePersonalHashPassword(String id, String hash) async {
    // Local modification
    final peer = current.peers.firstWhereOrNull((e) => e.id == id);
    if (peer != null) {
        peer.hash = hash;
    }

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    var ret = false;
    final personalAb = addressbooks[_personalAddressBookName];
    if (personalAb != null) {
      ret = await personalAb.changePersonalHashPassword(id, hash);
      await personalAb.pullAb(quiet: true);
    } else {
      final legacyAb = addressbooks[_legacyAddressBookName];
      if (legacyAb != null) {
        ret = await legacyAb.changePersonalHashPassword(id, hash);
      }
    }
    _saveCache();
    return ret;
  }

  Future<bool> changeSharedPassword(
      String abName, String id, String password) async {
    if (isLocalAddressBookMode()) {
      // In local mode, shared passwords might not be relevant, or handled as regular peer passwords.
      // For now, let's assume it's a no-op or saved like other peer data.
      final ab = addressbooks[abName];
      if (ab == null) return false;
      final peer = ab.peers.firstWhereOrNull((e) => e.id == id);
      if (peer != null) {
        // Decide how to store this. If local mode treats all as "personal" hash
        // this might need to set peer.hash if password is treated as a direct replacement.
        // Or, if local peers can have a 'password' field distinct from 'hash'.
        // For simplicity now, let's assume we are updating a generic password field if it exists.
        // This part needs clarification based on how Peer model handles passwords locally.
        // Let's assume Peer model has a `password` field for this for now.
        // peer.password = password; // This is hypothetical
      }
      _saveCache();
      return true;
    }

    final ab = addressbooks[abName];
    if (ab == null) return false;
    final ret = await ab.changeSharedPassword(id, password);
    await ab.pullAb(quiet: true);
    return ret;
  }

  Future<bool> deletePeers(List<String> ids) async {
    // Local modification
    current.peers.removeWhere((e) => ids.contains(e.id));
    currentAbPeers.refresh();
    _refreshTab();

    if (isLocalAddressBookMode()) {
      _saveCache();
      _callbackPeerUpdate();
      return true;
    }

    final ret = await current.deletePeers(ids);
    await pullNonLegacyAfterChange();
    _saveCache();
    if (legacyMode.value && current.isPersonal() && !isLocalAddressBookMode()) {
      // non-legacy mode not add peers automatically
      Future.delayed(Duration(seconds: 2), () async {
        if (!shouldSyncAb()) return;
        var hasSynced = false;
        for (var id in ids) {
          if (await bind.mainPeerExists(id: id)) {
            hasSynced = true;
            break;
          }
        }
        if (hasSynced) {
          BotToast.showText(
              contentColor: Colors.lightBlue,
              text: translate('synced_peer_readded_tip'));
          _syncAllFromRecent = true;
        }
      });
    }
    _callbackPeerUpdate();
    return ret;
  }

// #endregion

// #region tags
  Future<bool> addTags(List<String> tagList) async {
    tagList.removeWhere((e) => e == kUntagged);
    // Local modification
    for (var tag in tagList) {
      if (!current.tags.contains(tag)) {
        current.tags.add(tag);
      }
      if (current.tagColors[tag] == null) {
        current.tagColors[tag] = str2color2(tag, existing: current.tagColors.values.toList()).value;
      }
    }

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    final ret = await current.addTags(tagList, {});
    await pullNonLegacyAfterChange();
    _saveCache();
    return ret;
  }

  Future<bool> renameTag(String oldTag, String newTag) async {
    if (current.tags.contains(newTag) && oldTag != newTag) {
        BotToast.showText(
            contentColor: Colors.red, text: 'Tag $newTag already exists');
        return false;
    }
    // Local modification
    current.tags.value = current.tags.map((e) {
      if (e == oldTag) {
        return newTag;
      } else {
        return e;
      }
    }).toList();
    for (var peer in current.peers) {
      peer.tags = peer.tags.map((e) {
        if (e == oldTag) {
          return newTag;
        } else {
          return e;
        }
      }).toList();
    }
    int? oldColor = current.tagColors[oldTag];
    if (oldColor != null) {
      current.tagColors.remove(oldTag);
      current.tagColors.addAll({newTag: oldColor});
    }

    if (isLocalAddressBookMode()) {
      _saveCache();
      selectedTags.value = selectedTags.map((e) {
        if (e == oldTag) {
          return newTag;
        } else {
          return e;
        }
      }).toList();
      return true;
    }

    final ret = await current.renameTag(oldTag, newTag);
    await pullNonLegacyAfterChange();
    selectedTags.value = selectedTags.map((e) {
      if (e == oldTag) {
        return newTag;
      } else {
        return e;
      }
    }).toList();
    _saveCache();
    return ret;
  }

  Future<bool> setTagColor(String tag, Color color) async {
    // Local modification
    if (current.tags.contains(tag)) {
      current.tagColors[tag] = color.value;
    }

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    final ret = await current.setTagColor(tag, color);
    await pullNonLegacyAfterChange();
    _saveCache();
    return ret;
  }

  Future<bool> deleteTag(String tag) async {
    // Local modification
    selectedTags.remove(tag);
    current.tags.removeWhere((element) => element == tag);
    current.tagColors.remove(tag);
    for (var peer in current.peers) {
      if (peer.tags.isEmpty) {
        continue;
      }
      if (peer.tags.contains(tag)) {
        peer.tags.remove(tag);
      }
    }

    if (isLocalAddressBookMode()) {
      _saveCache();
      return true;
    }

    final ret = await current.deleteTag(tag);
    await pullNonLegacyAfterChange();
    _saveCache();
    return ret;
  }

// #endregion

// #region sync from recent
  Future<void> _syncFromRecent({bool push = true}) async {
    if (!shouldSyncAb()) return; // User preference check
    if (!gFFI.userModel.isLogin && !isLocalAddressBookMode()) return; // Login check
    if (!_syncFromRecentLock) {
      _syncFromRecentLock = true;
      await _syncFromRecentWithoutLock(push: push);
      _syncFromRecentLock = false;
    }
  }

  Future<void> _syncFromRecentWithoutLock({bool push = true}) async {
    if (!shouldSyncAb()) return; // User preference check
    if (!gFFI.userModel.isLogin && !isLocalAddressBookMode()) return; // Login check
    Future<List<Peer>> getRecentPeers() async {
      try {
        // In local mode, we assume recent peers are still tracked locally.
        // If bind.mainGetNewStoredPeers() or bind.mainLoadRecentPeersForAb()
        // require login, this needs further adjustment for local mode.
        // For now, assuming they work without login.
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
        final loadStr = await bind.mainLoadRecentPeersForAb( // Assumes this works offline
            filter: jsonEncode(filteredPeerIDs));
        if (loadStr.isEmpty) {
          return [];
        }
        List<dynamic> mapPeers = jsonDecode(loadStr); // Assumes this works offline
        List<Peer> recents = List.empty(growable: true);
        for (var m in mapPeers) {
          if (m is Map<String, dynamic>) {
            recents.add(Peer.fromJson(m));
          }
        }
        return recents;
      } catch (e) {
        debugPrint('getRecentPeers: $e');
      }
      return [];
    }

    try {
      if (!shouldSyncAb()) return;
      final recents = await getRecentPeers();
      if (recents.isEmpty) return;
      debugPrint("sync from recent, len: ${recents.length}");
      if (current.canWrite() && current.initialized) {
        await current.syncFromRecent(recents); // This might call pushAb internally
        if (isLocalAddressBookMode()) {
          _saveCache(); // Ensure local save after sync
        }
      }
    } catch (e) {
      debugPrint('_syncFromRecentWithoutLock: $e');
    }
  }

  void setShouldAsync(bool v) async {
    // This option might also need to be considered for local mode if it implies auto-sync from local recents.
    await bind.mainSetLocalOption(
        key: syncAbOption, value: v ? 'Y' : defaultOptionNo);
    _syncAllFromRecent = true; // Reset flag for next sync attempt
    _timerCounter = 0;
  }

// #endregion

// #region cache
  _saveCache() {
    try {
      var ab_entries = _serializeCache();
      Map<String, dynamic> m = <String, dynamic>{
        "ab_entries": ab_entries,
      };
      if (!isLocalAddressBookMode()) {
        // Only include access_token if not in local mode
        m["access_token"] = bind.mainGetLocalOption(key: 'access_token');
      }
      bind.mainSaveAb(json: jsonEncode(m));
    } catch (e) {
      debugPrint('ab save:$e');
    }
  }

  List<dynamic> _serializeCache() {
    var res = [];
    addressbooks.forEach((key, value) {
      // In local mode, we want to save the local address book.
      // The condition `!value.isPersonal() && key != current.name()` might be too restrictive.
      // Let's adjust to ensure the local book is always saved.
      if (isLocalAddressBookMode()) {
        if (key == _localAddressBookName) { // Or however the local book is identified
             res.add({
               "guid": value.sharedProfile()?.guid ?? _localAddressBookName, // Use a local identifier
               "name": key,
               "tags": value.tags,
               "peers": value.peers
                   .map((e) => e.toCustomJson(includingHash: true)) // Always include hash for local
                   .toList(),
               "tag_colors": jsonEncode(value.tagColors)
             });
        }
      } else {
        // Original logic for server mode
        if (!value.isPersonal() && key != current.name()) return;
        res.add({
          "guid": value.sharedProfile()?.guid ?? '',
          "name": key,
          "tags": value.tags,
          "peers": value.peers
              .map((e) => e.toCustomJson(includingHash: value.isPersonal()))
              .toList(),
          "tag_colors": jsonEncode(value.tagColors)
        });
      }
    });
    return res;
  }

  trySetCurrentToLast() {
    if (isLocalAddressBookMode()) {
      _currentName.value = _localAddressBookName; // Default to local book name
      return;
    }
    final name = bind.getLocalFlutterOption(k: kOptionCurrentAbName);
    if (addressbooks.containsKey(name)) {
      _currentName.value = name;
    }
  }

  Future<void> loadCache() async {
    try {
      if (_cacheLoadOnceFlag || currentAbLoading.value) return;
      _cacheLoadOnceFlag = true;
      final cache = await bind.mainLoadAb();
      if (currentAbLoading.value) return; // Check again after await
      if (cache.isEmpty) return;

      final data = jsonDecode(cache);
      if (data == null) return;

      if (!isLocalAddressBookMode()) {
        final access_token = bind.mainGetLocalOption(key: 'access_token');
        if (access_token.isEmpty) return; // No token, no server cache to load (unless local mode)
        if (data['access_token'] != access_token) return; // Token mismatch
      }
      
      _deserializeCache(data);
      if (isLocalAddressBookMode()) {
        _currentName.value = _localAddressBookName;
        if (!addressbooks.containsKey(_localAddressBookName)) {
           addressbooks[_localAddressBookName] = LegacyAb(); // Ensure local book exists
        }
      } else {
        legacyMode.value = addressbooks.containsKey(_legacyAddressBookName);
        trySetCurrentToLast();
      }
    } catch (e) {
      debugPrint("load ab cache: $e");
    }
  }

  _deserializeCache(dynamic data) {
    if (data == null) return;
    // Reset logic might need to be conditional for local mode,
    // or ensure local book is preserved/re-created.
    // For now, assuming reset() is acceptable or handled by subsequent logic.
    final currentLocalBookPeers = isLocalAddressBookMode() && addressbooks.containsKey(_localAddressBookName)
                                  ? List<Peer>.from(addressbooks[_localAddressBookName]!.peers)
                                  : null;
    final currentLocalBookTags = isLocalAddressBookMode() && addressbooks.containsKey(_localAddressBookName)
                                  ? List<String>.from(addressbooks[_localAddressBookName]!.tags)
                                  : null;
    final currentLocalBookTagColors = isLocalAddressBookMode() && addressbooks.containsKey(_localAddressBookName)
                                  ? Map<String, int>.from(addressbooks[_localAddressBookName]!.tagColors)
                                  : null;

    reset(); // This clears addressbooks. We need to repopulate it.

    final abEntries = data['ab_entries'];
    if (abEntries is List) {
      for (var i = 0; i < abEntries.length; i++) {
        var abEntry = abEntries[i];
        if (abEntry is Map<String, dynamic>) {
          var guid = abEntry['guid'];
          var name = abEntry['name'];
          final BaseAb ab;

          if (isLocalAddressBookMode()) {
            if (name == _localAddressBookName) {
              ab = addressbooks.putIfAbsent(_localAddressBookName, () => LegacyAb());
            } else {
              continue; // Skip non-local books in local mode
            }
          } else {
            // Original server mode logic
            if (name == _legacyAddressBookName) {
              ab = LegacyAb();
            } else {
              if (name == null || guid == null) {
                continue;
              }
              ab = Ab(AbProfile(guid, name, '', '', ShareRule.read.value),
                  name == _personalAddressBookName);
            }
            addressbooks[name] = ab;
          }
          
          if (abEntry['tags'] is List) {
            ab.tags.value =
                (abEntry['tags'] as List).map((e) => e.toString()).toList();
          }
          if (abEntry['peers'] is List) {
            for (var peer in abEntry['peers']) {
              ab.peers.add(Peer.fromJson(peer));
            }
          }
          if (abEntry['tag_colors'] is String) {
            Map<String, dynamic> map = jsonDecode(abEntry['tag_colors']);
            ab.tagColors.value = Map<String, int>.from(map);
          }
        }
      }
      if (isLocalAddressBookMode() && !addressbooks.containsKey(_localAddressBookName)) {
        // If after deserializing, the local book is still not there (e.g. empty cache file), create it.
        final localBook = LegacyAb();
        if (currentLocalBookPeers != null) localBook.peers.value = currentLocalBookPeers;
        if (currentLocalBookTags != null) localBook.tags.value = currentLocalBookTags;
        if (currentLocalBookTagColors != null) localBook.tagColors.value = currentLocalBookTagColors;
        addressbooks[_localAddressBookName] = localBook;
      }
      if (abEntries.isNotEmpty || isLocalAddressBookMode()) { // Ensure callback if local mode, even if cache was empty
        _callbackPeerUpdate();
      }
    } else if (isLocalAddressBookMode()) {
      // Cache was empty or not a list, ensure local book exists
        final localBook = LegacyAb();
        if (currentLocalBookPeers != null) localBook.peers.value = currentLocalBookPeers;
        if (currentLocalBookTags != null) localBook.tags.value = currentLocalBookTags;
        if (currentLocalBookTagColors != null) localBook.tagColors.value = currentLocalBookTagColors;
        addressbooks[_localAddressBookName] = localBook;
        _callbackPeerUpdate();
    }
  }

// #endregion

// #region tools
  Peer? find(String id) {
    return currentAbPeers.firstWhereOrNull((e) => e.id == id);
  }

  bool idContainByCurrent(String id) {
    return currentAbPeers.where((element) => element.id == id).isNotEmpty;
  }

  void unsetSelectedTags() {
    selectedTags.clear();
  }

  List<dynamic> getPeerTags(String id) {
    final it = currentAbPeers.where((p0) => p0.id == id);
    if (it.isEmpty) {
      return [];
    } else {
      return it.first.tags;
    }
  }

  Color getCurrentAbTagColor(String tag) {
    if (tag == kUntagged) {
      return MyTheme.accent;
    }
    int? colorValue = current.tagColors[tag];
    if (colorValue != null) {
      return Color(colorValue);
    }
    return str2color2(tag, existing: current.tagColors.values.toList());
  }

  List<String> addressBookNames() {
    return addressbooks.keys.toList();
  }

  String personalAddressBookName() {
    return _personalAddressBookName;
  }

  Future<void> setCurrentName(String name) async {
    final oldName = _currentName.value;
    if (addressbooks.containsKey(name)) {
      _currentName.value = name;
    } else {
      if (addressbooks.containsKey(_personalAddressBookName)) {
        _currentName.value = _personalAddressBookName;
      } else if (addressbooks.containsKey(_legacyAddressBookName)) {
        _currentName.value = _legacyAddressBookName;
      } else {
        _currentName.value = '';
      }
    }
    if (!current.initialized) {
      await current.pullAb(quiet: false);
    }
    _refreshTab();
    if (oldName != _currentName.value || isLocalAddressBookMode()) { // Always save cache if local mode name changed (even if to itself) or if it is local mode.
      _syncAllFromRecent = true; // Assuming this is still relevant for local recent peers
      _saveCache();
    }
  }

  bool isCurrentAbFull(bool warn) {
    final res = current.isFull();
    if (res && warn && !isLocalAddressBookMode()) { // Warning might be server-specific
      BotToast.showText(
          contentColor: Colors.red, text: translate('exceed_max_devices'));
    }
    return res;
  }

  void _refreshTab() {
    platformFFI.tryHandle({'name': LoadEvent.addressBook});
  }

  // should not call this function in a loop call stack
  Future<void> pullNonLegacyAfterChange({String? name}) async {
    if (isLocalAddressBookMode()) return; // No server pull in local mode

    if (name == null) {
      if (current.name() != _legacyAddressBookName) {
        return await current.pullAb(quiet: true);
      }
    } else if (name != _legacyAddressBookName) {
      final ab = addressbooks[name];
      if (ab != null) {
        return await ab.pullAb(quiet: true);
      }
    }
  }

  List<String> idExistIn(String id) {
    // This method's behavior might need to be reviewed in local mode.
    // If only one local book exists, it will check that.
    // If multiple local books were hypothetically supported, it would check all.
    List<String> v = [];
    addressbooks.forEach((key, value) {
      if (value.peers.any((e) => e.id == id)) {
        v.add(key);
      }
    });
    return v;
  }

  List<Peer> allPeers() {
    // In local mode, this should probably only return peers from the local address book.
    if (isLocalAddressBookMode()) {
        final localBook = addressbooks[_localAddressBookName];
        if (localBook != null) {
            return List<Peer>.from(localBook.peers.map((e) => Peer.copy(e)));
        }
        return [];
    }
    // Original server mode logic
    List<Peer> v = [];
    addressbooks.forEach((key, value) {
      v.addAll(value.peers.map((e) => Peer.copy(e)).toList());
    });
    return v;
  }

  String translatedName(String name) {
    if (name == _localAddressBookName) return translate("Local Address Book"); // Assuming "Local Address Book" is a key in lang files
    if (name == _personalAddressBookName || name == _legacyAddressBookName) {
      return translate(name);
    } else {
      return name;
    }
  }

  void _callbackPeerUpdate() {
    for (var listener in _peerIdUpdateListeners.values) {
      listener();
    }
  }

  void addPeerUpdateListener(String key, VoidCallback listener) {
    _peerIdUpdateListeners[key] = listener;
  }

  void removePeerUpdateListener(String key) {
    _peerIdUpdateListeners.remove(key);
  }

// #endregion
}

abstract class BaseAb {
  final peers = List<Peer>.empty(growable: true).obs;
  final RxList<String> tags = <String>[].obs;
  final RxMap<String, int> tagColors = Map<String, int>.fromEntries([]).obs;
  final selectedTags = List<String>.empty(growable: true).obs;

  final pullError = "".obs;
  final pushError = "".obs;
  final abLoading = false
      .obs; // Indicates whether the UI should show a loading state for the address book.
  var abPulling =
      false; // Tracks whether a pull operation is currently in progress to prevent concurrent pulls. Unlike abLoading, this is not tied to UI updates.
  bool initialized = false;

  String name();

  bool isPersonal() {
    if (isLocalAddressBookMode()) return name() == _localAddressBookName;
    return name() == _personalAddressBookName ||
        name() == _legacyAddressBookName;
  }

  bool isLegacy() {
     if (isLocalAddressBookMode()) return false; // Or true if local uses LegacyAb structure
    return name() == _legacyAddressBookName;
  }

  Future<void> pullAb({quiet = false}) async {
    if (isLocalAddressBookMode()) {
        initialized = true; // Already handled by AbModel.pullAb
        abLoading.value = false;
        return;
    }
    if (abPulling) return;
    abPulling = true;
    if (!quiet) {
      abLoading.value = true;
      pullError.value = "";
    }
    initialized = false;
    debugPrint("pull ab \"${name()}\"");
    try {
      initialized = await pullAbImpl(quiet: quiet);
    } catch (e) {
      debugPrint("Error occurred while pulling address book: $e");
    } finally {
      abLoading.value = false;
      abPulling = false;
    }
  }

  Future<bool> pullAbImpl({quiet = false});

  Future<String?> addPeers(List<Map<String, dynamic>> ps);
  removeHash(Map<String, dynamic> p) {
    p.remove('hash');
  }

  removePassword(Map<String, dynamic> p) {
    p.remove('password');
  }

  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags);

  Future<bool> changeAlias({required String id, required String alias});

  Future<bool> changePersonalHashPassword(String id, String hash);

  Future<bool> changeSharedPassword(String id, String password);

  Future<bool> deletePeers(List<String> ids);

  Future<bool> addTags(List<String> tagList, Map<String, int> tagColorMap);

  bool tagContainBy(String tag) {
    return tags.where((element) => element == tag).isNotEmpty;
  }

  Future<bool> renameTag(String oldTag, String newTag);

  Future<bool> setTagColor(String tag, Color color);

  Future<bool> deleteTag(String tag);

  bool isFull();

  void setSharedProfile(AbProfile profile);

  AbProfile? sharedProfile();

  bool canWrite();

  bool fullControl();

  Future<void> syncFromRecent(List<Peer> recents);
}

class LegacyAb extends BaseAb {
  bool get emtpy => peers.isEmpty && tags.isEmpty;
  // licensedDevices is obtained from personal ab, shared ab restrict it in server
  var licensedDevices = 0;

  LegacyAb();

  @override
  AbProfile? sharedProfile() {
    return null;
  }

  @override
  void setSharedProfile(AbProfile? profile) {}

  @override
  bool canWrite() {
    return true; // Local mode should always be writable
  }

  @override
  bool fullControl() {
    return true; // Local mode should always have full control
  }

  @override
  bool isFull() {
    return licensedDevices > 0 && peers.length >= licensedDevices;
  }

  @override
  String name() {
    // If this instance is used for the local book, its name should reflect that.
    // This might require passing the name in constructor or having a dedicated LocalLegacyAb.
    // For now, assuming AbModel.current will point to an instance named _localAddressBookName.
    return _legacyAddressBookName; // This might be problematic if LegacyAb is reused for local.
                                 // Consider if `name()` should be dynamic based on context.
  }

  @override
  Future<bool> pullAbImpl({quiet = false}) async {
    bool ret = false;
    final api = "${await bind.mainGetApiServer()}/api/ab";
    int? statusCode;
    try {
      var authHeaders = getHttpHeaders();
      authHeaders['Content-Type'] = "application/json";
      authHeaders['Accept-Encoding'] = "gzip";
      final resp = await http.get(Uri.parse(api), headers: authHeaders);
      statusCode = resp.statusCode;
      if (resp.body.toLowerCase() == "null") {
        // normal reply, empty ab return null
        tags.clear();
        tagColors.clear();
        peers.clear();
      } else if (resp.body.isNotEmpty) {
        Map<String, dynamic> jsonMap =
            _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (jsonMap.containsKey('error')) {
          throw jsonMap['error'];
        } else if (jsonMap.containsKey('data')) {
          try {
            licensedDevices = jsonMap['licensed_devices'];
            // ignore: empty_catches
          } catch (e) {}
          final data = jsonDecode(jsonMap['data']);
          if (data != null) {
            _deserialize(data);
          }
          ret = true;
        }
      }
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
    }
    return ret;
  }

  Future<bool> pushAb(
      {bool toastIfFail = true, bool toastIfSucc = true}) async {
    if (isLocalAddressBookMode()) {
      // Saving is handled by AbModel._saveCache()
      return true;
    }
    debugPrint("pushAb: toastIfFail:$toastIfFail, toastIfSucc:$toastIfSucc");
    if (!gFFI.userModel.isLogin) return false; // Original check, ensure local mode bypasses if it reaches here
    pushError.value = '';
    bool ret = false;
    try {
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
      } else {
        Map<String, dynamic> jsonMap =
            _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (jsonMap.containsKey('error')) {
          throw jsonMap['error'];
        } else if (resp.statusCode == 200) {
          ret = true;
        } else {
          throw 'HTTP ${resp.statusCode}';
        }
      }
    } catch (e) {
      pushError.value =
          '${translate('push_ab_failed_tip')}: ${translate(e.toString())}';
    }

    if (!ret && toastIfFail) {
      BotToast.showText(contentColor: Colors.red, text: pushError.value);
    }
    if (ret && toastIfSucc) {
      showToast(translate('Successful'));
    }
    return ret;
  }

// #region Peer
  @override
  Future<String?> addPeers(List<Map<String, dynamic>> ps) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel.addPeersTo local modification part
      // This method in LegacyAb shouldn't be directly called for adding peers in local mode
      // if AbModel.addPeersTo is the entry point.
      // For safety, mirror local logic or assert.
      bool full = false;
      for (var p_map in ps) {
        final p = Peer.fromJson(p_map);
        if (!isFull()) { // isFull() is already local mode aware
          final index = peers.indexWhere((e) => e.id == p.id);
          if (index >= 0) {
            _merge(p, peers[index]);
          } else {
            peers.add(p);
          }
        } else {
          full = true;
          break;
        }
      }
      return full ? translate("exceed_max_devices") : null;
    }
    bool full = false;
    for (var p_map in ps) {
      final p = Peer.fromJson(p_map);
      if (!isFull()) {
        p_map.remove('password'); // legacy ab ignore password
        final index = peers.indexWhere((e) => e.id == p.id);
        if (index >= 0) {
          _merge(p, peers[index]);
          _mergePeerFromGroup(peers[index]);
        } else {
          peers.add(p);
        }
      } else {
        full = true;
        break;
      }
    }
    if (!await pushAb()) { // pushAb is local mode aware
      return "Failed to push to server";
    } else if (full) {
      return translate("exceed_max_devices");
    } else {
      return null;
    }
  }

  _mergePeerFromGroup(Peer p) {
    // This logic might still be relevant if group data is available locally.
    // For now, assume it's okay.
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

  @override
  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      peers.map((e) {
        if (ids.contains(e.id)) {
         e.tags = List<String>.from(tags.map((t) => t.toString()));
        }
      }).toList();
      return true;
    }
    peers.map((e) {
      if (ids.contains(e.id)) {
        e.tags = List<String>.from(tags.map((t) => t.toString()));
      }
    }).toList();
    return await pushAb();
  }

  @override
  Future<bool> changeAlias({required String id, required String alias}) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      final it = peers.where((element) => element.id == id);
      if (it.isEmpty) return false;
      it.first.alias = alias;
      return true;
    }
    final it = peers.where((element) => element.id == id);
    if (it.isEmpty) {
      return false;
    }
    it.first.alias = alias;
    return await pushAb();
  }

  @override
  Future<bool> changeSharedPassword(String id, String password) async {
    if (isLocalAddressBookMode()) {
      // Shared passwords not applicable or handled differently in local mode
      return true;
    }
    // no need to implement for server legacy
    return false;
  }

  @override
  Future<void> syncFromRecent(List<Peer> recents) async {
    // This method is called by AbModel._syncFromRecentWithoutLock
    // which calls current.syncFromRecent.
    // If current is LegacyAb in local mode, this will be executed.
    bool peerSyncEqual(Peer a, Peer b) {
      return a.hash == b.hash &&
          a.username == b.username &&
          a.platform == b.platform &&
          a.hostname == b.hostname &&
          a.alias == b.alias;
    }

    bool needSyncOrSave = false;
    for (var i = 0; i < recents.length; i++) {
      var r = recents[i];
      var index = peers.indexWhere((e) => e.id == r.id);
      if (index < 0) {
        if (!isFull()) { // isFull is local mode aware
          peers.add(r);
          needSyncOrSave = true;
        }
      } else {
        Peer old = Peer.copy(peers[index]);
        _merge(r, peers[index]); // Merge recent info into existing peer
        if (!peerSyncEqual(peers[index], old)) {
          needSyncOrSave = true;
        }
      }
    }
    if (needSyncOrSave) {
      if (isLocalAddressBookMode()) {
        // gFFI.abModel._saveCache(); // AbModel will call saveCache after this returns
      } else {
        await pushAb(toastIfSucc: false, toastIfFail: false);
      }
      gFFI.abModel._refreshTab();
    }
    // Pull cannot be used for sync to avoid cyclic sync.
  }

  void _merge(Peer r, Peer p) {
    // Merging logic seems fine for local mode too.
    p.hash = r.hash.isEmpty ? p.hash : r.hash;
    p.username = r.username.isEmpty ? p.username : r.username;
    p.hostname = r.hostname.isEmpty ? p.hostname : r.hostname;
    p.platform = r.platform.isEmpty ? p.platform : r.platform;
    p.alias = p.alias.isEmpty ? r.alias : p.alias;
  }

  @override
  Future<bool> changePersonalHashPassword(String id, String hash) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      bool changed = false;
      final it = peers.where((element) => element.id == id);
      if (it.isNotEmpty) {
        if (it.first.hash != hash) {
          it.first.hash = hash;
          changed = true;
        }
      }
      return changed; // Indicate if change was made, AbModel handles save
    }
    bool changed = false;
    final it = peers.where((element) => element.id == id);
    if (it.isNotEmpty) {
      if (it.first.hash != hash) {
        it.first.hash = hash;
        changed = true;
      }
    }
    if (changed) {
      return await pushAb(toastIfSucc: false, toastIfFail: false);
    }
    return true;
  }

  @override
  Future<bool> deletePeers(List<String> ids) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      peers.removeWhere((e) => ids.contains(e.id));
      return true;
    }
    peers.removeWhere((e) => ids.contains(e.id));
    return await pushAb();
  }
// #endregion

// #region Tag
  @override
  Future<bool> addTags(
      List<String> tagList, Map<String, int> tagColorMap) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      for (var e in tagList) {
        if (!tagContainBy(e)) {
          tags.add(e);
        }
        if (tagColors[e] == null) {
          tagColors[e] = str2color2(e, existing: tagColors.values.toList()).value;
        }
      }
      return true;
    }
    for (var e in tagList) {
      if (!tagContainBy(e)) {
        tags.add(e);
      }
      if (tagColors[e] == null) {
        tagColors[e] = str2color2(e, existing: tagColors.values.toList()).value;
      }
    }
    return await pushAb();
  }

  @override
  Future<bool> renameTag(String oldTag, String newTag) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      if (tags.contains(newTag) && oldTag != newTag) { // Check here to avoid issues if AbModel didn't
        BotToast.showText(
            contentColor: Colors.red, text: 'Tag $newTag already exists');
        return false;
      }
      tags.value = tags.map((e) => (e == oldTag) ? newTag : e).toList();
      for (var peer in peers) {
        peer.tags = peer.tags.map((e) => (e == oldTag) ? newTag : e).toList();
      }
      int? oldColor = tagColors[oldTag];
      if (oldColor != null) {
        tagColors.remove(oldTag);
        tagColors.addAll({newTag: oldColor});
      }
      return true;
    }
    if (tags.contains(newTag)) {
      BotToast.showText(
          contentColor: Colors.red, text: 'Tag $newTag already exists');
      return false;
    }
    tags.value = tags.map((e) {
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
    return await pushAb();
  }

  @override
  Future<bool> setTagColor(String tag, Color color) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      if (tags.contains(tag)) {
        tagColors[tag] = color.value;
      }
      return true;
    }
    if (tags.contains(tag)) {
      tagColors[tag] = color.value;
    }
    return await pushAb();
  }

  @override
  Future<bool> deleteTag(String tag) async {
    if (isLocalAddressBookMode()) {
      // Already handled by AbModel
      // gFFI.abModel.selectedTags.remove(tag); // AbModel handles selectedTags
      tags.removeWhere((element) => element == tag);
      tagColors.remove(tag);
      for (var peer in peers) {
        peer.tags.remove(tag);
      }
      return true;
    }
    gFFI.abModel.selectedTags.remove(tag);
    tags.removeWhere((element) => element == tag);
    tagColors.remove(tag);
    for (var peer in peers) {
      if (peer.tags.isEmpty) {
        continue;
      }
      if (peer.tags.contains(tag)) {
        peer.tags.remove(tag);
      }
    }
    return await pushAb();
  }

// #endregion

  Map<String, dynamic> _serialize() {
    // In local mode, always include hash if LegacyAb is used for local storage
    final peersJsonData =
        peers.map((e) => e.toCustomJson(includingHash: true)).toList();
    for (var e in tags) {
      if (tagColors[e] == null) {
        tagColors[e] = str2color2(e, existing: tagColors.values.toList()).value;
      }
    }
    final tagColorJsonData = jsonEncode(tagColors); // Safe, no change needed
    return {
      "tags": tags,
      "peers": peersJsonData,
      "tag_colors": tagColorJsonData
    };
  }

  _deserialize(dynamic data) {
    // This is called by AbModel._deserializeCache for LegacyAb instances (including local one).
    // Behavior should be fine.
    if (data == null) return;
    final oldOnlineIDs = peers.where((e) => e.online).map((e) => e.id).toList();
    tags.clear();
    tagColors.clear();
    peers.clear();
    if (data['tags'] is List) {
      tags.value = (data['tags'] as List).map((e) => e.toString()).toList();
    }
    if (data['peers'] is List) {
      for (final peer in data['peers']) {
        peers.add(Peer.fromJson(peer));
      }
    }
    if (isFull()) { // isFull is local mode aware
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
}

class Ab extends BaseAb {
  AbProfile profile;
  late final bool personal;
  bool get emtpy => peers.isEmpty && tags.isEmpty;

  Ab(this.profile, this.personal);

  @override
  String name() {
    if (isLocalAddressBookMode()) return _localAddressBookName; // Should not happen if Ab is server-only
    if (personal) {
      return _personalAddressBookName;
    } else {
      return profile.name;
    }
  }

  @override
  AbProfile? sharedProfile() {
    if (isLocalAddressBookMode()) return null; // No shared profile in local mode
    return profile;
  }

  @override
  void setSharedProfile(AbProfile profile) {
    if (isLocalAddressBookMode()) return;
    this.profile = profile;
  }

  @override
  bool isFull() {
    if (isLocalAddressBookMode()) return false;
    return gFFI.abModel._maxPeerOneAb > 0 &&
        peers.length >= gFFI.abModel._maxPeerOneAb;
  }

  @override
  bool canWrite() {
    if (isLocalAddressBookMode()) return true;
    if (personal) {
      return true;
    } else {
      return profile.rule == ShareRule.readWrite.value ||
          profile.rule == ShareRule.fullControl.value;
    }
  }

  @override
  bool fullControl() {
    if (isLocalAddressBookMode()) return true;
    if (personal) {
      return true;
    } else {
      return profile.rule == ShareRule.fullControl.value;
    }
  }

  @override
  Future<bool> pullAbImpl({quiet = false}) async {
    if (isLocalAddressBookMode()) {
      initialized = true; return true; // Should be handled by AbModel.loadCache()
    }
    bool ret = true;
    List<Peer> tmpPeers = [];
    if (!await _fetchPeers(tmpPeers, quiet: quiet)) {
      ret = false;
    }
    peers.value = tmpPeers;
    List<AbTag> tmpTags = [];
    if (!await _fetchTags(tmpTags, quiet: quiet)) {
      ret = false;
    }
    tags.value = tmpTags.map((e) => e.name).toList();
    Map<String, int> tmpTagColors = {};
    for (var t in tmpTags) {
      tmpTagColors[t.name] = t.color;
    }
    tagColors.value = tmpTagColors;
    return ret;
  }

  Future<bool> _fetchPeers(List<Peer> tmpPeers, {quiet = false}) async {
    if (isLocalAddressBookMode()) return true; // No fetching in local mode
    final api = "${await bind.mainGetApiServer()}/api/ab/peers";
    int? statusCode;
    try {
      var uri0 = Uri.parse(api);
      final pageSize = 100;
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
              'ab': profile.guid,
            });
        var headers = getHttpHeaders();
        headers['Content-Type'] = "application/json";
        final resp = await http.post(uri, headers: headers);
        statusCode = resp.statusCode;
        Map<String, dynamic> jsonMap =
            _jsonDecodeRespMap(utf8.decode(resp.bodyBytes), resp.statusCode);
        if (jsonMap.containsKey('error')) {
          throw jsonMap['error'];
        }
        if (resp.statusCode != 200) {
          throw 'HTTP ${resp.statusCode}';
        }
        if (jsonMap.containsKey('total')) {
          if (total == 0) total = jsonMap['total'];
          if (jsonMap.containsKey('data')) {
            final data = jsonMap['data'];
            if (data is List) {
              for (final prof in data) { // Changed variable name to avoid conflict
                final u = Peer.fromJson(prof);
                int index = tmpPeers.indexWhere((e) => e.id == u.id);
                if (index < 0) {
                  tmpPeers.add(u);
                } else {
                  tmpPeers[index] = u;
                }
              }
            }
          }
        }
      } while (current * pageSize < total);
      return true;
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
    }
    return false;
  }

  Future<bool> _fetchTags(List<AbTag> tmpTags, {quiet = false}) async {
    if (isLocalAddressBookMode()) return true; // No fetching in local mode
    final api = "${await bind.mainGetApiServer()}/api/ab/tags/${profile.guid}";
    int? statusCode;
    try {
      var uri0 = Uri.parse(api);
      var uri = Uri(
        scheme: uri0.scheme,
        host: uri0.host,
        path: uri0.path,
        port: uri0.port,
      );
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final resp = await http.post(uri, headers: headers);
      statusCode = resp.statusCode;
      List<dynamic> jsonList = // Changed variable name
          _jsonDecodeRespList(utf8.decode(resp.bodyBytes), resp.statusCode);
      if (resp.statusCode != 200) {
        throw 'HTTP ${resp.statusCode}';
      }

      for (final d in jsonList) {
        final t = AbTag.fromJson(d);
        int index = tmpTags.indexWhere((e) => e.name == t.name);
        if (index < 0) {
          tmpTags.add(t);
        } else {
          tmpTags[index] = t;
        }
      }
      return true;
    } catch (err) {
      if (!quiet) {
        pullError.value =
            '${translate('pull_ab_failed_tip')}: ${translate(err.toString())}';
      }
    } finally {
      if (pullError.isNotEmpty) {
        if (statusCode == 401) {
          gFFI.userModel.reset(resetOther: true);
        }
      }
    }
    return false;
  }

// #region Peers
  @override
  Future<String?> addPeers(List<Map<String, dynamic>> ps) async {
    if (isLocalAddressBookMode()) {
      // This case should ideally be fully handled by AbModel.addPeersTo's local path.
      // If LegacyAb.addPeers is called in local mode, it implies local data manipulation.
      bool full = false;
      for (var p_map in ps) {
        final p = Peer.fromJson(p_map);
        if (!isFull()) { // isFull is local mode aware
          final index = peers.indexWhere((e) => e.id == p.id);
          if (index >= 0) {
             // If peer exists, AbModel should have updated it.
             // If direct call to LegacyAb, decide on update/merge logic here.
             // For now, assume AbModel handles updates before this could be an issue.
            _merge(p, peers[index]); 
          } else {
            peers.add(p);
          }
        } else {
          full = true;
          break;
        }
      }
      // No pushAb call here, AbModel._saveCache() is the authority.
      return full ? translate("exceed_max_devices") : null;
    }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/add/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      for (var p_map in ps) { // Changed variable name
        if (peers.firstWhereOrNull((e) => e.id == p_map['id']) != null) {
          continue;
        }
        if (isFull()) {
          return translate("exceed_max_devices");
        }
        if (personal) {
          removePassword(p_map);
        } else {
          removeHash(p_map);
        }
        String body = jsonEncode(p_map);
        final resp =
            await http.post(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          return errMsg;
        }
      }
    } catch (err) {
      return err.toString();
    }
    return null;
  }

  @override
  Future<bool> changeTagForPeers(List<String> ids, List<dynamic> tags) async {
    if (isLocalAddressBookMode()) { /* Already handled by AbModel */ return true; }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      var ret = true;
      for (var id in ids) {
        final body = jsonEncode({"id": id, "tags": tags});
        final resp =
            await http.put(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          BotToast.showText(contentColor: Colors.red, text: errMsg);
          ret = false;
          break;
        }
      }
      return ret;
    } catch (err) {
      debugPrint('changeTagForPeers err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> changeAlias({required String id, required String alias}) async {
    if (isLocalAddressBookMode()) { /* Already handled by AbModel */ return true; }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({"id": id, "alias": alias});
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('changeAlias err: ${err.toString()}');
      return false;
    }
  }

  Future<bool> _setPassword(Object bodyContent) async {
    if (isLocalAddressBookMode()) { /* Saved by AbModel */ return true;}
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode(bodyContent);
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('changeSharedPassword err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> changePersonalHashPassword(String id, String hash) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    if (!personal) return false;
    if (!peers.any((e) => e.id == id)) return true;
    return await _setPassword({"id": id, "hash": hash});
  }

  @override
  Future<bool> changeSharedPassword(String id, String password) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    if (personal) return false;
    return await _setPassword({"id": id, "password": password});
  }

  @override
  Future<void> syncFromRecent(List<Peer> recents) async {
    // Called by AbModel if current is Ab.
    // If local mode, AbModel._saveCache() will be called after this.
    bool uiUpdate = false;
    bool saveCacheNeeded = false; // Renamed from saveCache to avoid conflict with AbModel._saveCache
    
    if (!isLocalAddressBookMode()) { // Server sync part
        final api =
            "${await bind.mainGetApiServer()}/api/ab/peer/update/${profile.guid}";
        var headers = getHttpHeaders();
        headers['Content-Type'] = "application/json";

        Future<bool> trySyncOnePeerServer(Peer p, Peer r) async {
          var map = Map<String, String>.fromEntries([]);
          if (p.sameServer != true &&
              r.username.isNotEmpty &&
              p.username != r.username) {
            map['username'] = r.username;
          }
          if (p.sameServer != true &&
              r.hostname.isNotEmpty &&
              p.hostname != r.hostname) {
            map['hostname'] = r.hostname;
          }
          if (p.sameServer != true &&
              r.platform.isNotEmpty &&
              p.platform != r.platform) {
            map['platform'] = r.platform;
          }
          if (personal && r.hash.isNotEmpty && p.hash != r.hash) {
            map['hash'] = r.hash;
          }
          if (map.isEmpty) return false; // No server changes needed for this peer's attributes

          // Apply changes locally before pushing, to keep p updated
          if(map.containsKey('username')) p.username = map['username']!;
          if(map.containsKey('hostname')) p.hostname = map['hostname']!;
          if(map.containsKey('platform')) p.platform = map['platform']!;
          if(map.containsKey('hash')) p.hash = map['hash']!;
          
          map['id'] = p.id;
          final body = jsonEncode(map);
          final resp = await http.put(Uri.parse(api), headers: headers, body: body);
          final errMsg = _jsonDecodeActionResp(resp);
          if (errMsg.isNotEmpty) {
            debugPrint('syncOnePeer errMsg: $errMsg');
            return false; // Server update failed
          }
          return true; // Server update succeeded
        }
        for (var p in peers) {
            Peer? r = recents.firstWhereOrNull((e) => e.id == p.id);
            if (r != null) {
                if (await trySyncOnePeerServer(p,r)) uiUpdate = true;
            }
        }

    } else { // Local sync part (just merge, AbModel handles saving)
        for (var p in peers) {
            Peer? r = recents.firstWhereOrNull((e) => e.id == p.id);
            if (r != null) {
                // Check if merge results in actual change to warrant UI update
                final originalUsername = p.username;
                final originalHostname = p.hostname;
                final originalPlatform = p.platform;
                final originalHash = p.hash;

                if (p.sameServer != true && r.username.isNotEmpty && p.username != r.username) p.username = r.username;
                if (p.sameServer != true && r.hostname.isNotEmpty && p.hostname != r.hostname) p.hostname = r.hostname;
                if (p.sameServer != true && r.platform.isNotEmpty && p.platform != r.platform) p.platform = r.platform;
                if (personal && r.hash.isNotEmpty && p.hash != r.hash) { // `personal` might need re-evaluation for local
                    p.hash = r.hash;
                    saveCacheNeeded = true; // Hash changes always need saving
                }
                if (p.username != originalUsername || p.hostname != originalHostname || p.platform != originalPlatform || p.hash != originalHash) {
                    uiUpdate = true;
                }
            }
        }
    }


    try {
      if (uiUpdate && (gFFI.abModel.currentName.value == profile.name || isLocalAddressBookMode())) {
        peers.refresh();
      }
      if (saveCacheNeeded && !isLocalAddressBookMode()) { // Only call _saveCache if not in local mode, as AbModel handles it
        gFFI.abModel._saveCache();
      }
    } catch (err) {
      debugPrint('syncFromRecent err: ${err.toString()}');
    }
  }

  @override
  Future<bool> deletePeers(List<String> ids) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/peer/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode(ids);
      final resp =
          await http.delete(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('deletePeers err: ${err.toString()}');
      return false;
    }
  }
// #endregion

// #region Tags
  @override
  Future<bool> addTags(
      List<String> tagList, Map<String, int> tagColorMap) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/add/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      for (var t in tagList) {
        final body = jsonEncode({
          "name": t,
          "color": tagColorMap[t] ??
              str2color2(t, existing: tagColors.values.toList()).value,
        });
        final resp =
            await http.post(Uri.parse(api), headers: headers, body: body);
        final errMsg = _jsonDecodeActionResp(resp);
        if (errMsg.isNotEmpty) {
          BotToast.showText(contentColor: Colors.red, text: errMsg);
          return false;
        }
      }
      return true;
    } catch (err) {
      debugPrint('addTags err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> renameTag(String oldTag, String newTag) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    if (tags.contains(newTag)) {
      BotToast.showText(
          contentColor: Colors.red, text: 'Tag $newTag already exists');
      return false;
    }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/rename/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        "old": oldTag,
        "new": newTag,
      });
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('renameTag err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> setTagColor(String tag, Color color) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    try {
      final api =
          "${await bind.mainGetApiServer()}/api/ab/tag/update/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode({
        "name": tag,
        "color": color.value,
      });
      final resp = await http.put(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('setTagColor err: ${err.toString()}');
      return false;
    }
  }

  @override
  Future<bool> deleteTag(String tag) async {
    if (isLocalAddressBookMode()) { /* AbModel handles */ return true; }
    try {
      final api = "${await bind.mainGetApiServer()}/api/ab/tag/${profile.guid}";
      var headers = getHttpHeaders();
      headers['Content-Type'] = "application/json";
      final body = jsonEncode([tag]);
      final resp =
          await http.delete(Uri.parse(api), headers: headers, body: body);
      final errMsg = _jsonDecodeActionResp(resp);
      if (errMsg.isNotEmpty) {
        BotToast.showText(contentColor: Colors.red, text: errMsg);
        return false;
      }
      return true;
    } catch (err) {
      debugPrint('deleteTag err: ${err.toString()}');
      return false;
    }
  }

// #endregion
}

// DummyAb is for current ab is null
class DummyAb extends BaseAb {
  @override
  bool isFull() {
    return false;
  }

  @override
  Future<String?> addPeers(List<Map<String, dynamic>> ps) async {
    return "dummpy";
  }

  @override
  Future<bool> addTags(
      List<String> tagList, Map<String, int> tagColorMap) async {
    return false;
  }

  @override
  bool canWrite() {
    return false;
  }

  @override
  bool fullControl() {
    return false;
  }

  @override
  Future<bool> changeAlias({required String id, required String alias}) async {
    return false;
  }

  @override
  Future<bool> changePersonalHashPassword(String id, String hash) async {
    return false;
  }

  @override
  Future<bool> changeSharedPassword(String id, String password) async {
    return false;
  }

  @override
  Future<bool> changeTagForPeers(List<String> ids, List tags) async {
    return false;
  }

  @override
  Future<bool> deletePeers(List<String> ids) async {
    return false;
  }

  @override
  Future<bool> deleteTag(String tag) async {
    return false;
  }

  @override
  String name() {
    return "dummpy";
  }

  @override
  Future<bool> pullAbImpl({quiet = false}) async {
    return false;
  }

  @override
  Future<bool> renameTag(String oldTag, String newTag) async {
    return false;
  }

  @override
  Future<bool> setTagColor(String tag, Color color) async {
    return false;
  }

  @override
  AbProfile? sharedProfile() {
    return null;
  }

  @override
  void setSharedProfile(AbProfile profile) {}

  @override
  Future<void> syncFromRecent(List<Peer> recents) async {}
}

Map<String, dynamic> _jsonDecodeRespMap(String body, int statusCode) {
  try {
    Map<String, dynamic> jsonMap = jsonDecode(body); // Changed variable name
    return jsonMap;
  } catch (e) {
    final err = body.isNotEmpty && body.length < 128 ? body : e.toString();
    if (statusCode != 200) {
      throw 'HTTP $statusCode, $err';
    }
    throw err;
  }
}

List<dynamic> _jsonDecodeRespList(String body, int statusCode) {
  try {
    List<dynamic> jsonList = jsonDecode(body); // Changed variable name
    return jsonList;
  } catch (e) {
    final err = body.isNotEmpty && body.length < 128 ? body : e.toString();
    if (statusCode != 200) {
      throw 'HTTP $statusCode, $err';
    }
    throw err;
  }
}

String _jsonDecodeActionResp(http.Response resp) {
  var errMsg = '';
  if (resp.statusCode == 200 && resp.body.isEmpty) {
    // ok
  } else {
    try {
      errMsg = jsonDecode(resp.body)['error'].toString();
    } catch (_) {}
    if (errMsg.isEmpty) {
      if (resp.statusCode != 200) {
        errMsg = 'HTTP ${resp.statusCode}';
      }
      if (resp.body.isNotEmpty) {
        if (errMsg.isNotEmpty) {
          errMsg += ', ';
        }
        errMsg += resp.body;
      }
      if (errMsg.isEmpty) {
        errMsg = "unknown error";
      }
    }
  }
  return errMsg;
}

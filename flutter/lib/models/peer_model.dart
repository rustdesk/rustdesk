import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:get/get.dart';
import 'platform_model.dart';
// ignore: depend_on_referenced_packages
import 'package:collection/collection.dart';

class Peer {
  final String id;
  String hash;
  String username; // pc username
  String hostname;
  String platform;
  String alias;
  List<dynamic> tags;
  bool forceAlwaysRelay = false;
  String rdpPort;
  String rdpUsername;
  bool online = false;
  String loginName; //login username

  String getId() {
    if (alias != '') {
      return alias;
    }
    return id;
  }

  Peer.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        hash = json['hash'] ?? '',
        username = json['username'] ?? '',
        hostname = json['hostname'] ?? '',
        platform = json['platform'] ?? '',
        alias = json['alias'] ?? '',
        tags = json['tags'] ?? [],
        forceAlwaysRelay = json['forceAlwaysRelay'] == 'true',
        rdpPort = json['rdpPort'] ?? '',
        rdpUsername = json['rdpUsername'] ?? '',
        loginName = json['loginName'] ?? '';

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      "id": id,
      "hash": hash,
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "alias": alias,
      "tags": tags,
      "forceAlwaysRelay": forceAlwaysRelay.toString(),
      "rdpPort": rdpPort,
      "rdpUsername": rdpUsername,
      'loginName': loginName,
    };
  }

  Map<String, dynamic> toAbUploadJson() {
    return <String, dynamic>{
      "id": id,
      "hash": hash,
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "alias": alias,
      "tags": tags,
    };
  }

  Map<String, dynamic> toGroupCacheJson() {
    return <String, dynamic>{
      "id": id,
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "login_name": loginName,
    };
  }

  Peer({
    required this.id,
    required this.hash,
    required this.username,
    required this.hostname,
    required this.platform,
    required this.alias,
    required this.tags,
    required this.forceAlwaysRelay,
    required this.rdpPort,
    required this.rdpUsername,
    required this.loginName,
  });

  Peer.loading()
      : this(
          id: '...',
          hash: '',
          username: '...',
          hostname: '...',
          platform: '...',
          alias: '',
          tags: [],
          forceAlwaysRelay: false,
          rdpPort: '',
          rdpUsername: '',
          loginName: '',
        );
  bool equal(Peer other) {
    return id == other.id &&
        hash == other.hash &&
        username == other.username &&
        hostname == other.hostname &&
        platform == other.platform &&
        alias == other.alias &&
        tags.equals(other.tags) &&
        forceAlwaysRelay == other.forceAlwaysRelay &&
        rdpPort == other.rdpPort &&
        rdpUsername == other.rdpUsername &&
        loginName == other.loginName;
  }

  Peer.copy(Peer other)
      : this(
          id: other.id,
          hash: other.hash,
          username: other.username,
          hostname: other.hostname,
          platform: other.platform,
          alias: other.alias,
          tags: other.tags.toList(),
          forceAlwaysRelay: other.forceAlwaysRelay,
          rdpPort: other.rdpPort,
          rdpUsername: other.rdpUsername,
          loginName: other.loginName,
        );
}

enum UpdateEvent { online, load }

class Peers extends ChangeNotifier {
  final String name;
  final String loadEvent;
  List<Peer> peers = List.empty(growable: true);
  final RxList<Peer>? initPeers;
  UpdateEvent event = UpdateEvent.load;
  static const _cbQueryOnlines = 'callback_query_onlines';

  Peers(
      {required this.name, required this.initPeers, required this.loadEvent}) {
    peers = initPeers ?? [];
    platformFFI.registerEventHandler(_cbQueryOnlines, name, (evt) async {
      _updateOnlineState(evt);
    });
    platformFFI.registerEventHandler(loadEvent, name, (evt) async {
      _updatePeers(evt);
    });
  }

  @override
  void dispose() {
    platformFFI.unregisterEventHandler(_cbQueryOnlines, name);
    platformFFI.unregisterEventHandler(loadEvent, name);
    super.dispose();
  }

  Peer getByIndex(int index) {
    if (index < peers.length) {
      return peers[index];
    } else {
      return Peer.loading();
    }
  }

  int getPeersCount() {
    return peers.length;
  }

  void _updateOnlineState(Map<String, dynamic> evt) {
    evt['onlines'].split(',').forEach((online) {
      for (var i = 0; i < peers.length; i++) {
        if (peers[i].id == online) {
          peers[i].online = true;
        }
      }
    });

    evt['offlines'].split(',').forEach((offline) {
      for (var i = 0; i < peers.length; i++) {
        if (peers[i].id == offline) {
          peers[i].online = false;
        }
      }
    });

    event = UpdateEvent.online;
    notifyListeners();
  }

  void _updatePeers(Map<String, dynamic> evt) {
    final onlineStates = _getOnlineStates();
    if (initPeers != null) {
      peers = initPeers!;
    } else {
      peers = _decodePeers(evt['peers']);
    }
    for (var peer in peers) {
      final state = onlineStates[peer.id];
      peer.online = state != null && state != false;
    }
    event = UpdateEvent.load;
    notifyListeners();
  }

  Map<String, bool> _getOnlineStates() {
    var onlineStates = <String, bool>{};
    for (var peer in peers) {
      onlineStates[peer.id] = peer.online;
    }
    return onlineStates;
  }

  List<Peer> _decodePeers(String peersStr) {
    try {
      if (peersStr == "") return [];
      List<dynamic> peers = json.decode(peersStr);
      return peers.map((peer) {
        return Peer.fromJson(peer as Map<String, dynamic>);
      }).toList();
    } catch (e) {
      debugPrint('peers(): $e');
    }
    return [];
  }
}

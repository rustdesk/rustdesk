import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';
import 'platform_model.dart';
// ignore: depend_on_referenced_packages
import 'package:collection/collection.dart';

class HashSalt {
  late final Uint8List hash;
  late final String salt;

  HashSalt({required this.hash, required this.salt});

  HashSalt.fromJson(dynamic v) {
    if (v is Map<String, dynamic>) {
      final hashValue = v['hash'];
      if (hashValue is Uint8List) {
        hash = hashValue;
      } else if (hashValue is List<dynamic>) {
        // Convert List<dynamic> to Uint8List
        hash = Uint8List.fromList(hashValue.cast<int>());
      } else if (hashValue is List<int>) {
        hash = Uint8List.fromList(hashValue);
      } else {
        hash = Uint8List(0);
      }
      salt = v['salt'] is String ? v['salt'] : '';
    } else {
      hash = Uint8List(0);
      salt = '';
    }
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{'hash': hash, 'salt': salt};
  }
}

class Peer {
  final String id;
  String hash; // personal ab hash password
  String password; // shared ab password
  HashSalt sharedPassword;
  String username; // pc username
  String hostname;
  String platform;
  String alias;
  List<dynamic> tags;
  bool forceAlwaysRelay = false;
  String rdpPort;
  String rdpUsername;
  bool online = false;
  String user;
  String loginName; //login username
  String device_group_name;
  bool? sameServer;

  String getId() {
    if (alias != '') {
      return alias;
    }
    return id;
  }

  Peer.fromJson(Map<String, dynamic> json)
      : id = json['id'] ?? '',
        hash = json['hash'] ?? '',
        password = json['password'] ?? '',
        sharedPassword = HashSalt.fromJson(json['shared_password']),
        username = json['username'] ?? '',
        hostname = json['hostname'] ?? '',
        platform = json['platform'] ?? '',
        alias = json['alias'] ?? '',
        tags = json['tags'] ?? [],
        forceAlwaysRelay = json['forceAlwaysRelay'] == 'true',
        rdpPort = json['rdpPort'] ?? '',
        rdpUsername = json['rdpUsername'] ?? '',
        user = json['user'] ?? '',
        loginName = json['login_name'] ?? '',
        device_group_name = json['device_group_name'] ?? '',
        sameServer = json['same_server'];

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      "id": id,
      "hash": hash,
      "password": password,
      "shared_password": sharedPassword.toJson(),
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "alias": alias,
      "tags": tags,
      "forceAlwaysRelay": forceAlwaysRelay.toString(),
      "rdpPort": rdpPort,
      "rdpUsername": rdpUsername,
      'user': user,
      'login_name': loginName,
      'device_group_name': device_group_name,
      'same_server': sameServer,
    };
  }

  Map<String, dynamic> toCustomJson({required bool includingHash}) {
    var res = <String, dynamic>{
      "id": id,
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "alias": alias,
      "tags": tags,
    };
    if (includingHash) {
      res['hash'] = hash;
    }
    return res;
  }

  Map<String, dynamic> toGroupCacheJson() {
    return <String, dynamic>{
      "id": id,
      "username": username,
      "hostname": hostname,
      "platform": platform,
      "login_name": loginName,
      'user': user,
      "device_group_name": device_group_name,
    };
  }

  Peer({
    required this.id,
    required this.hash,
    required this.password,
    required this.sharedPassword,
    required this.username,
    required this.hostname,
    required this.platform,
    required this.alias,
    required this.tags,
    required this.forceAlwaysRelay,
    required this.rdpPort,
    required this.rdpUsername,
    required this.user,
    required this.loginName,
    required this.device_group_name,
    this.sameServer,
  });

  Peer.loading()
      : this(
          id: '...',
          hash: '',
          password: '',
          sharedPassword: HashSalt(hash: Uint8List(0), salt: ''),
          username: '...',
          hostname: '...',
          platform: '...',
          alias: '',
          tags: [],
          forceAlwaysRelay: false,
          rdpPort: '',
          rdpUsername: '',
          user: '',
          loginName: '',
          device_group_name: '',
        );
  bool equal(Peer other) {
    return id == other.id &&
        hash == other.hash &&
        password == other.password &&
        sharedPassword == other.sharedPassword &&
        username == other.username &&
        hostname == other.hostname &&
        platform == other.platform &&
        alias == other.alias &&
        tags.equals(other.tags) &&
        forceAlwaysRelay == other.forceAlwaysRelay &&
        rdpPort == other.rdpPort &&
        rdpUsername == other.rdpUsername &&
        device_group_name == other.device_group_name &&
        loginName == other.loginName;
  }

  Peer.copy(Peer other)
      : this(
            id: other.id,
            hash: other.hash,
            password: other.password,
            sharedPassword: other.sharedPassword,
            username: other.username,
            hostname: other.hostname,
            platform: other.platform,
            alias: other.alias,
            tags: other.tags.toList(),
            forceAlwaysRelay: other.forceAlwaysRelay,
            rdpPort: other.rdpPort,
            rdpUsername: other.rdpUsername,
            user: other.user,
            loginName: other.loginName,
            device_group_name: other.device_group_name,
            sameServer: other.sameServer);

  bool hasValidPassword() {
    if (withPublic()) {
      return sharedPassword.hash.isNotEmpty && sharedPassword.salt.isNotEmpty;
    } else {
      return password.isNotEmpty;
    }
  }
}

enum UpdateEvent { online, load }

typedef GetInitPeers = RxList<Peer> Function();

class Peers extends ChangeNotifier {
  final String name;
  final String loadEvent;
  List<Peer> peers = List.empty(growable: true);
  // Part of the peers that are not in the rest peers list.
  // When there're too many peers, we may want to load the front 100 peers first,
  // so we can see peers in UI quickly. `restPeerIds` is the rest peers' ids.
  // And then load all peers later.
  List<String> restPeerIds = List.empty(growable: true);
  final GetInitPeers? getInitPeers;
  UpdateEvent event = UpdateEvent.load;
  static const _cbQueryOnlines = 'callback_query_onlines';

  Peers(
      {required this.name,
      required this.getInitPeers,
      required this.loadEvent}) {
    peers = getInitPeers?.call() ?? [];
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
    int changedCount = 0;
    evt['onlines'].split(',').forEach((online) {
      for (var i = 0; i < peers.length; i++) {
        if (peers[i].id == online) {
          if (!peers[i].online) {
            changedCount += 1;
            peers[i].online = true;
          }
        }
      }
    });

    evt['offlines'].split(',').forEach((offline) {
      for (var i = 0; i < peers.length; i++) {
        if (peers[i].id == offline) {
          if (peers[i].online) {
            changedCount += 1;
            peers[i].online = false;
          }
        }
      }
    });

    if (changedCount > 0) {
      event = UpdateEvent.online;
      notifyListeners();
    }
  }

  void _updatePeers(Map<String, dynamic> evt) {
    final onlineStates = _getOnlineStates();
    if (getInitPeers != null) {
      peers = getInitPeers?.call() ?? [];
    } else {
      peers = _decodePeers(evt['peers']);
    }

    restPeerIds = [];
    if (evt['ids'] != null) {
      restPeerIds = (evt['ids'] as String).split(',');
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

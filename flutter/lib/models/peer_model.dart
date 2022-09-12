import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'platform_model.dart';

class Peer {
  final String id;
  final String username;
  final String hostname;
  final String platform;
  final List<dynamic> tags;
  bool online = false;

  Peer.fromJson(this.id, Map<String, dynamic> json)
      : username = json['username'] ?? '',
        hostname = json['hostname'] ?? '',
        platform = json['platform'] ?? '',
        tags = json['tags'] ?? [];

  Peer({
    required this.id,
    required this.username,
    required this.hostname,
    required this.platform,
    required this.tags,
  });

  Peer.loading()
      : this(
            id: '...',
            username: '...',
            hostname: '...',
            platform: '...',
            tags: []);
}

class Peers extends ChangeNotifier {
  final String name;
  final String loadEvent;
  List<Peer> peers;
  static const _cbQueryOnlines = 'callback_query_onlines';

  Peers({required this.name, required this.peers, required this.loadEvent}) {
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

    notifyListeners();
  }

  void _updatePeers(Map<String, dynamic> evt) {
    final onlineStates = _getOnlineStates();
    peers = _decodePeers(evt['peers']);
    for (var peer in peers) {
      final state = onlineStates[peer.id];
      peer.online = state != null && state != false;
    }
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
      return peers
          .map((s) => s as List<dynamic>)
          .map((s) =>
              Peer.fromJson(s[0] as String, s[1] as Map<String, dynamic>))
          .toList();
    } catch (e) {
      debugPrint('peers(): $e');
    }
    return [];
  }
}

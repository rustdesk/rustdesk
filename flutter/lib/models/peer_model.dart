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

  Peer.fromJson(String id, Map<String, dynamic> json)
      : id = id,
        username = json['username'] ?? '',
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
  late String _name;
  late List<Peer> _peers;
  late final _loadEvent;
  static const _cbQueryOnlines = 'callback_query_onlines';

  Peers(String name, String loadEvent, List<Peer> _initPeers) {
    _name = name;
    _loadEvent = loadEvent;
    _peers = _initPeers;
    platformFFI.registerEventHandler(_cbQueryOnlines, _name, (evt) {
      _updateOnlineState(evt);
    });
    platformFFI.registerEventHandler(_loadEvent, _name, (evt) {
      _updatePeers(evt);
    });
  }

  List<Peer> get peers => _peers;

  @override
  void dispose() {
    platformFFI.unregisterEventHandler(_cbQueryOnlines, _name);
    platformFFI.unregisterEventHandler(_loadEvent, _name);
    super.dispose();
  }

  Peer getByIndex(int index) {
    if (index < _peers.length) {
      return _peers[index];
    } else {
      return Peer.loading();
    }
  }

  int getPeersCount() {
    return _peers.length;
  }

  void _updateOnlineState(Map<String, dynamic> evt) {
    evt['onlines'].split(',').forEach((online) {
      for (var i = 0; i < _peers.length; i++) {
        if (_peers[i].id == online) {
          _peers[i].online = true;
        }
      }
    });

    evt['offlines'].split(',').forEach((offline) {
      for (var i = 0; i < _peers.length; i++) {
        if (_peers[i].id == offline) {
          _peers[i].online = false;
        }
      }
    });

    notifyListeners();
  }

  void _updatePeers(Map<String, dynamic> evt) {
    final onlineStates = _getOnlineStates();
    _peers = _decodePeers(evt['peers']);
    _peers.forEach((peer) {
      final state = onlineStates[peer.id];
      peer.online = state != null && state != false;
    });
    notifyListeners();
  }

  Map<String, bool> _getOnlineStates() {
    var onlineStates = new Map<String, bool>();
    _peers.forEach((peer) {
      onlineStates[peer.id] = peer.online;
    });
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
      print('peers(): $e');
    }
    return [];
  }
}

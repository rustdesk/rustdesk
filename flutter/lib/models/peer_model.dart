import 'package:flutter/foundation.dart';
import '../../common.dart';

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
  late var _peers;
  static const cbQueryOnlines = 'callback_query_onlines';

  Peers(String name, List<Peer> peers) {
    _name = name;
    _peers = peers;
    gFFI.ffiModel.platformFFI.registerEventHandler(cbQueryOnlines, _name,
        (evt) {
      _updateOnlineState(evt);
    });
  }

  List<Peer> get peers => _peers;

  @override
  void dispose() {
    gFFI.ffiModel.platformFFI.unregisterEventHandler(cbQueryOnlines, _name);
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
}

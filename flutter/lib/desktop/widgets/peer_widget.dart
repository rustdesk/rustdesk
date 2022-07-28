import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:provider/provider.dart';
import 'package:visibility_detector/visibility_detector.dart';
import 'package:window_manager/window_manager.dart';

import '../../models/peer_model.dart';
import '../../common.dart';
import 'peercard_widget.dart';

typedef OffstageFunc = bool Function(Peer peer);
typedef PeerCardWidgetFunc = Widget Function(Peer peer);

class _PeerWidget extends StatefulWidget {
  late final _name;
  late final _peers;
  late final OffstageFunc _offstageFunc;
  late final PeerCardWidgetFunc _peerCardWidgetFunc;
  _PeerWidget(String name, List<Peer> peers, OffstageFunc offstageFunc,
      PeerCardWidgetFunc peerCardWidgetFunc,
      {Key? key})
      : super(key: key) {
    _name = name;
    _peers = peers;
    _offstageFunc = offstageFunc;
    _peerCardWidgetFunc = peerCardWidgetFunc;
  }

  @override
  _PeerWidgetState createState() => _PeerWidgetState();
}

/// State for the peer widget.
class _PeerWidgetState extends State<_PeerWidget> with WindowListener {
  static const int _maxQueryCount = 3;

  var _curPeers = Set<String>();
  var _lastChangeTime = DateTime.now();
  var _lastQueryPeers = Set<String>();
  var _lastQueryTime = DateTime.now().subtract(Duration(hours: 1));
  var _queryCoun = 0;
  var _exit = false;

  _PeerWidgetState() {
    _startCheckOnlines();
  }

  @override
  void initState() {
    windowManager.addListener(this);
    super.initState();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    _exit = true;
    super.dispose();
  }

  @override
  void onWindowFocus() {
    _queryCoun = 0;
  }

  @override
  Widget build(BuildContext context) {
    final space = 8.0;
    return ChangeNotifierProvider<Peers>(
      create: (context) => Peers(super.widget._name, super.widget._peers),
      child: SingleChildScrollView(
          child: Consumer<Peers>(
              builder: (context, peers, child) => Wrap(
                  children: () {
                    final cards = <Widget>[];
                    peers.peers.forEach((peer) {
                      cards.add(Offstage(
                          offstage: super.widget._offstageFunc(peer),
                          child: Container(
                            width: 225,
                            height: 150,
                            child: VisibilityDetector(
                              key: Key('${peer.id}'),
                              onVisibilityChanged: (info) {
                                final peerId = (info.key as ValueKey).value;
                                if (info.visibleFraction > 0.00001) {
                                  _curPeers.add(peerId);
                                } else {
                                  _curPeers.remove(peerId);
                                }
                                _lastChangeTime = DateTime.now();
                              },
                              child: super.widget._peerCardWidgetFunc(peer),
                            ),
                          )));
                    });
                    return cards;
                  }(),
                  spacing: space,
                  runSpacing: space))),
    );
  }

  // ignore: todo
  // TODO: variables walk through async tasks?
  void _startCheckOnlines() {
    () async {
      while (!_exit) {
        final now = DateTime.now();
        if (!setEquals(_curPeers, _lastQueryPeers)) {
          if (now.difference(_lastChangeTime) > Duration(seconds: 1)) {
            gFFI.ffiModel.platformFFI.ffiBind
                .queryOnlines(ids: _curPeers.toList(growable: false));
            _lastQueryPeers = {..._curPeers};
            _lastQueryTime = DateTime.now();
            _queryCoun = 0;
          }
        } else {
          if (_queryCoun < _maxQueryCount) {
            if (now.difference(_lastQueryTime) > Duration(seconds: 20)) {
              gFFI.ffiModel.platformFFI.ffiBind
                  .queryOnlines(ids: _curPeers.toList(growable: false));
              _lastQueryTime = DateTime.now();
              _queryCoun += 1;
            }
          }
        }
        await Future.delayed(Duration(milliseconds: 300));
      }
    }();
  }
}

abstract class BasePeerWidget extends StatelessWidget {
  late final _name;
  late final OffstageFunc _offstageFunc;
  late final PeerCardWidgetFunc _peerCardWidgetFunc;

  BasePeerWidget({Key? key}) : super(key: key) {}

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<Widget>(future: () async {
      return _PeerWidget(
          _name, await _loadPeers(), _offstageFunc, _peerCardWidgetFunc);
    }(), builder: (context, snapshot) {
      if (snapshot.hasData) {
        return snapshot.data!;
      } else {
        return Offstage();
      }
    });
  }

  @protected
  Future<List<Peer>> _loadPeers();
}

class RecentPeerWidget extends BasePeerWidget {
  RecentPeerWidget({Key? key}) : super(key: key) {
    super._name = "recent peer";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => RecentPeerCard(peer: peer);
  }

  Future<List<Peer>> _loadPeers() async {
    return gFFI.peers();
  }
}

class FavoritePeerWidget extends BasePeerWidget {
  FavoritePeerWidget({Key? key}) : super(key: key) {
    super._name = "favorite peer";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => FavoritePeerCard(peer: peer);
  }

  @override
  Future<List<Peer>> _loadPeers() async {
    return await gFFI.bind.mainGetFav().then((peers) async {
      final peersEntities = await Future.wait(peers
              .map((id) => gFFI.bind.mainGetPeers(id: id))
              .toList(growable: false))
          .then((peers_str) {
        final len = peers_str.length;
        final ps = List<Peer>.empty(growable: true);
        for (var i = 0; i < len; i++) {
          print("${peers[i]}: ${peers_str[i]}");
          ps.add(Peer.fromJson(peers[i], jsonDecode(peers_str[i])['info']));
        }
        return ps;
      });
      return peersEntities;
    });
  }
}

class DiscoveredPeerWidget extends BasePeerWidget {
  DiscoveredPeerWidget({Key? key}) : super(key: key) {
    super._name = "discovered peer";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => DiscoveredPeerCard(peer: peer);
  }

  Future<List<Peer>> _loadPeers() async {
    return await gFFI.bind.mainGetLanPeers().then((peers_string) {
      debugPrint(peers_string);
      return [];
    });
  }
}

class AddressBookPeerWidget extends BasePeerWidget {
  AddressBookPeerWidget({Key? key}) : super(key: key) {
    super._name = "address book peer";
    super._offstageFunc =
        (Peer peer) => !_hitTag(gFFI.abModel.selectedTags, peer.tags);
    super._peerCardWidgetFunc = (Peer peer) => AddressBookPeerCard(peer: peer);
  }

  Future<List<Peer>> _loadPeers() async {
    return gFFI.abModel.peers.map((e) {
      return Peer.fromJson(e['id'], e);
    }).toList();
  }

  bool _hitTag(List<dynamic> selectedTags, List<dynamic> idents) {
    if (selectedTags.isEmpty) {
      return true;
    }
    if (idents.isEmpty) {
      return false;
    }
    for (final tag in selectedTags) {
      if (!idents.contains(tag)) {
        return false;
      }
    }
    return true;
  }
}

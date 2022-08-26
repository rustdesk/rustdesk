import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:visibility_detector/visibility_detector.dart';
import 'package:window_manager/window_manager.dart';

import '../../common.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import 'peercard_widget.dart';

typedef OffstageFunc = bool Function(Peer peer);
typedef PeerCardWidgetFunc = Widget Function(Peer peer);

/// for peer search text, global obs value
final peerSearchText = "".obs;
final peerSearchTextController =
    TextEditingController(text: peerSearchText.value);

class _PeerWidget extends StatefulWidget {
  late final _peers;
  late final OffstageFunc _offstageFunc;
  late final PeerCardWidgetFunc _peerCardWidgetFunc;

  _PeerWidget(Peers peers, OffstageFunc offstageFunc,
      PeerCardWidgetFunc peerCardWidgetFunc,
      {Key? key})
      : super(key: key) {
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
  void onWindowMinimize() {
    _queryCoun = _maxQueryCount;
  }

  @override
  Widget build(BuildContext context) {
    final space = 12.0;
    return ChangeNotifierProvider<Peers>(
      create: (context) => super.widget._peers,
      child: Consumer<Peers>(
          builder: (context, peers, child) => peers.peers.isEmpty
              ? Center(
                  child: Text(translate("Empty")),
                )
              : SingleChildScrollView(
                  child: ObxValue<RxString>((searchText) {
                    return FutureBuilder<List<Peer>>(
                      builder: (context, snapshot) {
                        if (snapshot.hasData) {
                          final peers = snapshot.data!;
                          final cards = <Widget>[];
                          for (final peer in peers) {
                            cards.add(Offstage(
                                key: ValueKey("off${peer.id}"),
                                offstage: super.widget._offstageFunc(peer),
                                child: Obx(
                                  () => SizedBox(
                                    width: 220,
                                    height:
                                        peerCardUiType.value == PeerUiType.grid
                                            ? 140
                                            : 42,
                                    child: VisibilityDetector(
                                      key: ValueKey(peer.id),
                                      onVisibilityChanged: (info) {
                                        final peerId =
                                            (info.key as ValueKey).value;
                                        if (info.visibleFraction > 0.00001) {
                                          _curPeers.add(peerId);
                                        } else {
                                          _curPeers.remove(peerId);
                                        }
                                        _lastChangeTime = DateTime.now();
                                      },
                                      child: super
                                          .widget
                                          ._peerCardWidgetFunc(peer),
                                    ),
                                  ),
                                )));
                          }
                          return Wrap(
                              spacing: space,
                              runSpacing: space,
                              children: cards);
                        } else {
                          return const Center(
                            child: CircularProgressIndicator(),
                          );
                        }
                      },
                      future: matchPeers(searchText.value, peers.peers),
                    );
                  }, peerSearchText),
                )),
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
            if (_curPeers.length > 0) {
              platformFFI.ffiBind
                  .queryOnlines(ids: _curPeers.toList(growable: false));
              _lastQueryPeers = {..._curPeers};
              _lastQueryTime = DateTime.now();
              _queryCoun = 0;
            }
          }
        } else {
          if (_queryCoun < _maxQueryCount) {
            if (now.difference(_lastQueryTime) > Duration(seconds: 20)) {
              if (_curPeers.length > 0) {
                platformFFI.ffiBind
                    .queryOnlines(ids: _curPeers.toList(growable: false));
                _lastQueryTime = DateTime.now();
                _queryCoun += 1;
              }
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
  late final _loadEvent;
  late final OffstageFunc _offstageFunc;
  late final PeerCardWidgetFunc _peerCardWidgetFunc;
  late final List<Peer> _initPeers;

  BasePeerWidget({Key? key}) : super(key: key) {}

  @override
  Widget build(BuildContext context) {
    return _PeerWidget(Peers(_name, _loadEvent, _initPeers), _offstageFunc,
        _peerCardWidgetFunc);
  }
}

class RecentPeerWidget extends BasePeerWidget {
  RecentPeerWidget({Key? key}) : super(key: key) {
    super._name = "recent peer";
    super._loadEvent = "load_recent_peers";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => RecentPeerCard(
          peer: peer,
        );
    super._initPeers = [];
  }

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadRecentPeers();
    return widget;
  }
}

class FavoritePeerWidget extends BasePeerWidget {
  FavoritePeerWidget({Key? key}) : super(key: key) {
    super._name = "favorite peer";
    super._loadEvent = "load_fav_peers";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => FavoritePeerCard(peer: peer);
    super._initPeers = [];
  }

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadFavPeers();
    return widget;
  }
}

class DiscoveredPeerWidget extends BasePeerWidget {
  DiscoveredPeerWidget({Key? key}) : super(key: key) {
    super._name = "discovered peer";
    super._loadEvent = "load_lan_peers";
    super._offstageFunc = (Peer _peer) => false;
    super._peerCardWidgetFunc = (Peer peer) => DiscoveredPeerCard(peer: peer);
    super._initPeers = [];
  }

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadLanPeers();
    return widget;
  }
}

class AddressBookPeerWidget extends BasePeerWidget {
  AddressBookPeerWidget({Key? key}) : super(key: key) {
    super._name = "address book peer";
    super._offstageFunc =
        (Peer peer) => !_hitTag(gFFI.abModel.selectedTags, peer.tags);
    super._peerCardWidgetFunc = (Peer peer) => AddressBookPeerCard(peer: peer);
    super._initPeers = _loadPeers();
  }

  List<Peer> _loadPeers() {
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

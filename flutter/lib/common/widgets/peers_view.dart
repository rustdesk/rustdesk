import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:visibility_detector/visibility_detector.dart';
import 'package:window_manager/window_manager.dart';

import '../../common.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import 'peer_card.dart';

typedef PeerFilter = bool Function(Peer peer);
typedef PeerCardBuilder = Widget Function(Peer peer);

/// for peer search text, global obs value
final peerSearchText = "".obs;
final peerSearchTextController =
    TextEditingController(text: peerSearchText.value);

class _PeersView extends StatefulWidget {
  final Peers peers;
  final PeerFilter? peerFilter;
  final PeerCardBuilder peerCardBuilder;

  const _PeersView(
      {required this.peers,
      required this.peerCardBuilder,
      this.peerFilter,
      Key? key})
      : super(key: key);

  @override
  _PeersViewState createState() => _PeersViewState();
}

/// State for the peer widget.
class _PeersViewState extends State<_PeersView> with WindowListener {
  static const int _maxQueryCount = 3;
  final space = isDesktop ? 12.0 : 8.0;
  final _curPeers = <String>{};
  var _lastChangeTime = DateTime.now();
  var _lastQueryPeers = <String>{};
  var _lastQueryTime = DateTime.now().subtract(const Duration(hours: 1));
  var _queryCount = 0;
  var _exit = false;

  late final mobileWidth = () {
    const minWidth = 320.0;
    final windowWidth = MediaQuery.of(context).size.width;
    var width = windowWidth - 2 * space;
    if (windowWidth > minWidth + 2 * space) {
      final n = (windowWidth / (minWidth + 2 * space)).floor();
      width = windowWidth / n - 2 * space;
    }
    return width;
  }();

  _PeersViewState() {
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
    _queryCount = 0;
  }

  @override
  void onWindowMinimize() {
    _queryCount = _maxQueryCount;
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider<Peers>(
      create: (context) => widget.peers,
      child: Consumer<Peers>(
          builder: (context, peers, child) => peers.peers.isEmpty
              ? Container(
                  margin: EdgeInsets.only(top: kEmptyMarginTop),
                  alignment: Alignment.topCenter,
                  child: Text(translate("Empty")))
              : _buildPeersView(peers)),
    );
  }

  Widget _buildPeersView(Peers peers) {
    final body = ObxValue<RxString>((searchText) {
      return FutureBuilder<List<Peer>>(
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            final peers = snapshot.data!;
            final cards = <Widget>[];
            for (final peer in peers) {
              final visibilityChild = VisibilityDetector(
                key: ValueKey(peer.id),
                onVisibilityChanged: (info) {
                  final peerId = (info.key as ValueKey).value;
                  if (info.visibleFraction > 0.00001) {
                    _curPeers.add(peerId);
                  } else {
                    _curPeers.remove(peerId);
                  }
                  _lastChangeTime = DateTime.now();
                },
                child: widget.peerCardBuilder(peer),
              );
              cards.add(isDesktop
                  ? Obx(
                      () => SizedBox(
                        width: 220,
                        height:
                            peerCardUiType.value == PeerUiType.grid ? 140 : 42,
                        child: visibilityChild,
                      ),
                    )
                  : SizedBox(width: mobileWidth, child: visibilityChild));
            }
            return Wrap(spacing: space, runSpacing: space, children: cards);
          } else {
            return const Center(
              child: CircularProgressIndicator(),
            );
          }
        },
        future: matchPeers(searchText.value, peers.peers),
      );
    }, peerSearchText);

    return body;
  }

  // ignore: todo
  // TODO: variables walk through async tasks?
  void _startCheckOnlines() {
    final queryInterval = const Duration(seconds: 20);
    () async {
      while (!_exit) {
        final now = DateTime.now();
        if (!setEquals(_curPeers, _lastQueryPeers)) {
          if (now.difference(_lastChangeTime) > const Duration(seconds: 1)) {
            if (_curPeers.isNotEmpty) {
              platformFFI.ffiBind
                  .queryOnlines(ids: _curPeers.toList(growable: false));
              _lastQueryPeers = {..._curPeers};
              _lastQueryTime = DateTime.now().subtract(queryInterval);
              _queryCount = 0;
            }
          }
        } else {
          if (_queryCount < _maxQueryCount) {
            if (now.difference(_lastQueryTime) > queryInterval) {
              if (_curPeers.isNotEmpty) {
                platformFFI.ffiBind
                    .queryOnlines(ids: _curPeers.toList(growable: false));
                _lastQueryTime = DateTime.now();
                _queryCount += 1;
              }
            }
          }
        }
        await Future.delayed(const Duration(milliseconds: 300));
      }
    }();
  }

  Future<List<Peer>>? matchPeers(String searchText, List<Peer> peers) async {
    if (widget.peerFilter != null) {
      peers = peers.where((peer) => widget.peerFilter!(peer)).toList();
    }

    searchText = searchText.trim();
    if (searchText.isEmpty) {
      return peers;
    }
    searchText = searchText.toLowerCase();
    final matches =
        await Future.wait(peers.map((peer) => matchPeer(searchText, peer)));
    final filteredList = List<Peer>.empty(growable: true);
    for (var i = 0; i < peers.length; i++) {
      if (matches[i]) {
        filteredList.add(peers[i]);
      }
    }
    return filteredList;
  }
}

abstract class BasePeersView extends StatelessWidget {
  final String name;
  final String loadEvent;
  final PeerFilter? peerFilter;
  final PeerCardBuilder peerCardBuilder;
  final List<Peer> initPeers;

  const BasePeersView({
    Key? key,
    required this.name,
    required this.loadEvent,
    this.peerFilter,
    required this.peerCardBuilder,
    required this.initPeers,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _PeersView(
        peers: Peers(name: name, loadEvent: loadEvent, peers: initPeers),
        peerFilter: peerFilter,
        peerCardBuilder: peerCardBuilder);
  }
}

class RecentPeersView extends BasePeersView {
  RecentPeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          name: 'recent peer',
          loadEvent: 'load_recent_peers',
          peerCardBuilder: (Peer peer) => RecentPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
          initPeers: [],
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadRecentPeers();
    return widget;
  }
}

class FavoritePeersView extends BasePeersView {
  FavoritePeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          name: 'favorite peer',
          loadEvent: 'load_fav_peers',
          peerCardBuilder: (Peer peer) => FavoritePeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
          initPeers: [],
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadFavPeers();
    return widget;
  }
}

class DiscoveredPeersView extends BasePeersView {
  DiscoveredPeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          name: 'discovered peer',
          loadEvent: 'load_lan_peers',
          peerCardBuilder: (Peer peer) => DiscoveredPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
          initPeers: [],
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadLanPeers();
    return widget;
  }
}

class AddressBookPeersView extends BasePeersView {
  AddressBookPeersView(
      {Key? key,
      EdgeInsets? menuPadding,
      ScrollController? scrollController,
      required List<Peer> initPeers})
      : super(
          key: key,
          name: 'address book peer',
          loadEvent: 'load_address_book_peers',
          peerFilter: (Peer peer) =>
              _hitTag(gFFI.abModel.selectedTags, peer.tags),
          peerCardBuilder: (Peer peer) => AddressBookPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
          initPeers: initPeers,
        );

  static bool _hitTag(List<dynamic> selectedTags, List<dynamic> idents) {
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

class MyGroupPeerView extends BasePeersView {
  MyGroupPeerView(
      {Key? key,
      EdgeInsets? menuPadding,
      ScrollController? scrollController,
      required List<Peer> initPeers})
      : super(
          key: key,
          name: 'my group peer',
          loadEvent: 'load_my_group_peers',
          peerCardBuilder: (Peer peer) => MyGroupPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
          initPeers: initPeers,
        );
}

import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
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

class PeerSortType {
  static const String remoteId = 'Remote ID';
  static const String remoteHost = 'Remote Host';
  static const String username = 'Username';
  // static const String status = 'Status';

  static List<String> values = [
    PeerSortType.remoteId,
    PeerSortType.remoteHost,
    PeerSortType.username,
    // PeerSortType.status
  ];
}

class LoadEvent {
  static const String recent = 'load_recent_peers';
  static const String favorite = 'load_fav_peers';
  static const String lan = 'load_lan_peers';
  static const String addressBook = 'load_address_book_peers';
}

/// for peer search text, global obs value
final peerSearchText = "".obs;

/// for peer sort, global obs value
final peerSort = bind.getLocalFlutterConfig(k: 'peer-sorting').obs;

// list for listener
final obslist = [peerSearchText, peerSort].obs;

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
  final HashMap<String, String> _emptyMessages = HashMap.from({
    LoadEvent.recent: 'empty_recent_tip',
    LoadEvent.favorite: 'empty_favorite_tip',
    LoadEvent.lan: 'empty_lan_tip',
    LoadEvent.addressBook: 'empty_address_book_tip',
  });
  final space = isDesktop ? 12.0 : 8.0;
  final _curPeers = <String>{};
  var _lastChangeTime = DateTime.now();
  var _lastQueryPeers = <String>{};
  var _lastQueryTime = DateTime.now().subtract(const Duration(hours: 1));
  var _queryCount = 0;
  var _loaded = false;
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
        builder: (context, peers, child) => peers.peers.isEmpty && _loaded
            ? Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.sentiment_very_dissatisfied_rounded,
                      color: Theme.of(context).tabBarTheme.labelColor,
                      size: 40,
                    ).paddingOnly(bottom: 10),
                    Text(
                      translate(
                        _emptyMessages[widget.peers.loadEvent] ?? 'Empty',
                      ),
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: Theme.of(context).tabBarTheme.labelColor,
                      ),
                    ),
                  ],
                ),
              )
            : _buildPeersView(peers),
      ),
    );
  }

  onVisibilityChanged(VisibilityInfo info) {
    final peerId = _peerId((info.key as ValueKey).value);
    if (info.visibleFraction > 0.00001) {
      _curPeers.add(peerId);
    } else {
      _curPeers.remove(peerId);
    }
    _lastChangeTime = DateTime.now();
  }

  String _cardId(String id) => widget.peers.name + id;
  String _peerId(String cardId) => cardId.replaceAll(widget.peers.name, '');

  Widget _buildPeersView(Peers peers) {
    _loaded = true;
    final body = ObxValue<RxList>((filters) {
      return FutureBuilder<List<Peer>>(
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            final peers = snapshot.data!;
            final cards = <Widget>[];
            for (final peer in peers) {
              final visibilityChild = VisibilityDetector(
                key: ValueKey(_cardId(peer.id)),
                onVisibilityChanged: onVisibilityChanged,
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
        future: matchPeers(filters[0].value, filters[1].value, peers.peers),
      );
    }, obslist);

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
            if (now.difference(_lastQueryTime) >= queryInterval) {
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

  Future<List<Peer>>? matchPeers(
      String searchText, String sortedBy, List<Peer> peers) async {
    if (widget.peerFilter != null) {
      peers = peers.where((peer) => widget.peerFilter!(peer)).toList();
    }

    // fallback to id sorting
    if (!PeerSortType.values.contains(sortedBy)) {
      sortedBy = PeerSortType.remoteId;
      bind.setLocalFlutterConfig(
        k: "peer-sorting",
        v: sortedBy,
      );
    }

    if (widget.peers.loadEvent != LoadEvent.recent) {
      switch (sortedBy) {
        case PeerSortType.remoteId:
          peers.sort((p1, p2) => p1.getId().compareTo(p2.getId()));
          break;
        case PeerSortType.remoteHost:
          peers.sort((p1, p2) =>
              p1.hostname.toLowerCase().compareTo(p2.hostname.toLowerCase()));
          break;
        case PeerSortType.username:
          peers.sort((p1, p2) =>
              p1.username.toLowerCase().compareTo(p2.username.toLowerCase()));
          break;
        // case PeerSortType.status:
        // peers.sort((p1, p2) => p1.online ? -1 : 1);
        // break;
      }
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
          loadEvent: LoadEvent.recent,
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
          loadEvent: LoadEvent.favorite,
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
          loadEvent: LoadEvent.lan,
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
          loadEvent: LoadEvent.addressBook,
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

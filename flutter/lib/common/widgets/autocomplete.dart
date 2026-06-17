import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import '../../../models/platform_model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';

@visibleForTesting
List<Peer> mergeAutocompletePeers({
  Iterable<Peer> addressBookPeers = const [],
  Iterable<Peer> groupPeers = const [],
  Iterable<Peer> lanPeers = const [],
  Iterable<Peer> recentPeers = const [],
  Iterable<String> restRecentPeerIds = const [],
}) {
  final combinedPeers = <String, Peer>{};

  void addPeer(Peer peer) {
    if (peer.id.isEmpty) {
      return;
    }
    final existingPeer = combinedPeers[peer.id];
    if (existingPeer == null) {
      combinedPeers[peer.id] = Peer.copy(peer);
    } else if (peer.online) {
      existingPeer.online = true;
    }
  }

  for (final peer in addressBookPeers) {
    addPeer(peer);
  }
  for (final peer in groupPeers) {
    addPeer(peer);
  }
  for (final peer in lanPeers) {
    addPeer(peer);
  }
  for (final peer in recentPeers) {
    addPeer(peer);
  }
  for (final id in restRecentPeerIds) {
    if (id.isNotEmpty && !combinedPeers.containsKey(id)) {
      combinedPeers[id] = Peer.fromJson({'id': id});
    }
  }

  return combinedPeers.values.toList(growable: false);
}

@visibleForTesting
bool updateAutocompletePeerOnlineStates(
  List<Peer> peers, {
  required Set<String> onlines,
  required Set<String> offlines,
}) {
  var changed = false;
  for (final peer in peers) {
    if (onlines.contains(peer.id)) {
      if (!peer.online) {
        peer.online = true;
        changed = true;
      }
    } else if (offlines.contains(peer.id)) {
      if (peer.online) {
        peer.online = false;
        changed = true;
      }
    }
  }
  return changed;
}

@visibleForTesting
List<String> autocompleteOnlineQueryIds(
  Iterable<Peer> options, {
  required int limit,
}) {
  final ids = <String>[];
  final seenIds = <String>{};
  for (final peer in options) {
    if (peer.id.isEmpty || seenIds.contains(peer.id)) {
      continue;
    }
    seenIds.add(peer.id);
    ids.add(peer.id);
    if (ids.length >= limit) {
      break;
    }
  }
  return ids;
}

class AllPeersLoader {
  List<Peer> peers = [];

  bool _isPeersLoading = false;
  bool _isPeersLoaded = false;
  Set<String> _lastQueryOnlineIds = {};
  DateTime _lastQueryOnlineTime = DateTime.fromMillisecondsSinceEpoch(0);
  Timer? _queryOnlineTimer;
  List<Peer> _lastQueryOnlineOptions = const [];
  Set<String> _lastOnlineIds = {};
  Set<String> _lastOfflineIds = {};
  final Future<void> Function(List<String> ids) _queryOnlines;
  final Duration _queryOnlineDebounce;
  void Function(VoidCallback)? _setState;
  bool _isCleared = false;

  final String _listenerKey = 'AllPeersLoader';
  static const String _cbQueryOnlines = 'callback_query_onlines';
  static const Duration _queryOnlineInterval = Duration(seconds: 5);
  static const Duration _defaultQueryOnlineDebounce =
      Duration(milliseconds: 300);
  static const int _maxQueryOnlineOptions = 20;

  bool get needLoad => !_isPeersLoaded && !_isPeersLoading;
  bool get isPeersLoaded => _isPeersLoaded;

  AllPeersLoader({
    @visibleForTesting Future<void> Function(List<String> ids)? queryOnlines,
    @visibleForTesting Duration? queryOnlineDebounce,
  })  : _queryOnlines = queryOnlines ?? ((ids) => bind.queryOnlines(ids: ids)),
        _queryOnlineDebounce =
            queryOnlineDebounce ?? _defaultQueryOnlineDebounce;

  void init(void Function(VoidCallback) setState) {
    _setState = setState;
    _isCleared = false;
    gFFI.recentPeersModel.addListener(_mergeAllPeers);
    gFFI.lanPeersModel.addListener(_mergeAllPeers);
    gFFI.abModel.addPeerUpdateListener(_listenerKey, _mergeAllPeers);
    gFFI.groupModel.addPeerUpdateListener(_listenerKey, _mergeAllPeers);
    platformFFI.registerEventHandler(_cbQueryOnlines, _listenerKey,
        (evt) async {
      _updateOnlineState(evt);
    });
  }

  void clear() {
    gFFI.recentPeersModel.removeListener(_mergeAllPeers);
    gFFI.lanPeersModel.removeListener(_mergeAllPeers);
    gFFI.abModel.removePeerUpdateListener(_listenerKey);
    gFFI.groupModel.removePeerUpdateListener(_listenerKey);
    platformFFI.unregisterEventHandler(_cbQueryOnlines, _listenerKey);
    _queryOnlineTimer?.cancel();
    _lastQueryOnlineOptions = const [];
    _setState = null;
    _isCleared = true;
  }

  Future<void> getAllPeers() async {
    if (!needLoad) {
      return;
    }
    _isPeersLoading = true;

    if (gFFI.recentPeersModel.peers.isEmpty) {
      bind.mainLoadRecentPeers();
    }
    if (gFFI.lanPeersModel.peers.isEmpty) {
      bind.mainLoadLanPeers();
    }
    // No need to care about peers from abModel, and group model.
    // Because they will pull data in `refreshCurrentUser()` on startup.

    final startTime = DateTime.now();
    _mergeAllPeers();
    final diffTime = DateTime.now().difference(startTime).inMilliseconds;
    if (diffTime < 100) {
      await Future.delayed(Duration(milliseconds: diffTime));
    }
  }

  void _mergeAllPeers() {
    if (_isCleared) {
      return;
    }
    peers = mergeAutocompletePeers(
      addressBookPeers: gFFI.abModel.allPeers(),
      groupPeers: gFFI.groupModel.peers,
      lanPeers: gFFI.lanPeersModel.peers,
      recentPeers: gFFI.recentPeersModel.peers,
      restRecentPeerIds: gFFI.recentPeersModel.restPeerIds,
    );
    _applyLastOnlineState(peers);
    _scheduleSetState(() {
      _isPeersLoading = false;
      _isPeersLoaded = true;
    });
  }

  void _updateOnlineState(Map<String, dynamic> evt) {
    if (_isCleared) {
      return;
    }
    _lastOnlineIds = _splitPeerIds(evt['onlines']);
    _lastOfflineIds = _splitPeerIds(evt['offlines']);
    final peersChanged = _applyLastOnlineState(peers);
    final optionsChanged = _applyLastOnlineState(_lastQueryOnlineOptions);
    if (peersChanged || optionsChanged) {
      _scheduleSetState(() {});
    }
  }

  void _scheduleSetState(VoidCallback callback) {
    if (_isCleared) {
      return;
    }
    final setState = _setState;
    if (setState == null) {
      callback();
    } else {
      setState(callback);
    }
  }

  bool _applyLastOnlineState(List<Peer> peers) {
    return updateAutocompletePeerOnlineStates(
      peers,
      onlines: _lastOnlineIds,
      offlines: _lastOfflineIds,
    );
  }

  Set<String> _splitPeerIds(dynamic ids) {
    if (ids is! String || ids.isEmpty) {
      return {};
    }
    return ids.split(',').where((id) => id.isNotEmpty).toSet();
  }

  void queryOnlines(Iterable<Peer> options) {
    if (_isCleared) {
      return;
    }
    _lastQueryOnlineOptions = options.toList(growable: false);
    final ids = autocompleteOnlineQueryIds(
      _lastQueryOnlineOptions,
      limit: _maxQueryOnlineOptions,
    ).toSet();
    _queryOnlineTimer?.cancel();
    _queryOnlineTimer = null;
    if (ids.isEmpty) {
      return;
    }
    final now = DateTime.now();
    if (setEquals(ids, _lastQueryOnlineIds) &&
        now.difference(_lastQueryOnlineTime) < _queryOnlineInterval) {
      return;
    }

    _queryOnlineTimer = Timer(_queryOnlineDebounce, () async {
      try {
        await _queryOnlines(ids.toList(growable: false));
        if (_isCleared) {
          return;
        }
        _lastQueryOnlineIds = ids;
        _lastQueryOnlineTime = DateTime.now();
      } catch (e) {
        debugPrint('query autocomplete online state failed: $e');
      }
    });
  }

  @visibleForTesting
  void updateOnlineStateForTesting(Map<String, dynamic> evt) {
    _updateOnlineState(evt);
  }

  @visibleForTesting
  bool applyLastOnlineStateForTesting(List<Peer> peers) {
    return _applyLastOnlineState(peers);
  }
}

class AutocompletePeerTile extends StatefulWidget {
  final VoidCallback onSelect;
  final Peer peer;

  const AutocompletePeerTile({
    Key? key,
    required this.onSelect,
    required this.peer,
  }) : super(key: key);

  @override
  AutocompletePeerTileState createState() => AutocompletePeerTileState();
}

class AutocompletePeerTileState extends State<AutocompletePeerTile> {
  List _frontN<T>(List list, int n) {
    if (list.length <= n) {
      return list;
    } else {
      return list.sublist(0, n);
    }
  }

  @override
  Widget build(BuildContext context) {
    final double tileRadius = 5;
    final name =
        '${widget.peer.username}${widget.peer.username.isNotEmpty && widget.peer.hostname.isNotEmpty ? '@' : ''}${widget.peer.hostname}';
    final greyStyle = TextStyle(
        fontSize: 11,
        color: Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6));
    final child = GestureDetector(
        onTap: () => widget.onSelect(),
        child: Padding(
            padding: EdgeInsets.only(left: 5, right: 5),
            child: Container(
                height: 42,
                margin: EdgeInsets.only(bottom: 5),
                child: Row(
                  mainAxisSize: MainAxisSize.max,
                  children: [
                    Container(
                        decoration: BoxDecoration(
                          color: str2color(
                              '${widget.peer.id}${widget.peer.platform}', 0x7f),
                          borderRadius: BorderRadius.only(
                            topLeft: Radius.circular(tileRadius),
                            bottomLeft: Radius.circular(tileRadius),
                          ),
                        ),
                        alignment: Alignment.center,
                        width: 42,
                        height: null,
                        child: Padding(
                            padding: EdgeInsets.all(6),
                            child: getPlatformImage(widget.peer.platform,
                                size: 30))),
                    Expanded(
                      child: Container(
                          padding: EdgeInsets.only(left: 10),
                          decoration: BoxDecoration(
                            color: Theme.of(context).colorScheme.background,
                            borderRadius: BorderRadius.only(
                              topRight: Radius.circular(tileRadius),
                              bottomRight: Radius.circular(tileRadius),
                            ),
                          ),
                          child: Row(
                            children: [
                              Expanded(
                                  child: Container(
                                      margin: EdgeInsets.only(top: 2),
                                      child: Container(
                                          margin: EdgeInsets.only(top: 2),
                                          child: Column(
                                            children: [
                                              Container(
                                                  margin:
                                                      EdgeInsets.only(top: 2),
                                                  child: Row(children: [
                                                    getOnline(
                                                        8, widget.peer.online),
                                                    Expanded(
                                                        child: Text(
                                                      widget.peer.alias.isEmpty
                                                          ? formatID(
                                                              widget.peer.id)
                                                          : widget.peer.alias,
                                                      overflow:
                                                          TextOverflow.ellipsis,
                                                      style: Theme.of(context)
                                                          .textTheme
                                                          .titleSmall,
                                                    )),
                                                    widget.peer.alias.isNotEmpty
                                                        ? Padding(
                                                            padding:
                                                                const EdgeInsets
                                                                    .only(
                                                                    left: 5,
                                                                    right: 5),
                                                            child: Text(
                                                              "(${widget.peer.id})",
                                                              style: greyStyle,
                                                              overflow:
                                                                  TextOverflow
                                                                      .ellipsis,
                                                            ))
                                                        : Container(),
                                                  ])),
                                              Align(
                                                alignment: Alignment.centerLeft,
                                                child: Text(
                                                  name,
                                                  style: greyStyle,
                                                  textAlign: TextAlign.start,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                ),
                                              ),
                                            ],
                                          )))),
                            ],
                          )),
                    )
                  ],
                ))));
    final colors = _frontN(widget.peer.tags, 25)
        .map((e) => gFFI.abModel.getCurrentAbTagColor(e))
        .toList();
    return Tooltip(
      message: !(isDesktop || isWebDesktop)
          ? ''
          : widget.peer.tags.isNotEmpty
              ? '${translate('Tags')}: ${widget.peer.tags.join(', ')}'
              : '',
      child: Stack(children: [
        child,
        if (colors.isNotEmpty)
          Positioned(
            top: 5,
            right: 10,
            child: CustomPaint(
              painter: TagPainter(radius: 3, colors: colors),
            ),
          )
      ]),
    );
  }
}

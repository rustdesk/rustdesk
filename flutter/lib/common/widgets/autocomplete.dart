import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import '../../../models/platform_model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';

class AllPeersLoader {
  List<Peer> peers = [];
  bool hasMoreRecentPeers = false;

  bool isPeersLoading = false;
  bool _isPartialPeersLoaded = false;
  bool _isPeersLoaded = false;

  AllPeersLoader();

  bool get isLoaded => _isPartialPeersLoaded || _isPeersLoaded;

  void reset() {
    peers.clear();
    hasMoreRecentPeers = false;
    _isPartialPeersLoaded = false;
    _isPeersLoaded = false;
  }

  Future<void> getAllPeers(void Function(VoidCallback) setState) async {
    if (isPeersLoading) {
      return;
    }
    reset();
    isPeersLoading = true;

    final startTime = DateTime.now();
    await _getAllPeers(false);
    if (!hasMoreRecentPeers) {
      final diffTime = DateTime.now().difference(startTime).inMilliseconds;
      if (diffTime < 100) {
        await Future.delayed(Duration(milliseconds: diffTime));
      }
      setState(() {
        isPeersLoading = false;
        _isPeersLoaded = true;
      });
    } else {
      setState(() {
        _isPartialPeersLoaded = true;
      });
      await _getAllPeers(true);
      setState(() {
        isPeersLoading = false;
        _isPeersLoaded = true;
      });
    }
  }

  Future<void> _getAllPeers(bool getAllRecentPeers) async {
    Map<String, dynamic> recentPeers =
        jsonDecode(await bind.mainGetRecentPeers(getAll: getAllRecentPeers));
    Map<String, dynamic> lanPeers = jsonDecode(bind.mainLoadLanPeersSync());
    Map<String, dynamic> combinedPeers = {};

    void mergePeers(Map<String, dynamic> peers) {
      if (peers.containsKey("peers")) {
        dynamic peerData = peers["peers"];

        if (peerData is String) {
          try {
            peerData = jsonDecode(peerData);
          } catch (e) {
            print("Error decoding peers: $e");
            return;
          }
        }

        if (peerData is List) {
          for (var peer in peerData) {
            if (peer is Map && peer.containsKey("id")) {
              String id = peer["id"];
              if (!combinedPeers.containsKey(id)) {
                combinedPeers[id] = peer;
              }
            }
          }
        }
      }
    }

    mergePeers(recentPeers);
    mergePeers(lanPeers);
    for (var p in gFFI.abModel.allPeers()) {
      if (!combinedPeers.containsKey(p.id)) {
        combinedPeers[p.id] = p.toJson();
      }
    }
    for (var p in gFFI.groupModel.peers.map((e) => Peer.copy(e)).toList()) {
      if (!combinedPeers.containsKey(p.id)) {
        combinedPeers[p.id] = p.toJson();
      }
    }

    List<Peer> parsedPeers = [];

    for (var peer in combinedPeers.values) {
      parsedPeers.add(Peer.fromJson(peer));
    }

    try {
      final List<dynamic> moreRecentPeerIds =
          jsonDecode(recentPeers["ids"] ?? '[]');
      hasMoreRecentPeers = false;
      for (final id in moreRecentPeerIds) {
        final sid = id.toString();
        if (!parsedPeers.any((element) => element.id == sid)) {
          parsedPeers.add(Peer.fromJson({'id': sid}));
          hasMoreRecentPeers = true;
        }
      }
    } catch (e) {
      debugPrint("Error parsing more peer ids: $e");
    }

    peers = parsedPeers;
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

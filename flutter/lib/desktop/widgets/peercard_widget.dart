import 'package:flutter/material.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:contextmenu/contextmenu.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/peer_model.dart';

typedef PopupMenuItemsFunc = Future<List<PopupMenuItem<String>>> Function();

class _PeerCard extends StatefulWidget {
  final Peer peer;
  final PopupMenuItemsFunc popupMenuItemsFunc;

  _PeerCard({required this.peer, required this.popupMenuItemsFunc, Key? key})
      : super(key: key);

  @override
  _PeerCardState createState() => _PeerCardState();
}

/// State for the connection page.
class _PeerCardState extends State<_PeerCard> {
  var _menuPos;

  @override
  Widget build(BuildContext context) {
    final peer = super.widget.peer;
    var deco = Rx<BoxDecoration?>(BoxDecoration(
        border: Border.all(color: Colors.transparent, width: 1.0),
        borderRadius: BorderRadius.circular(20)));
    return Card(
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
        child: MouseRegion(
          onEnter: (evt) {
            deco.value = BoxDecoration(
                border: Border.all(color: Colors.blue, width: 1.0),
                borderRadius: BorderRadius.circular(20));
          },
          onExit: (evt) {
            deco.value = BoxDecoration(
                border: Border.all(color: Colors.transparent, width: 1.0),
                borderRadius: BorderRadius.circular(20));
          },
          child: _buildPeerTile(context, peer, deco),
        ));
  }

  Widget _buildPeerTile(
      BuildContext context, Peer peer, Rx<BoxDecoration?> deco) {
    return Obx(
      () => Container(
        decoration: deco.value,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Expanded(
              child: Container(
                decoration: BoxDecoration(
                  color: str2color('${peer.id}${peer.platform}', 0x7f),
                  borderRadius: BorderRadius.only(
                    topLeft: Radius.circular(20),
                    topRight: Radius.circular(20),
                  ),
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: [
                          Container(
                            padding: const EdgeInsets.all(6),
                            child: _getPlatformImage('${peer.platform}'),
                          ),
                          Row(
                            children: [
                              Expanded(
                                child: Tooltip(
                                  message: '${peer.username}@${peer.hostname}',
                                  child: Text(
                                    '${peer.username}@${peer.hostname}',
                                    style: TextStyle(
                                        color: Colors.white70, fontSize: 12),
                                    textAlign: TextAlign.center,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ).paddingAll(4.0),
                    ),
                  ],
                ),
              ),
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(children: [
                  Padding(
                      padding: EdgeInsets.fromLTRB(0, 4, 8, 4),
                      child: CircleAvatar(
                          radius: 5,
                          backgroundColor:
                              peer.online ? Colors.green : Colors.yellow)),
                  Text('${peer.id}')
                ]),
                InkWell(
                    child: Icon(Icons.more_vert),
                    onTapDown: (e) {
                      final x = e.globalPosition.dx;
                      final y = e.globalPosition.dy;
                      _menuPos = RelativeRect.fromLTRB(x, y, x, y);
                    },
                    onTap: () {
                      _showPeerMenu(context, peer.id);
                    }),
              ],
            ).paddingSymmetric(vertical: 8.0, horizontal: 12.0)
          ],
        ),
      ),
    );
  }

  /// Connect to a peer with [id].
  /// If [isFileTransfer], starts a session only for file transfer.
  void _connect(String id, {bool isFileTransfer = false}) async {
    if (id == '') return;
    id = id.replaceAll(' ', '');
    if (isFileTransfer) {
      await rustDeskWinManager.new_file_transfer(id);
    } else {
      await rustDeskWinManager.new_remote_desktop(id);
    }
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  /// Show the peer menu and handle user's choice.
  /// User might remove the peer or send a file to the peer.
  void _showPeerMenu(BuildContext context, String id) async {
    var value = await showMenu(
      context: context,
      position: this._menuPos,
      items: await super.widget.popupMenuItemsFunc(),
      elevation: 8,
    );
    if (value == 'remove') {
      setState(() => gFFI.setByName('remove', '$id'));
      () async {
        removePreference(id);
      }();
    } else if (value == 'file') {
      _connect(id, isFileTransfer: true);
    } else if (value == 'add-fav') {
    } else if (value == 'connect') {
      _connect(id, isFileTransfer: false);
    } else if (value == 'ab-delete') {
      gFFI.abModel.deletePeer(id);
      await gFFI.abModel.updateAb();
      setState(() {});
    } else if (value == 'ab-edit-tag') {
      _abEditTag(id);
    }
  }

  Widget _buildTag(String tagName, RxList<dynamic> rxTags,
      {Function()? onTap}) {
    return ContextMenuArea(
      width: 100,
      builder: (context) => [
        ListTile(
          title: Text(translate("Delete")),
          onTap: () {
            gFFI.abModel.deleteTag(tagName);
            gFFI.abModel.updateAb();
            Future.delayed(Duration.zero, () => Get.back());
          },
        )
      ],
      child: GestureDetector(
        onTap: onTap,
        child: Obx(
          () => Container(
            decoration: BoxDecoration(
                color: rxTags.contains(tagName) ? Colors.blue : null,
                border: Border.all(color: MyTheme.darkGray),
                borderRadius: BorderRadius.circular(10)),
            margin: EdgeInsets.symmetric(horizontal: 4.0, vertical: 8.0),
            padding: EdgeInsets.symmetric(vertical: 2.0, horizontal: 8.0),
            child: Text(
              tagName,
              style: TextStyle(
                  color: rxTags.contains(tagName) ? MyTheme.white : null),
            ),
          ),
        ),
      ),
    );
  }

  /// Get the image for the current [platform].
  Widget _getPlatformImage(String platform) {
    platform = platform.toLowerCase();
    if (platform == 'mac os')
      platform = 'mac';
    else if (platform != 'linux' && platform != 'android') platform = 'win';
    return Image.asset('assets/$platform.png', height: 50);
  }

  void _abEditTag(String id) {
    var isInProgress = false;

    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = gFFI.abModel.getPeerTags(id).obs;

    DialogManager.show((setState, close) {
      return CustomAlertDialog(
        title: Text(translate("Edit Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
              child: Wrap(
                children: tags
                    .map((e) => _buildTag(e, selectedTag, onTap: () {
                          if (selectedTag.contains(e)) {
                            selectedTag.remove(e);
                          } else {
                            selectedTag.add(e);
                          }
                        }))
                    .toList(growable: false),
              ),
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(
              onPressed: () {
                close();
              },
              child: Text(translate("Cancel"))),
          TextButton(
              onPressed: () async {
                setState(() {
                  isInProgress = true;
                });
                gFFI.abModel.changeTagForPeer(id, selectedTag);
                await gFFI.abModel.updateAb();
                close();
              },
              child: Text(translate("OK"))),
        ],
      );
    });
  }
}

abstract class BasePeerCard extends StatelessWidget {
  final Peer peer;
  BasePeerCard({required this.peer, Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _PeerCard(peer: peer, popupMenuItemsFunc: _getPopupMenuItems);
  }

  @protected
  Future<List<PopupMenuItem<String>>> _getPopupMenuItems();
}

class RecentPeerCard extends BasePeerCard {
  RecentPeerCard({required Peer peer, Key? key}) : super(peer: peer, key: key);

  Future<List<PopupMenuItem<String>>> _getPopupMenuItems() async {
    debugPrint("call RecentPeerCard _getPopupMenuItems");
    return [
      PopupMenuItem<String>(
          child: Text(translate('Connect')), value: 'connect'),
      PopupMenuItem<String>(
          child: Text(translate('Transfer File')), value: 'file'),
      PopupMenuItem<String>(
          child: Text(translate('TCP Tunneling')), value: 'tcp-tunnel'),
      PopupMenuItem<String>(child: Text(translate('Rename')), value: 'rename'),
      PopupMenuItem<String>(child: Text(translate('Remove')), value: 'remove'),
      PopupMenuItem<String>(
          child: Text(translate('Unremember Password')),
          value: 'unremember-password'),
      PopupMenuItem<String>(
          child: Text(translate('Edit Tag')), value: 'ab-edit-tag'),
    ];
  }
}

class FavoritePeerCard extends BasePeerCard {
  FavoritePeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  Future<List<PopupMenuItem<String>>> _getPopupMenuItems() async {
    debugPrint("call FavoritePeerCard _getPopupMenuItems");
    return [
      PopupMenuItem<String>(
          child: Text(translate('Connect')), value: 'connect'),
      PopupMenuItem<String>(
          child: Text(translate('Transfer File')), value: 'file'),
      PopupMenuItem<String>(
          child: Text(translate('TCP Tunneling')), value: 'tcp-tunnel'),
      PopupMenuItem<String>(child: Text(translate('Rename')), value: 'rename'),
      PopupMenuItem<String>(child: Text(translate('Remove')), value: 'remove'),
      PopupMenuItem<String>(
          child: Text(translate('Unremember Password')),
          value: 'unremember-password'),
      PopupMenuItem<String>(
          child: Text(translate('Remove from Favorites')), value: 'remove-fav'),
    ];
  }
}

class DiscoveredPeerCard extends BasePeerCard {
  DiscoveredPeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  Future<List<PopupMenuItem<String>>> _getPopupMenuItems() async {
    debugPrint("call DiscoveredPeerCard _getPopupMenuItems");
    return [
      PopupMenuItem<String>(
          child: Text(translate('Connect')), value: 'connect'),
      PopupMenuItem<String>(
          child: Text(translate('Transfer File')), value: 'file'),
      PopupMenuItem<String>(
          child: Text(translate('TCP Tunneling')), value: 'tcp-tunnel'),
      PopupMenuItem<String>(child: Text(translate('Rename')), value: 'rename'),
      PopupMenuItem<String>(child: Text(translate('Remove')), value: 'remove'),
      PopupMenuItem<String>(
          child: Text(translate('Unremember Password')),
          value: 'unremember-password'),
      PopupMenuItem<String>(
          child: Text(translate('Edit Tag')), value: 'ab-edit-tag'),
    ];
  }
}

class AddressBookPeerCard extends BasePeerCard {
  AddressBookPeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  Future<List<PopupMenuItem<String>>> _getPopupMenuItems() async {
    debugPrint("call AddressBookPeerCard _getPopupMenuItems");
    return [
      PopupMenuItem<String>(
          child: Text(translate('Connect')), value: 'connect'),
      PopupMenuItem<String>(
          child: Text(translate('Transfer File')), value: 'file'),
      PopupMenuItem<String>(
          child: Text(translate('TCP Tunneling')), value: 'tcp-tunnel'),
      PopupMenuItem<String>(child: Text(translate('Rename')), value: 'rename'),
      PopupMenuItem<String>(
          child: Text(translate('Remove')), value: 'ab-delete'),
      PopupMenuItem<String>(
          child: Text(translate('Unremember Password')),
          value: 'unremember-password'),
      PopupMenuItem<String>(
          child: Text(translate('Add to Favorites')), value: 'add-fav'),
    ];
  }
}

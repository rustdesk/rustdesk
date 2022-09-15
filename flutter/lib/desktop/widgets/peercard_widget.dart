import 'package:contextmenu/contextmenu.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../models/model.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import './material_mod_popup_menu.dart' as mod_menu;
import './popup_menu.dart';
import './utils.dart';

class _PopupMenuTheme {
  static const Color commonColor = MyTheme.accent;
  // kMinInteractiveDimension
  static const double height = 25.0;
  static const double dividerHeight = 3.0;
}

typedef PopupMenuEntryBuilder = Future<List<mod_menu.PopupMenuEntry<String>>>
    Function(BuildContext);

enum PeerUiType { grid, list }

final peerCardUiType = PeerUiType.grid.obs;

class _PeerCard extends StatefulWidget {
  final Peer peer;
  final RxString alias;
  final Function(BuildContext, String) connect;
  final PopupMenuEntryBuilder popupMenuEntryBuilder;

  const _PeerCard(
      {required this.peer,
      required this.alias,
      required this.connect,
      required this.popupMenuEntryBuilder,
      Key? key})
      : super(key: key);

  @override
  _PeerCardState createState() => _PeerCardState();
}

/// State for the connection page.
class _PeerCardState extends State<_PeerCard>
    with AutomaticKeepAliveClientMixin {
  var _menuPos = RelativeRect.fill;
  final double _cardRadis = 16;
  final double _borderWidth = 2;
  final RxBool _iconMoreHover = false.obs;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final peer = super.widget.peer;
    var deco = Rx<BoxDecoration?>(BoxDecoration(
        border: Border.all(color: Colors.transparent, width: _borderWidth),
        borderRadius: peerCardUiType.value == PeerUiType.grid
            ? BorderRadius.circular(_cardRadis)
            : null));
    return MouseRegion(
      onEnter: (evt) {
        deco.value = BoxDecoration(
            border: Border.all(color: MyTheme.button, width: _borderWidth),
            borderRadius: peerCardUiType.value == PeerUiType.grid
                ? BorderRadius.circular(_cardRadis)
                : null);
      },
      onExit: (evt) {
        deco.value = BoxDecoration(
            border: Border.all(color: Colors.transparent, width: _borderWidth),
            borderRadius: peerCardUiType.value == PeerUiType.grid
                ? BorderRadius.circular(_cardRadis)
                : null);
      },
      child: GestureDetector(
          onDoubleTap: () => widget.connect(context, peer.id),
          child: Obx(() => peerCardUiType.value == PeerUiType.grid
              ? _buildPeerCard(context, peer, deco)
              : _buildPeerTile(context, peer, deco))),
    );
  }

  Widget _buildPeerTile(
      BuildContext context, Peer peer, Rx<BoxDecoration?> deco) {
    final greyStyle =
        TextStyle(fontSize: 12, color: MyTheme.color(context).lighterText);
    return Obx(
      () => Container(
        foregroundDecoration: deco.value,
        child: Row(
          mainAxisSize: MainAxisSize.max,
          children: [
            Container(
              decoration: BoxDecoration(
                color: str2color('${peer.id}${peer.platform}', 0x7f),
              ),
              alignment: Alignment.center,
              child: getPlatformImage(peer.platform, size: 30).paddingAll(6),
            ),
            Expanded(
              child: Container(
                decoration: BoxDecoration(color: MyTheme.color(context).bg),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.spaceAround,
                        children: [
                          Row(children: [
                            Padding(
                                padding: const EdgeInsets.fromLTRB(0, 4, 4, 4),
                                child: CircleAvatar(
                                    radius: 5,
                                    backgroundColor: peer.online
                                        ? Colors.green
                                        : Colors.yellow)),
                            Text(
                              formatID(peer.id),
                              style:
                                  const TextStyle(fontWeight: FontWeight.w400),
                            ),
                          ]),
                          Align(
                            alignment: Alignment.centerLeft,
                            child: FutureBuilder<String>(
                              future: bind.mainGetPeerOption(
                                  id: peer.id, key: 'alias'),
                              builder: (_, snapshot) {
                                if (snapshot.hasData) {
                                  final name = snapshot.data!.isEmpty
                                      ? '${peer.username}@${peer.hostname}'
                                      : snapshot.data!;
                                  return Tooltip(
                                    message: name,
                                    waitDuration: const Duration(seconds: 1),
                                    child: Text(
                                      name,
                                      style: greyStyle,
                                      textAlign: TextAlign.start,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  );
                                } else {
                                  // alias has not arrived
                                  return Text(
                                    '${peer.username}@${peer.hostname}',
                                    style: greyStyle,
                                    textAlign: TextAlign.start,
                                    overflow: TextOverflow.ellipsis,
                                  );
                                }
                              },
                            ),
                          ),
                        ],
                      ),
                    ),
                    _actionMore(peer),
                  ],
                ).paddingSymmetric(horizontal: 4.0),
              ),
            )
          ],
        ),
      ),
    );
  }

  Widget _buildPeerCard(
      BuildContext context, Peer peer, Rx<BoxDecoration?> deco) {
    return Card(
      color: Colors.transparent,
      elevation: 0,
      margin: EdgeInsets.zero,
      child: Obx(
        () => Container(
          foregroundDecoration: deco.value,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(_cardRadis - _borderWidth),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Expanded(
                  child: Container(
                    color: str2color('${peer.id}${peer.platform}', 0x7f),
                    child: Row(
                      children: [
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.center,
                            children: [
                              Container(
                                padding: const EdgeInsets.all(6),
                                child:
                                    getPlatformImage(peer.platform, size: 60),
                              ),
                              Row(
                                children: [
                                  Expanded(
                                    child: Obx(() {
                                      final name = widget.alias.value.isEmpty
                                          ? '${peer.username}@${peer.hostname}'
                                          : widget.alias.value;
                                      return Tooltip(
                                        message: name,
                                        waitDuration:
                                            const Duration(seconds: 1),
                                        child: Text(
                                          name,
                                          style: const TextStyle(
                                              color: Colors.white70,
                                              fontSize: 12),
                                          textAlign: TextAlign.center,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      );
                                    }),
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
                Container(
                  color: MyTheme.color(context).bg,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Row(children: [
                        Padding(
                            padding: const EdgeInsets.fromLTRB(0, 4, 8, 4),
                            child: CircleAvatar(
                                radius: 5,
                                backgroundColor: peer.online
                                    ? Colors.green
                                    : Colors.yellow)),
                        Text(formatID(peer.id))
                      ]).paddingSymmetric(vertical: 8),
                      _actionMore(peer),
                    ],
                  ).paddingSymmetric(horizontal: 12.0),
                )
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _actionMore(Peer peer) => Listener(
      onPointerDown: (e) {
        final x = e.position.dx;
        final y = e.position.dy;
        _menuPos = RelativeRect.fromLTRB(x, y, x, y);
      },
      onPointerUp: (_) => _showPeerMenu(context, peer.id),
      child: MouseRegion(
          onEnter: (_) => _iconMoreHover.value = true,
          onExit: (_) => _iconMoreHover.value = false,
          child: CircleAvatar(
              radius: 14,
              backgroundColor: _iconMoreHover.value
                  ? MyTheme.color(context).grayBg!
                  : MyTheme.color(context).bg!,
              child: Icon(Icons.more_vert,
                  size: 18,
                  color: _iconMoreHover.value
                      ? MyTheme.color(context).text
                      : MyTheme.color(context).lightText))));

  /// Show the peer menu and handle user's choice.
  /// User might remove the peer or send a file to the peer.
  void _showPeerMenu(BuildContext context, String id) async {
    await mod_menu.showMenu(
      context: context,
      position: _menuPos,
      items: await super.widget.popupMenuEntryBuilder(context),
      elevation: 8,
    );
  }

  @override
  bool get wantKeepAlive => true;
}

abstract class BasePeerCard extends StatelessWidget {
  final RxString alias = ''.obs;
  final Peer peer;

  BasePeerCard({required this.peer, Key? key}) : super(key: key) {
    bind
        .mainGetPeerOption(id: peer.id, key: 'alias')
        .then((value) => alias.value = value);
  }

  @override
  Widget build(BuildContext context) {
    return _PeerCard(
      peer: peer,
      alias: alias,
      connect: (BuildContext context, String id) => connect(context, id),
      popupMenuEntryBuilder: _buildPopupMenuEntry,
    );
  }

  Future<List<mod_menu.PopupMenuEntry<String>>> _buildPopupMenuEntry(
          BuildContext context) async =>
      (await _buildMenuItems(context))
          .map((e) => e.build(
              context,
              const MenuConfig(
                  commonColor: _PopupMenuTheme.commonColor,
                  height: _PopupMenuTheme.height,
                  dividerHeight: _PopupMenuTheme.dividerHeight)))
          .expand((i) => i)
          .toList();

  @protected
  Future<List<MenuEntryBase<String>>> _buildMenuItems(BuildContext context);

  MenuEntryBase<String> _connectCommonAction(
      BuildContext context, String id, String title,
      {bool isFileTransfer = false,
      bool isTcpTunneling = false,
      bool isRDP = false}) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate(title),
        style: style,
      ),
      proc: () {
        connect(
          context,
          peer.id,
          isFileTransfer: isFileTransfer,
          isTcpTunneling: isTcpTunneling,
          isRDP: isRDP,
        );
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _connectAction(BuildContext context, String id) {
    return _connectCommonAction(context, id, 'Connect');
  }

  @protected
  MenuEntryBase<String> _transferFileAction(BuildContext context, String id) {
    return _connectCommonAction(
      context,
      id,
      'Transfer File',
      isFileTransfer: true,
    );
  }

  @protected
  MenuEntryBase<String> _tcpTunnelingAction(BuildContext context, String id) {
    return _connectCommonAction(
      context,
      id,
      'TCP Tunneling',
      isTcpTunneling: true,
    );
  }

  @protected
  MenuEntryBase<String> _rdpAction(BuildContext context, String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Container(
          alignment: AlignmentDirectional.center,
          height: _PopupMenuTheme.height,
          child: Row(
            children: [
              Text(
                translate('RDP'),
                style: style,
              ),
              Expanded(
                  child: Align(
                alignment: Alignment.centerRight,
                child: IconButton(
                  padding: EdgeInsets.zero,
                  icon: const Icon(Icons.edit),
                  onPressed: () => _rdpDialog(id),
                ),
              ))
            ],
          )),
      proc: () {
        connect(context, id, isRDP: true);
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _wolAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('WOL'),
        style: style,
      ),
      proc: () {
        bind.mainWol(id: id);
      },
      dismissOnClicked: true,
    );
  }

  @protected
  Future<MenuEntryBase<String>> _forceAlwaysRelayAction(String id) async {
    const option = 'force-always-relay';
    return MenuEntrySwitch<String>(
      text: translate('Always connect via relay'),
      getter: () async {
        return (await bind.mainGetPeerOption(id: id, key: option)).isNotEmpty;
      },
      setter: (bool v) async {
        String value;
        String oldValue = await bind.mainGetPeerOption(id: id, key: option);
        if (oldValue.isEmpty) {
          value = 'Y';
        } else {
          value = '';
        }
        await bind.mainSetPeerOption(id: id, key: option, value: value);
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _renameAction(String id, bool isAddressBook) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Rename'),
        style: style,
      ),
      proc: () {
        _rename(id, isAddressBook);
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _removeAction(
      String id, Future<void> Function() reloadFunc) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Remove'),
        style: style,
      ),
      proc: () {
        () async {
          await bind.mainRemovePeer(id: id);
          removePreference(id);
          await reloadFunc();
          // Get.forceAppUpdate(); // TODO use inner model / state
        }();
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _unrememberPasswordAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Unremember Password'),
        style: style,
      ),
      proc: () {
        bind.mainForgetPassword(id: id);
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _addFavAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Add to Favorites'),
        style: style,
      ),
      proc: () {
        () async {
          final favs = (await bind.mainGetFav()).toList();
          if (!favs.contains(id)) {
            favs.add(id);
            await bind.mainStoreFav(favs: favs);
          }
        }();
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _rmFavAction(
      String id, Future<void> Function() reloadFunc) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Remove from Favorites'),
        style: style,
      ),
      proc: () {
        () async {
          final favs = (await bind.mainGetFav()).toList();
          if (favs.remove(id)) {
            await bind.mainStoreFav(favs: favs);
            await reloadFunc();
            // Get.forceAppUpdate(); // TODO use inner model / state
          }
        }();
      },
      dismissOnClicked: true,
    );
  }

  void _rename(String id, bool isAddressBook) async {
    RxBool isInProgress = false.obs;
    var name = await bind.mainGetPeerOption(id: id, key: 'alias');
    var controller = TextEditingController(text: name);
    if (isAddressBook) {
      final peer = gFFI.abModel.peers.firstWhere((p) => id == p['id']);
      if (peer == null) {
        // this should not happen
      } else {
        name = peer['alias'] ?? '';
      }
    }
    gFFI.dialogManager.show((setState, close) {
      submit() async {
        isInProgress.value = true;
        name = controller.text;
        await bind.mainSetPeerOption(id: id, key: 'alias', value: name);
        if (isAddressBook) {
          gFFI.abModel.setPeerOption(id, 'alias', name);
          await gFFI.abModel.updateAb();
        }
        alias.value = await bind.mainGetPeerOption(id: peer.id, key: 'alias');
        close();
        isInProgress.value = false;
      }

      return CustomAlertDialog(
        title: Text(translate('Rename')),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
              child: Form(
                child: TextFormField(
                  controller: controller,
                  focusNode: FocusNode()..requestFocus(),
                  decoration:
                      const InputDecoration(border: OutlineInputBorder()),
                ),
              ),
            ),
            Obx(() => Offstage(
                offstage: isInProgress.isFalse,
                child: const LinearProgressIndicator())),
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }
}

class RecentPeerCard extends BasePeerCard {
  RecentPeerCard({required Peer peer, Key? key}) : super(peer: peer, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer.id),
      _transferFileAction(context, peer.id),
      _tcpTunnelingAction(context, peer.id),
    ];
    MenuEntryBase<String>? rdpAction;
    if (peer.platform == 'Windows') {
      rdpAction = _rdpAction(context, peer.id);
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (rdpAction != null) {
      menuItems.add(rdpAction);
    }
    menuItems.add(_wolAction(peer.id));
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id, false));
    menuItems.add(_removeAction(peer.id, () async {
      await bind.mainLoadRecentPeers();
    }));
    menuItems.add(_unrememberPasswordAction(peer.id));
    menuItems.add(_addFavAction(peer.id));
    return menuItems;
  }
}

class FavoritePeerCard extends BasePeerCard {
  FavoritePeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer.id),
      _transferFileAction(context, peer.id),
      _tcpTunnelingAction(context, peer.id),
    ];
    MenuEntryBase<String>? rdpAction;
    if (peer.platform == 'Windows') {
      rdpAction = _rdpAction(context, peer.id);
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (rdpAction != null) {
      menuItems.add(rdpAction);
    }
    menuItems.add(_wolAction(peer.id));
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id, false));
    menuItems.add(_removeAction(peer.id, () async {
      await bind.mainLoadFavPeers();
    }));
    menuItems.add(_unrememberPasswordAction(peer.id));
    menuItems.add(_rmFavAction(peer.id, () async {
      await bind.mainLoadFavPeers();
    }));
    return menuItems;
  }
}

class DiscoveredPeerCard extends BasePeerCard {
  DiscoveredPeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer.id),
      _transferFileAction(context, peer.id),
      _tcpTunnelingAction(context, peer.id),
    ];
    MenuEntryBase<String>? rdpAction;
    if (peer.platform == 'Windows') {
      rdpAction = _rdpAction(context, peer.id);
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (rdpAction != null) {
      menuItems.add(rdpAction);
    }
    menuItems.add(_wolAction(peer.id));
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id, false));
    menuItems.add(_removeAction(peer.id, () async {
      await bind.mainLoadLanPeers();
    }));
    menuItems.add(_unrememberPasswordAction(peer.id));
    return menuItems;
  }
}

class AddressBookPeerCard extends BasePeerCard {
  AddressBookPeerCard({required Peer peer, Key? key})
      : super(peer: peer, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer.id),
      _transferFileAction(context, peer.id),
      _tcpTunnelingAction(context, peer.id),
    ];
    MenuEntryBase<String>? rdpAction;
    if (peer.platform == 'Windows') {
      rdpAction = _rdpAction(context, peer.id);
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (rdpAction != null) {
      menuItems.add(rdpAction);
    }
    menuItems.add(_wolAction(peer.id));
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id, false));
    menuItems.add(_removeAction(peer.id, () async {}));
    menuItems.add(_unrememberPasswordAction(peer.id));
    menuItems.add(_addFavAction(peer.id));
    menuItems.add(_editTagAction(peer.id));
    return menuItems;
  }

  @protected
  @override
  MenuEntryBase<String> _removeAction(
      String id, Future<void> Function() reloadFunc) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Remove'),
        style: style,
      ),
      proc: () {
        () async {
          gFFI.abModel.deletePeer(id);
          await gFFI.abModel.updateAb();
        }();
      },
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _editTagAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Edit Tag'),
        style: style,
      ),
      proc: () {
        _abEditTag(id);
      },
      dismissOnClicked: true,
    );
  }

  void _abEditTag(String id) {
    var isInProgress = false;

    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = gFFI.abModel.getPeerTags(id).obs;

    gFFI.dialogManager.show((setState, close) {
      submit() async {
        setState(() {
          isInProgress = true;
        });
        gFFI.abModel.changeTagForPeer(id, selectedTag);
        await gFFI.abModel.updateAb();
        close();
      }

      return CustomAlertDialog(
        title: Text(translate("Edit Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
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
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
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
            margin: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 8.0),
            padding: const EdgeInsets.symmetric(vertical: 2.0, horizontal: 8.0),
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
}

void _rdpDialog(String id) async {
  final portController = TextEditingController(
      text: await bind.mainGetPeerOption(id: id, key: 'rdp_port'));
  final userController = TextEditingController(
      text: await bind.mainGetPeerOption(id: id, key: 'rdp_username'));
  final passwordContorller = TextEditingController(
      text: await bind.mainGetPeerOption(id: id, key: 'rdp_password'));
  RxBool secure = true.obs;

  gFFI.dialogManager.show((setState, close) {
    submit() async {
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_port', value: portController.text.trim());
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_username', value: userController.text);
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_password', value: passwordContorller.text);
      close();
    }

    return CustomAlertDialog(
      title: Text('RDP ${translate('Settings')}'),
      content: ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 100),
                    child: Text(
                      "${translate('Port')}:",
                      textAlign: TextAlign.start,
                    ).marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$'))
                    ],
                    decoration: const InputDecoration(
                        border: OutlineInputBorder(), hintText: '3389'),
                    controller: portController,
                    focusNode: FocusNode()..requestFocus(),
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 100),
                    child: Text(
                      "${translate('Username')}:",
                      textAlign: TextAlign.start,
                    ).marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration:
                        const InputDecoration(border: OutlineInputBorder()),
                    controller: userController,
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 100),
                    child: Text("${translate('Password')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: Obx(() => TextField(
                        obscureText: secure.value,
                        decoration: InputDecoration(
                            border: const OutlineInputBorder(),
                            suffixIcon: IconButton(
                                onPressed: () => secure.value = !secure.value,
                                icon: Icon(secure.value
                                    ? Icons.visibility_off
                                    : Icons.visibility))),
                        controller: passwordContorller,
                      )),
                ),
              ],
            ),
          ],
        ),
      ),
      actions: [
        TextButton(onPressed: close, child: Text(translate("Cancel"))),
        TextButton(onPressed: submit, child: Text(translate("OK"))),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

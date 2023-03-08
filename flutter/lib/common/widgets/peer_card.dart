import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../models/model.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../desktop/widgets/popup_menu.dart';

typedef PopupMenuEntryBuilder = Future<List<mod_menu.PopupMenuEntry<String>>>
    Function(BuildContext);

enum PeerUiType { grid, list }

final peerCardUiType = PeerUiType.grid.obs;

class _PeerCard extends StatefulWidget {
  final Peer peer;
  final Function(BuildContext, String) connect;
  final PopupMenuEntryBuilder popupMenuEntryBuilder;

  const _PeerCard(
      {required this.peer,
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
  final double _cardRadius = 16;
  final double _borderWidth = 2;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    if (isDesktop) {
      return _buildDesktop();
    } else {
      return _buildMobile();
    }
  }

  Widget _buildMobile() {
    final peer = super.widget.peer;
    final name =
        '${peer.username}${peer.username.isNotEmpty && peer.hostname.isNotEmpty ? '@' : ''}${peer.hostname}';

    return Card(
        margin: EdgeInsets.symmetric(horizontal: 2),
        child: GestureDetector(
            onTap: !isWebDesktop ? () => connect(context, peer.id) : null,
            onDoubleTap: isWebDesktop ? () => connect(context, peer.id) : null,
            onLongPressStart: (details) {
              final x = details.globalPosition.dx;
              final y = details.globalPosition.dy;
              _menuPos = RelativeRect.fromLTRB(x, y, x, y);
              _showPeerMenu(peer.id);
            },
            child: Container(
              padding: EdgeInsets.only(left: 12, top: 8, bottom: 8),
              child: Row(
                children: [
                  Container(
                      width: 50,
                      height: 50,
                      decoration: BoxDecoration(
                        color: str2color('${peer.id}${peer.platform}', 0x7f),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      padding: const EdgeInsets.all(6),
                      child: getPlatformImage(peer.platform)),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(children: [
                          getOnline(4, peer.online),
                          Text(peer.alias.isEmpty
                              ? formatID(peer.id)
                              : peer.alias)
                        ]),
                        Text(name)
                      ],
                    ).paddingOnly(left: 8.0),
                  ),
                  InkWell(
                      child: const Padding(
                          padding: EdgeInsets.all(12),
                          child: Icon(Icons.more_vert)),
                      onTapDown: (e) {
                        final x = e.globalPosition.dx;
                        final y = e.globalPosition.dy;
                        _menuPos = RelativeRect.fromLTRB(x, y, x, y);
                      },
                      onTap: () {
                        _showPeerMenu(peer.id);
                      })
                ],
              ),
            )));
  }

  Widget _buildDesktop() {
    final peer = super.widget.peer;
    var deco = Rx<BoxDecoration?>(BoxDecoration(
        border: Border.all(color: Colors.transparent, width: _borderWidth),
        borderRadius: peerCardUiType.value == PeerUiType.grid
            ? BorderRadius.circular(_cardRadius)
            : null));
    return MouseRegion(
      onEnter: (evt) {
        deco.value = BoxDecoration(
            border: Border.all(
                color: Theme.of(context).colorScheme.primary,
                width: _borderWidth),
            borderRadius: peerCardUiType.value == PeerUiType.grid
                ? BorderRadius.circular(_cardRadius)
                : null);
      },
      onExit: (evt) {
        deco.value = BoxDecoration(
            border: Border.all(color: Colors.transparent, width: _borderWidth),
            borderRadius: peerCardUiType.value == PeerUiType.grid
                ? BorderRadius.circular(_cardRadius)
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
    final name =
        '${peer.username}${peer.username.isNotEmpty && peer.hostname.isNotEmpty ? '@' : ''}${peer.hostname}';
    final greyStyle = TextStyle(
        fontSize: 11,
        color: Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6));
    final alias = bind.mainGetPeerOptionSync(id: peer.id, key: 'alias');
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
              width: 42,
              child: getPlatformImage(peer.platform, size: 30).paddingAll(6),
            ),
            Expanded(
              child: Container(
                decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.background),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        children: [
                          Row(children: [
                            getOnline(8, peer.online),
                            Expanded(
                                child: Text(
                              alias.isEmpty ? formatID(peer.id) : alias,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.titleSmall,
                            )),
                          ]).marginOnly(bottom: 2),
                          Align(
                            alignment: Alignment.centerLeft,
                            child: Text(
                              name,
                              style: greyStyle,
                              textAlign: TextAlign.start,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      ).marginOnly(top: 2),
                    ),
                    _actionMore(peer),
                  ],
                ).paddingOnly(left: 10.0, top: 3.0),
              ),
            )
          ],
        ),
      ),
    );
  }

  Widget _buildPeerCard(
      BuildContext context, Peer peer, Rx<BoxDecoration?> deco) {
    final name =
        '${peer.username}${peer.username.isNotEmpty && peer.hostname.isNotEmpty ? '@' : ''}${peer.hostname}';
    return Card(
      color: Colors.transparent,
      elevation: 0,
      margin: EdgeInsets.zero,
      child: Obx(
        () => Container(
          foregroundDecoration: deco.value,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(_cardRadius - _borderWidth),
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
                                    child: Tooltip(
                                      message: name,
                                      waitDuration: const Duration(seconds: 1),
                                      child: Text(
                                        name,
                                        style: const TextStyle(
                                            color: Colors.white70,
                                            fontSize: 12),
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
                Container(
                  color: Theme.of(context).colorScheme.background,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Expanded(
                          child: Row(children: [
                        getOnline(8, peer.online),
                        Expanded(
                            child: Text(
                          peer.alias.isEmpty ? formatID(peer.id) : peer.alias,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.titleSmall,
                        )),
                      ]).paddingSymmetric(vertical: 8)),
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
      onPointerUp: (_) => _showPeerMenu(peer.id),
      child: ActionMore());

  /// Show the peer menu and handle user's choice.
  /// User might remove the peer or send a file to the peer.
  void _showPeerMenu(String id) async {
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
  final Peer peer;
  final EdgeInsets? menuPadding;

  BasePeerCard({required this.peer, this.menuPadding, Key? key})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _PeerCard(
      peer: peer,
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
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: CustomPopupMenuTheme.dividerHeight)))
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
        title,
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
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _connectAction(BuildContext context, Peer peer) {
    return _connectCommonAction(
        context,
        peer.id,
        peer.alias.isEmpty
            ? translate('Connect')
            : "${translate('Connect')} ${peer.id}");
  }

  @protected
  MenuEntryBase<String> _transferFileAction(BuildContext context, String id) {
    return _connectCommonAction(
      context,
      id,
      translate('Transfer File'),
      isFileTransfer: true,
    );
  }

  @protected
  MenuEntryBase<String> _tcpTunnelingAction(BuildContext context, String id) {
    return _connectCommonAction(
      context,
      id,
      translate('TCP Tunneling'),
      isTcpTunneling: true,
    );
  }

  @protected
  MenuEntryBase<String> _rdpAction(BuildContext context, String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Container(
          alignment: AlignmentDirectional.center,
          height: CustomPopupMenuTheme.height,
          child: Row(
            children: [
              Text(
                translate('RDP'),
                style: style,
              ),
              Expanded(
                  child: Align(
                alignment: Alignment.centerRight,
                child: Transform.scale(
                    scale: 0.8,
                    child: IconButton(
                      icon: const Icon(Icons.edit),
                      padding: EdgeInsets.zero,
                      onPressed: () {
                        if (Navigator.canPop(context)) {
                          Navigator.pop(context);
                        }
                        _rdpDialog(id);
                      },
                    )),
              ))
            ],
          )),
      proc: () {
        connect(context, id, isRDP: true);
      },
      padding: menuPadding,
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
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  /// Only available on Windows.
  @protected
  MenuEntryBase<String> _createShortCutAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Create Desktop Shortcut'),
        style: style,
      ),
      proc: () {
        bind.mainCreateShortcut(id: id);
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  Future<bool> _isForceAlwaysRelay(String id) async {
    return (await bind.mainGetPeerOption(id: id, key: 'force-always-relay'))
        .isNotEmpty;
  }

  @protected
  Future<MenuEntryBase<String>> _forceAlwaysRelayAction(String id) async {
    const option = 'force-always-relay';
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Always connect via relay'),
      getter: () async {
        return await _isForceAlwaysRelay(id);
      },
      setter: (bool v) async {
        gFFI.abModel.setPeerForceAlwaysRelay(id, v);
        await bind.mainSetPeerOption(
            id: id, key: option, value: bool2option(option, v));
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _renameAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Rename'),
        style: style,
      ),
      proc: () {
        _rename(id);
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _removeAction(
      String id, Future<void> Function() reloadFunc,
      {bool isLan = false}) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Row(
        children: [
          Text(
            translate('Delete'),
            style: style?.copyWith(color: Colors.red),
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Transform.scale(
              scale: 0.8,
              child: Icon(Icons.delete_forever, color: Colors.red),
            ),
          ).marginOnly(right: 4)),
        ],
      ),
      proc: () {
        _delete(id, isLan, reloadFunc);
      },
      padding: menuPadding,
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
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _addFavAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Row(
        children: [
          Text(
            translate('Add to Favorites'),
            style: style,
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Transform.scale(
              scale: 0.8,
              child: Icon(Icons.star_outline),
            ),
          ).marginOnly(right: 4)),
        ],
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
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _rmFavAction(
      String id, Future<void> Function() reloadFunc) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Row(
        children: [
          Text(
            translate('Remove from Favorites'),
            style: style,
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Transform.scale(
              scale: 0.8,
              child: Icon(Icons.star),
            ),
          ).marginOnly(right: 4)),
        ],
      ),
      proc: () {
        () async {
          final favs = (await bind.mainGetFav()).toList();
          if (favs.remove(id)) {
            await bind.mainStoreFav(favs: favs);
            await reloadFunc();
          }
        }();
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _addToAb(Peer peer) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Add to Address Book'),
        style: style,
      ),
      proc: () {
        () async {
          if (!gFFI.abModel.idContainBy(peer.id)) {
            gFFI.abModel.addPeer(peer);
            await gFFI.abModel.pushAb();
          }
        }();
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  Future<String> _getAlias(String id) async =>
      await bind.mainGetPeerOption(id: id, key: 'alias');

  void _rename(String id) async {
    RxBool isInProgress = false.obs;
    String name = await _getAlias(id);
    var controller = TextEditingController(text: name);
    gFFI.dialogManager.show((setState, close) {
      submit() async {
        isInProgress.value = true;
        String name = controller.text.trim();
        await bind.mainSetPeerAlias(id: id, alias: name);
        gFFI.abModel.setPeerAlias(id, name);
        _update();
        close();
        isInProgress.value = false;
      }

      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.edit_rounded, color: MyTheme.accent),
            Text(translate('Rename')).paddingOnly(left: 10),
          ],
        ),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              child: Form(
                child: TextFormField(
                  controller: controller,
                  autofocus: true,
                  decoration: InputDecoration(labelText: translate('Name')),
                ),
              ),
            ),
            Obx(() => Offstage(
                offstage: isInProgress.isFalse,
                child: const LinearProgressIndicator())),
          ],
        ),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: close,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  @protected
  void _update();

  void _delete(String id, bool isLan, Function reloadFunc) async {
    gFFI.dialogManager.show(
      (setState, close) {
        submit() async {
          if (isLan) {
            bind.mainRemoveDiscovered(id: id);
          } else {
            final favs = (await bind.mainGetFav()).toList();
            if (favs.remove(id)) {
              await bind.mainStoreFav(favs: favs);
            }
            await bind.mainRemovePeer(id: id);
          }
          removePreference(id);
          await reloadFunc();
          close();
        }

        return CustomAlertDialog(
          title: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.delete_rounded,
                color: Colors.red,
              ),
              Text(translate('Delete')).paddingOnly(
                left: 10,
              ),
            ],
          ),
          content: SizedBox.shrink(),
          actions: [
            dialogButton(
              "Cancel",
              icon: Icon(Icons.close_rounded),
              onPressed: close,
              isOutline: true,
            ),
            dialogButton(
              "OK",
              icon: Icon(Icons.done_rounded),
              onPressed: submit,
            ),
          ],
          onSubmit: submit,
          onCancel: close,
        );
      },
    );
  }
}

class RecentPeerCard extends BasePeerCard {
  RecentPeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(peer: peer, menuPadding: menuPadding, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer),
      _transferFileAction(context, peer.id),
    ];

    final List favs = (await bind.mainGetFav()).toList();

    if (isDesktop && peer.platform != 'Android') {
      menuItems.add(_tcpTunnelingAction(context, peer.id));
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (peer.platform == 'Windows') {
      menuItems.add(_rdpAction(context, peer.id));
    }
    menuItems.add(_wolAction(peer.id));
    if (Platform.isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id));
    if (await bind.mainPeerHasPassword(id: peer.id)) {
      menuItems.add(_unrememberPasswordAction(peer.id));
    }

    if (!favs.contains(peer.id)) {
      menuItems.add(_addFavAction(peer.id));
    } else {
      menuItems.add(_rmFavAction(peer.id, () async {}));
    }

    if (gFFI.userModel.userName.isNotEmpty) {
      if (!gFFI.abModel.idContainBy(peer.id)) {
        menuItems.add(_addToAb(peer));
      }
    }

    menuItems.add(MenuEntryDivider());
    menuItems.add(_removeAction(peer.id, () async {
      await bind.mainLoadRecentPeers();
    }));
    return menuItems;
  }

  @protected
  @override
  void _update() => bind.mainLoadRecentPeers();
}

class FavoritePeerCard extends BasePeerCard {
  FavoritePeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(peer: peer, menuPadding: menuPadding, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer),
      _transferFileAction(context, peer.id),
    ];
    if (isDesktop && peer.platform != 'Android') {
      menuItems.add(_tcpTunnelingAction(context, peer.id));
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (peer.platform == 'Windows') {
      menuItems.add(_rdpAction(context, peer.id));
    }
    menuItems.add(_wolAction(peer.id));
    if (Platform.isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id));
    if (await bind.mainPeerHasPassword(id: peer.id)) {
      menuItems.add(_unrememberPasswordAction(peer.id));
    }
    menuItems.add(_rmFavAction(peer.id, () async {
      await bind.mainLoadFavPeers();
    }));

    if (gFFI.userModel.userName.isNotEmpty) {
      if (!gFFI.abModel.idContainBy(peer.id)) {
        menuItems.add(_addToAb(peer));
      }
    }

    menuItems.add(MenuEntryDivider());
    menuItems.add(_removeAction(peer.id, () async {
      await bind.mainLoadFavPeers();
    }));
    return menuItems;
  }

  @protected
  @override
  void _update() => bind.mainLoadFavPeers();
}

class DiscoveredPeerCard extends BasePeerCard {
  DiscoveredPeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(peer: peer, menuPadding: menuPadding, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer),
      _transferFileAction(context, peer.id),
    ];

    final List favs = (await bind.mainGetFav()).toList();

    if (isDesktop && peer.platform != 'Android') {
      menuItems.add(_tcpTunnelingAction(context, peer.id));
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (peer.platform == 'Windows') {
      menuItems.add(_rdpAction(context, peer.id));
    }
    menuItems.add(_wolAction(peer.id));
    if (Platform.isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }

    if (!favs.contains(peer.id)) {
      menuItems.add(_addFavAction(peer.id));
    } else {
      menuItems.add(_rmFavAction(peer.id, () async {}));
    }

    if (gFFI.userModel.userName.isNotEmpty) {
      if (!gFFI.abModel.idContainBy(peer.id)) {
        menuItems.add(_addToAb(peer));
      }
    }

    menuItems.add(MenuEntryDivider());
    menuItems.add(
      _removeAction(peer.id, () async {
        await bind.mainLoadLanPeers();
      }, isLan: true),
    );
    return menuItems;
  }

  @protected
  @override
  void _update() => bind.mainLoadLanPeers();
}

class AddressBookPeerCard extends BasePeerCard {
  AddressBookPeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(peer: peer, menuPadding: menuPadding, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer),
      _transferFileAction(context, peer.id),
    ];
    if (isDesktop && peer.platform != 'Android') {
      menuItems.add(_tcpTunnelingAction(context, peer.id));
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (peer.platform == 'Windows') {
      menuItems.add(_rdpAction(context, peer.id));
    }
    menuItems.add(_wolAction(peer.id));
    if (Platform.isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id));
    if (await bind.mainPeerHasPassword(id: peer.id)) {
      menuItems.add(_unrememberPasswordAction(peer.id));
    }
    if (gFFI.abModel.tags.isNotEmpty) {
      menuItems.add(_editTagAction(peer.id));
    }

    menuItems.add(MenuEntryDivider());
    menuItems.add(_removeAction(peer.id, () async {}));
    return menuItems;
  }

  @protected
  @override
  Future<bool> _isForceAlwaysRelay(String id) async =>
      gFFI.abModel.find(id)?.forceAlwaysRelay ?? false;

  @protected
  @override
  Future<String> _getAlias(String id) async =>
      gFFI.abModel.find(id)?.alias ?? '';

  @protected
  @override
  void _update() => gFFI.abModel.pullAb();

  @protected
  @override
  MenuEntryBase<String> _removeAction(
      String id, Future<void> Function() reloadFunc,
      {bool isLan = false}) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Remove'),
        style: style,
      ),
      proc: () {
        () async {
          gFFI.abModel.deletePeer(id);
          await gFFI.abModel.pushAb();
        }();
      },
      padding: super.menuPadding,
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
      padding: super.menuPadding,
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
        await gFFI.abModel.pushAb();
        close();
      }

      return CustomAlertDialog(
        title: Text(translate("Edit Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: const EdgeInsets.symmetric(vertical: 8.0),
              child: Wrap(
                children: tags
                    .map((e) => AddressBookTag(
                        name: e,
                        tags: selectedTag,
                        onTap: () {
                          if (selectedTag.contains(e)) {
                            selectedTag.remove(e);
                          } else {
                            selectedTag.add(e);
                          }
                        },
                        showActionMenu: false))
                    .toList(growable: false),
              ),
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          dialogButton("Cancel", onPressed: close, isOutline: true),
          dialogButton("OK", onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }
}

class MyGroupPeerCard extends BasePeerCard {
  MyGroupPeerCard({required Peer peer, EdgeInsets? menuPadding, Key? key})
      : super(peer: peer, menuPadding: menuPadding, key: key);

  @override
  Future<List<MenuEntryBase<String>>> _buildMenuItems(
      BuildContext context) async {
    final List<MenuEntryBase<String>> menuItems = [
      _connectAction(context, peer),
      _transferFileAction(context, peer.id),
    ];
    if (isDesktop && peer.platform != 'Android') {
      menuItems.add(_tcpTunnelingAction(context, peer.id));
    }
    menuItems.add(await _forceAlwaysRelayAction(peer.id));
    if (peer.platform == 'Windows') {
      menuItems.add(_rdpAction(context, peer.id));
    }
    menuItems.add(_wolAction(peer.id));
    if (Platform.isWindows) {
      menuItems.add(_createShortCutAction(peer.id));
    }
    menuItems.add(MenuEntryDivider());
    menuItems.add(_renameAction(peer.id));
    if (await bind.mainPeerHasPassword(id: peer.id)) {
      menuItems.add(_unrememberPasswordAction(peer.id));
    }
    return menuItems;
  }

  @protected
  @override
  void _update() => gFFI.groupModel.pull();
}

void _rdpDialog(String id) async {
  final port = await bind.mainGetPeerOption(id: id, key: 'rdp_port');
  final username = await bind.mainGetPeerOption(id: id, key: 'rdp_username');
  final portController = TextEditingController(text: port);
  final userController = TextEditingController(text: username);
  final passwordController = TextEditingController(
      text: await bind.mainGetPeerOption(id: id, key: 'rdp_password'));
  RxBool secure = true.obs;

  gFFI.dialogManager.show((setState, close) {
    submit() async {
      String port = portController.text.trim();
      String username = userController.text;
      String password = passwordController.text;
      await bind.mainSetPeerOption(id: id, key: 'rdp_port', value: port);
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_username', value: username);
      await bind.mainSetPeerOption(
          id: id, key: 'rdp_password', value: password);
      gFFI.abModel.setRdp(id, port, username);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate('RDP Settings')),
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
                    constraints: const BoxConstraints(minWidth: 140),
                    child: Text(
                      "${translate('Port')}:",
                      textAlign: TextAlign.right,
                    ).marginOnly(right: 10)),
                Expanded(
                  child: TextField(
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$'))
                    ],
                    decoration: const InputDecoration(
                        border: OutlineInputBorder(), hintText: '3389'),
                    controller: portController,
                    autofocus: true,
                  ),
                ),
              ],
            ).marginOnly(bottom: 8),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 140),
                    child: Text(
                      "${translate('Username')}:",
                      textAlign: TextAlign.right,
                    ).marginOnly(right: 10)),
                Expanded(
                  child: TextField(
                    decoration:
                        const InputDecoration(border: OutlineInputBorder()),
                    controller: userController,
                  ),
                ),
              ],
            ).marginOnly(bottom: 8),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 140),
                    child: Text(
                      "${translate('Password')}:",
                      textAlign: TextAlign.right,
                    ).marginOnly(right: 10)),
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
                        controller: passwordController,
                      )),
                ),
              ],
            ).marginOnly(bottom: 8),
          ],
        ),
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

Widget getOnline(double rightPadding, bool online) {
  return Tooltip(
      message: translate(online ? 'Online' : 'Offline'),
      waitDuration: const Duration(seconds: 1),
      child: Padding(
          padding: EdgeInsets.fromLTRB(0, 4, rightPadding, 4),
          child: CircleAvatar(
              radius: 3, backgroundColor: online ? Colors.green : kColorWarn)));
}

class ActionMore extends StatelessWidget {
  final RxBool _hover = false.obs;

  @override
  Widget build(BuildContext context) {
    return InkWell(
        onTap: () {},
        onHover: (value) => _hover.value = value,
        child: Obx(() => CircleAvatar(
            radius: 14,
            backgroundColor: _hover.value
                ? Theme.of(context).scaffoldBackgroundColor
                : Theme.of(context).colorScheme.background,
            child: Icon(Icons.more_vert,
                size: 18,
                color: _hover.value
                    ? Theme.of(context).textTheme.titleLarge?.color
                    : Theme.of(context)
                        .textTheme
                        .titleLarge
                        ?.color
                        ?.withOpacity(0.5)))));
  }
}

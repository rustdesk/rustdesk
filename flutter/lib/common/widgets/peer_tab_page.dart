import 'dart:ui' as ui;

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

class PeerTabPage extends StatefulWidget {
  const PeerTabPage({Key? key}) : super(key: key);
  @override
  State<PeerTabPage> createState() => _PeerTabPageState();
}

class _TabEntry {
  final String name;
  final Widget widget;
  final Function() load;
  _TabEntry(this.name, this.widget, this.load);
}

class _PeerTabPageState extends State<PeerTabPage>
    with SingleTickerProviderStateMixin {
  late final RxInt _tabHiddenFlag;
  late final RxString _currentTab;
  final List<_TabEntry> entries = [
    _TabEntry(
        'Recent Sessions',
        RecentPeersView(
          menuPadding: kDesktopMenuPadding,
        ),
        bind.mainLoadRecentPeers),
    _TabEntry(
        'Favorites',
        FavoritePeersView(
          menuPadding: kDesktopMenuPadding,
        ),
        bind.mainLoadFavPeers),
    _TabEntry(
        'Discovered',
        DiscoveredPeersView(
          menuPadding: kDesktopMenuPadding,
        ),
        bind.mainDiscover),
    _TabEntry(
        'Address Book',
        const AddressBook(
          menuPadding: kDesktopMenuPadding,
        ),
        () => {}),
  ];

  @override
  void initState() {
    _tabHiddenFlag = (int.tryParse(
                bind.getLocalFlutterConfig(k: 'hidden-peer-card'),
                radix: 2) ??
            0)
        .obs;
    _currentTab = bind.getLocalFlutterConfig(k: 'current-peer-tab').obs;
    adjustTab();

    final uiType = bind.getLocalFlutterConfig(k: 'peer-card-ui-type');
    if (uiType != '') {
      peerCardUiType.value = int.parse(uiType) == PeerUiType.list.index
          ? PeerUiType.list
          : PeerUiType.grid;
    }
    super.initState();
  }

  // hard code for now
  Future<void> handleTabSelection(String tabName) async {
    _currentTab.value = tabName;
    await bind.setLocalFlutterConfig(k: 'current-peer-tab', v: tabName);
    entries.firstWhereOrNull((e) => e.name == tabName)?.load();
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      textBaseline: TextBaseline.ideographic,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          height: 28,
          child: Container(
              padding: isDesktop ? null : EdgeInsets.symmetric(horizontal: 2),
              constraints: isDesktop ? null : kMobilePageConstraints,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Expanded(
                      child: visibleContextMenuListener(
                          _createSwitchBar(context))),
                  const PeerSearchBar(),
                  Offstage(
                      offstage: !isDesktop,
                      child: _createPeerViewTypeSwitch(context)
                          .marginOnly(left: 13)),
                ],
              )),
        ),
        _createPeersView(),
      ],
    );
  }

  Widget _createSwitchBar(BuildContext context) {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    return Obx(() => ListView(
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        controller: ScrollController(),
        children: entries.where((e) => !isTabHidden(e.name)).map((t) {
          return InkWell(
            child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                decoration: BoxDecoration(
                  color: _currentTab.value == t.name
                      ? Theme.of(context).backgroundColor
                      : null,
                  borderRadius: BorderRadius.circular(isDesktop ? 2 : 6),
                ),
                child: Align(
                  alignment: Alignment.center,
                  child: Text(
                    translate(t.name),
                    textAlign: TextAlign.center,
                    style: TextStyle(
                        height: 1,
                        fontSize: 14,
                        color:
                            _currentTab.value == t.name ? textColor : textColor
                              ?..withOpacity(0.5)),
                  ),
                )),
            onTap: () async => await handleTabSelection(t.name),
          );
        }).toList()));
  }

  Widget _createPeersView() {
    final verticalMargin = isDesktop ? 12.0 : 6.0;
    return Expanded(
      child: Obx(() =>
          entries
              .firstWhereOrNull((e) => e.name == _currentTab.value)
              ?.widget ??
          visibleContextMenuListener(Center(
            child: Text(translate('Right click to select tabs')),
          ))).marginSymmetric(vertical: verticalMargin),
    );
  }

  Widget _createPeerViewTypeSwitch(BuildContext context) {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    final activeDeco = BoxDecoration(color: Theme.of(context).backgroundColor);
    return Row(
      children: [PeerUiType.grid, PeerUiType.list]
          .map((type) => Obx(
                () => Container(
                  padding: EdgeInsets.all(4.0),
                  decoration: peerCardUiType.value == type ? activeDeco : null,
                  child: InkWell(
                      onTap: () async {
                        await bind.setLocalFlutterConfig(
                            k: 'peer-card-ui-type', v: type.index.toString());
                        peerCardUiType.value = type;
                      },
                      child: Icon(
                        type == PeerUiType.grid
                            ? Icons.grid_view_rounded
                            : Icons.list,
                        size: 18,
                        color:
                            peerCardUiType.value == type ? textColor : textColor
                              ?..withOpacity(0.5),
                      )),
                ),
              ))
          .toList(),
    );
  }

  bool isTabHidden(String name) {
    int index = entries.indexWhere((e) => e.name == name);
    if (index >= 0) {
      return _tabHiddenFlag & (1 << index) != 0;
    }
    assert(false);
    return false;
  }

  adjustTab() {
    List<String> visibleTabs =
        entries.where((e) => !isTabHidden(e.name)).map((e) => e.name).toList();
    if (visibleTabs.isNotEmpty) {
      if (!visibleTabs.contains(_currentTab.value)) {
        handleTabSelection(visibleTabs[0]);
      }
    } else {
      _currentTab.value = '';
    }
  }

  Widget visibleContextMenuListener(Widget child) {
    return Listener(
        onPointerDown: (e) {
          if (e.kind != ui.PointerDeviceKind.mouse) {
            return;
          }
          if (e.buttons == 2) {
            showRightMenu(
              (CancelFunc cancelFunc) {
                return visibleContextMenu(cancelFunc);
              },
              target: e.position,
            );
          }
        },
        child: child);
  }

  Widget visibleContextMenu(CancelFunc cancelFunc) {
    final List<MenuEntryBase> menu = entries.asMap().entries.map((e) {
      int bitMask = 1 << e.key;
      return MenuEntrySwitch(
          switchType: SwitchType.scheckbox,
          text: translate(e.value.name),
          getter: () async {
            return _tabHiddenFlag.value & bitMask == 0;
          },
          setter: (show) async {
            if (show) {
              _tabHiddenFlag.value &= ~bitMask;
            } else {
              _tabHiddenFlag.value |= bitMask;
            }
            await bind.setLocalFlutterConfig(
                k: 'hidden-peer-card',
                v: _tabHiddenFlag.value.toRadixString(2));
            cancelFunc();
            adjustTab();
          });
    }).toList();
    return mod_menu.PopupMenu(
      items: menu
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: MyTheme.accent,
                height: 20.0,
                dividerHeight: 12.0,
              )))
          .expand((i) => i)
          .toList(),
    );
  }
}

class PeerSearchBar extends StatefulWidget {
  const PeerSearchBar({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _PeerSearchBarState();
}

class _PeerSearchBarState extends State<PeerSearchBar> {
  var drawer = false;

  @override
  Widget build(BuildContext context) {
    return drawer
        ? _buildSearchBar()
        : IconButton(
            alignment: Alignment.centerRight,
            padding: const EdgeInsets.only(right: 2),
            onPressed: () {
              setState(() {
                drawer = true;
              });
            },
            icon: Icon(
              Icons.search_rounded,
              color: Theme.of(context).hintColor,
            ));
  }

  Widget _buildSearchBar() {
    RxBool focused = false.obs;
    FocusNode focusNode = FocusNode();
    focusNode.addListener(() => focused.value = focusNode.hasFocus);
    return Container(
      width: 120,
      decoration: BoxDecoration(
        color: Theme.of(context).backgroundColor,
        borderRadius: BorderRadius.circular(6),
      ),
      child: Obx(() => Row(
            children: [
              Expanded(
                child: Row(
                  children: [
                    Icon(
                      Icons.search_rounded,
                      color: Theme.of(context).hintColor,
                    ).marginSymmetric(horizontal: 4),
                    Expanded(
                      child: TextField(
                        autofocus: true,
                        controller: peerSearchTextController,
                        onChanged: (searchText) {
                          peerSearchText.value = searchText;
                        },
                        focusNode: focusNode,
                        textAlign: TextAlign.start,
                        maxLines: 1,
                        cursorColor: Theme.of(context)
                            .textTheme
                            .titleLarge
                            ?.color
                            ?.withOpacity(0.5),
                        cursorHeight: 18,
                        cursorWidth: 1,
                        style: const TextStyle(fontSize: 14),
                        decoration: InputDecoration(
                          contentPadding:
                              const EdgeInsets.symmetric(vertical: 6),
                          hintText:
                              focused.value ? null : translate("Search ID"),
                          hintStyle: TextStyle(
                              fontSize: 14, color: Theme.of(context).hintColor),
                          border: InputBorder.none,
                          isDense: true,
                        ),
                      ),
                    ),
                    // Icon(Icons.close),
                    IconButton(
                        alignment: Alignment.centerRight,
                        padding: const EdgeInsets.only(right: 2),
                        onPressed: () {
                          setState(() {
                            peerSearchTextController.clear();
                            peerSearchText.value = "";
                            drawer = false;
                          });
                        },
                        icon: Icon(
                          Icons.close,
                          color: Theme.of(context).hintColor,
                        )),
                  ],
                ),
              )
            ],
          )),
    );
  }
}

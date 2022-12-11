import 'dart:convert';
import 'dart:ui' as ui;

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/common/widgets/my_group.dart';
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

const int groupTabIndex = 4;

class StatePeerTab {
  final RxInt currentTab = 0.obs;
  static const List<int> tabIndexs = [0, 1, 2, 3, 4];
  List<int> tabOrder = List.empty(growable: true);
  final RxList<int> visibleTabOrder = RxList.empty(growable: true);
  int tabHiddenFlag = 0;
  final RxList<String> tabNames = [
    translate('Recent Sessions'),
    translate('Favorites'),
    translate('Discovered'),
    translate('Address Book'),
    translate('Group'),
  ].obs;

  StatePeerTab._() {
    tabHiddenFlag = (int.tryParse(
            bind.getLocalFlutterConfig(k: 'hidden-peer-card'),
            radix: 2) ??
        0);
    currentTab.value =
        int.tryParse(bind.getLocalFlutterConfig(k: 'peer-tab-index')) ?? 0;
    if (!tabIndexs.contains(currentTab.value)) {
      currentTab.value = tabIndexs[0];
    }
    tabOrder = tabIndexs.toList();
    try {
      final conf = bind.getLocalFlutterConfig(k: 'peer-tab-order');
      if (conf.isNotEmpty) {
        final json = jsonDecode(conf);
        if (json is List) {
          final List<int> list =
              json.map((e) => int.tryParse(e.toString()) ?? -1).toList();
          if (list.length == tabOrder.length &&
              tabOrder.every((e) => list.contains(e))) {
            tabOrder = list;
          }
        }
      }
    } catch (e) {
      debugPrintStack(label: '$e');
    }
    visibleTabOrder.value = tabOrder.where((e) => !isTabHidden(e)).toList();
    visibleTabOrder.remove(groupTabIndex);
  }
  static final StatePeerTab instance = StatePeerTab._();

  check() {
    List<int> oldOrder = visibleTabOrder;
    if (filterGroupCard()) {
      visibleTabOrder.remove(groupTabIndex);
      if (currentTab.value == groupTabIndex) {
        currentTab.value =
            visibleTabOrder.firstWhereOrNull((e) => e != groupTabIndex) ?? 0;
        bind.setLocalFlutterConfig(
            k: 'peer-tab-index', v: currentTab.value.toString());
      }
    } else {
      if (gFFI.userModel.isAdmin.isFalse &&
          gFFI.userModel.groupName.isNotEmpty) {
        tabNames[groupTabIndex] = gFFI.userModel.groupName.value;
      } else {
        tabNames[groupTabIndex] = translate('Group');
      }
      if (isTabHidden(groupTabIndex)) {
        visibleTabOrder.remove(groupTabIndex);
      } else {
        if (!visibleTabOrder.contains(groupTabIndex)) {
          addTabInOrder(visibleTabOrder, groupTabIndex);
        }
      }
      if (visibleTabOrder.contains(groupTabIndex) &&
          int.tryParse(bind.getLocalFlutterConfig(k: 'peer-tab-index')) ==
              groupTabIndex) {
        currentTab.value = groupTabIndex;
      }
    }
    if (oldOrder != visibleTabOrder) {
      saveTabOrder();
    }
  }

  bool isTabHidden(int tabindex) {
    return tabHiddenFlag & (1 << tabindex) != 0;
  }

  bool filterGroupCard() {
    if (gFFI.groupModel.users.isEmpty ||
        (gFFI.userModel.isAdmin.isFalse && gFFI.userModel.groupName.isEmpty)) {
      return true;
    } else {
      return false;
    }
  }

  addTabInOrder(List<int> list, int tabIndex) {
    if (!tabOrder.contains(tabIndex) || list.contains(tabIndex)) {
      return;
    }
    bool sameOrder = true;
    int lastIndex = -1;
    for (int i = 0; i < list.length; i++) {
      var index = tabOrder.lastIndexOf(list[i]);
      if (index > lastIndex) {
        lastIndex = index;
        continue;
      } else {
        sameOrder = false;
        break;
      }
    }
    if (sameOrder) {
      var indexInTabOrder = tabOrder.indexOf(tabIndex);
      var left = List.empty(growable: true);
      for (int i = 0; i < indexInTabOrder; i++) {
        left.add(tabOrder[i]);
      }
      int insertIndex = list.lastIndexWhere((e) => left.contains(e));
      if (insertIndex < 0) {
        insertIndex = 0;
      } else {
        insertIndex += 1;
      }
      list.insert(insertIndex, tabIndex);
    } else {
      list.add(tabIndex);
    }
  }

  saveTabOrder() {
    var list = statePeerTab.visibleTabOrder.toList();
    var left = tabOrder
        .where((e) => !statePeerTab.visibleTabOrder.contains(e))
        .toList();
    for (var t in left) {
      addTabInOrder(list, t);
    }
    statePeerTab.tabOrder = list;
    bind.setLocalFlutterConfig(k: 'peer-tab-order', v: jsonEncode(list));
  }
}

final statePeerTab = StatePeerTab.instance;

class PeerTabPage extends StatefulWidget {
  const PeerTabPage({Key? key}) : super(key: key);
  @override
  State<PeerTabPage> createState() => _PeerTabPageState();
}

class _TabEntry {
  final Widget widget;
  final Function() load;
  _TabEntry(this.widget, this.load);
}

EdgeInsets? _menuPadding() {
  return isDesktop ? kDesktopMenuPadding : null;
}

class _PeerTabPageState extends State<PeerTabPage>
    with SingleTickerProviderStateMixin {
  final List<_TabEntry> entries = [
    _TabEntry(
        RecentPeersView(
          menuPadding: _menuPadding(),
        ),
        bind.mainLoadRecentPeers),
    _TabEntry(
        FavoritePeersView(
          menuPadding: _menuPadding(),
        ),
        bind.mainLoadFavPeers),
    _TabEntry(
        DiscoveredPeersView(
          menuPadding: _menuPadding(),
        ),
        bind.mainDiscover),
    _TabEntry(
        AddressBook(
          menuPadding: _menuPadding(),
        ),
        () => {}),
    _TabEntry(
        MyGroup(
          menuPadding: _menuPadding(),
        ),
        () => {}),
  ];

  @override
  void initState() {
    adjustTab();

    final uiType = bind.getLocalFlutterConfig(k: 'peer-card-ui-type');
    if (uiType != '') {
      peerCardUiType.value = int.parse(uiType) == PeerUiType.list.index
          ? PeerUiType.list
          : PeerUiType.grid;
    }
    super.initState();
  }

  Future<void> handleTabSelection(int tabIndex) async {
    if (tabIndex < entries.length) {
      statePeerTab.currentTab.value = tabIndex;
      entries[tabIndex].load();
    }
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
    statePeerTab.visibleTabOrder
        .removeWhere((e) => !StatePeerTab.tabIndexs.contains(e));
    return Obx(() {
      int indexCounter = -1;
      return ReorderableListView(
          buildDefaultDragHandles: false,
          onReorder: (oldIndex, newIndex) {
            var list = statePeerTab.visibleTabOrder.toList();
            if (oldIndex < newIndex) {
              newIndex -= 1;
            }
            final int item = list.removeAt(oldIndex);
            list.insert(newIndex, item);
            statePeerTab.visibleTabOrder.value = list;
            statePeerTab.saveTabOrder();
          },
          scrollDirection: Axis.horizontal,
          shrinkWrap: true,
          scrollController: ScrollController(),
          children: statePeerTab.visibleTabOrder.map((t) {
            indexCounter++;
            return ReorderableDragStartListener(
              key: ValueKey(t),
              index: indexCounter,
              child: InkWell(
                child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: statePeerTab.currentTab.value == t
                          ? Theme.of(context).backgroundColor
                          : null,
                      borderRadius: BorderRadius.circular(isDesktop ? 2 : 6),
                    ),
                    child: Align(
                      alignment: Alignment.center,
                      child: Text(
                        statePeerTab.tabNames[t], // TODO
                        textAlign: TextAlign.center,
                        style: TextStyle(
                            height: 1,
                            fontSize: 14,
                            color: statePeerTab.currentTab.value == t
                                ? textColor
                                : textColor
                              ?..withOpacity(0.5)),
                      ),
                    )),
                onTap: () async {
                  await handleTabSelection(t);
                  await bind.setLocalFlutterConfig(
                      k: 'peer-tab-index', v: t.toString());
                },
              ),
            );
          }).toList());
    });
  }

  Widget _createPeersView() {
    final verticalMargin = isDesktop ? 12.0 : 6.0;
    statePeerTab.visibleTabOrder
        .removeWhere((e) => !StatePeerTab.tabIndexs.contains(e));
    return Expanded(
        child: Obx(() {
      if (statePeerTab.visibleTabOrder.isEmpty) {
        return visibleContextMenuListener(Center(
          child: Text(translate('Right click to select tabs')),
        ));
      } else {
        if (statePeerTab.visibleTabOrder
            .contains(statePeerTab.currentTab.value)) {
          return entries[statePeerTab.currentTab.value].widget;
        } else {
          statePeerTab.currentTab.value = statePeerTab.visibleTabOrder[0];
          return entries[statePeerTab.currentTab.value].widget;
        }
      }
    }).marginSymmetric(vertical: verticalMargin));
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

  adjustTab() {
    if (statePeerTab.visibleTabOrder.isNotEmpty) {
      if (!statePeerTab.visibleTabOrder
          .contains(statePeerTab.currentTab.value)) {
        handleTabSelection(statePeerTab.visibleTabOrder[0]);
      }
    } else {
      statePeerTab.currentTab.value = 0;
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
    return Obx(() {
      final List<MenuEntryBase> menu = List.empty(growable: true);
      for (int i = 0; i < statePeerTab.tabNames.length; i++) {
        if (i == groupTabIndex && statePeerTab.filterGroupCard()) {
          continue;
        }
        int bitMask = 1 << i;
        menu.add(MenuEntrySwitch(
            switchType: SwitchType.scheckbox,
            text: statePeerTab.tabNames[i],
            getter: () async {
              return statePeerTab.tabHiddenFlag & bitMask == 0;
            },
            setter: (show) async {
              if (show) {
                statePeerTab.tabHiddenFlag &= ~bitMask;
              } else {
                statePeerTab.tabHiddenFlag |= bitMask;
              }
              await bind.setLocalFlutterConfig(
                  k: 'hidden-peer-card',
                  v: statePeerTab.tabHiddenFlag.toRadixString(2));
              statePeerTab.visibleTabOrder
                  .removeWhere((e) => statePeerTab.isTabHidden(e));
              for (int j = 0; j < statePeerTab.tabNames.length; j++) {
                if (!statePeerTab.visibleTabOrder.contains(j) &&
                    !statePeerTab.isTabHidden(j)) {
                  statePeerTab.visibleTabOrder.add(j);
                }
              }
              statePeerTab.saveTabOrder();
              cancelFunc();
              adjustTab();
            }));
      }
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
              .toList());
    });
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

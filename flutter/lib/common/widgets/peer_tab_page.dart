import 'dart:ui' as ui;

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/gestures.dart';
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
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:get/get.dart';
import 'package:get/get_rx/src/rx_workers/utils/debouncer.dart';
import 'package:provider/provider.dart';
import 'package:visibility_detector/visibility_detector.dart';
import 'package:dropdown_button2/dropdown_button2.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

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
  bool _hideSort = bind.getLocalFlutterConfig(k: 'peer-tab-index') == '0';

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
  final _scrollDebounce = Debouncer(delay: Duration(milliseconds: 50));

  @override
  void initState() {
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
      gFFI.peerTabModel.setCurrentTab(tabIndex);
      entries[tabIndex].load();
      _hideSort = tabIndex == 0;
    }
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
                    child:
                        visibleContextMenuListener(_createSwitchBar(context))),
                buildScrollJumper(),
                const PeerSearchBar(),
                Offstage(
                    offstage: !isDesktop,
                    child: _createPeerViewTypeSwitch(context)
                        .marginOnly(left: 13)),
                Offstage(
                  offstage: _hideSort,
                  child: PeerSortDropdown().marginOnly(left: 8),
                ),
              ],
            ),
          ),
        ),
        _createPeersView(),
      ],
    );
  }

  Widget _createSwitchBar(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);
    int indexCounter = -1;
    return ReorderableListView(
        buildDefaultDragHandles: false,
        onReorder: (oldIndex, newIndex) {
          model.onReorder(oldIndex, newIndex);
        },
        scrollDirection: Axis.horizontal,
        physics: NeverScrollableScrollPhysics(),
        scrollController: model.sc,
        children: model.visibleOrderedTabs.map((t) {
          indexCounter++;
          return ReorderableDragStartListener(
            key: ValueKey(t),
            index: indexCounter,
            child: VisibilityDetector(
              key: ValueKey(t),
              onVisibilityChanged: (info) {
                final id = (info.key as ValueKey).value;
                model.setTabFullyVisible(id, info.visibleFraction > 0.99);
              },
              child: Listener(
                // handle mouse wheel
                onPointerSignal: (e) {
                  if (e is PointerScrollEvent) {
                    if (!model.sc.canScroll) return;
                    _scrollDebounce.call(() {
                      model.sc.animateTo(model.sc.offset + e.scrollDelta.dy,
                          duration: Duration(milliseconds: 200),
                          curve: Curves.ease);
                    });
                  }
                },
                child: InkWell(
                  child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      decoration: BoxDecoration(
                        color: model.currentTab == t
                            ? Theme.of(context).colorScheme.background
                            : null,
                        borderRadius: BorderRadius.circular(isDesktop ? 2 : 6),
                      ),
                      child: Align(
                        alignment: Alignment.center,
                        child: Text(
                          model.translatedTabname(t),
                          textAlign: TextAlign.center,
                          style: TextStyle(
                              height: 1,
                              fontSize: 14,
                              color: model.currentTab == t
                                  ? MyTheme.tabbar(context).selectedTextColor
                                  : MyTheme.tabbar(context).unSelectedTextColor
                                ?..withOpacity(0.5)),
                        ),
                      )),
                  onTap: () async {
                    await handleTabSelection(t);
                    await bind.setLocalFlutterConfig(
                        k: 'peer-tab-index', v: t.toString());
                  },
                ),
              ),
            ),
          );
        }).toList());
  }

  Widget buildScrollJumper() {
    final model = Provider.of<PeerTabModel>(context);
    return Offstage(
        offstage: !model.showScrollBtn,
        child: Row(
          children: [
            GestureDetector(
                child: Icon(Icons.arrow_left,
                    size: 22,
                    color: model.leftFullyVisible
                        ? Theme.of(context).disabledColor
                        : null),
                onTap: model.sc.backward),
            GestureDetector(
                child: Icon(Icons.arrow_right,
                    size: 22,
                    color: model.rightFullyVisible
                        ? Theme.of(context).disabledColor
                        : null),
                onTap: model.sc.forward)
          ],
        ));
  }

  Widget _createPeersView() {
    final model = Provider.of<PeerTabModel>(context);
    Widget child;
    if (model.visibleOrderedTabs.isEmpty) {
      child = visibleContextMenuListener(Center(
        child: Text(translate('Right click to select tabs')),
      ));
    } else {
      if (model.visibleOrderedTabs.contains(model.currentTab)) {
        child = entries[model.currentTab].widget;
      } else {
        model.setCurrentTab(model.visibleOrderedTabs[0]);
        child = entries[0].widget;
      }
    }
    return Expanded(
        child: child.marginSymmetric(vertical: isDesktop ? 12.0 : 6.0));
  }

  Widget _createPeerViewTypeSwitch(BuildContext context) {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    final deco = BoxDecoration(
      color: Theme.of(context).colorScheme.background,
      borderRadius: BorderRadius.circular(5),
    );
    final types = [PeerUiType.grid, PeerUiType.list];

    return Obx(
      () => Container(
        padding: EdgeInsets.all(4.0),
        decoration: deco,
        child: InkWell(
            onTap: () async {
              final type = types.elementAt(
                  peerCardUiType.value == types.elementAt(0) ? 1 : 0);
              await bind.setLocalFlutterConfig(
                  k: 'peer-card-ui-type', v: type.index.toString());
              peerCardUiType.value = type;
            },
            child: Icon(
              peerCardUiType.value == PeerUiType.grid
                  ? Icons.list_rounded
                  : Icons.grid_view_rounded,
              size: 18,
              color: textColor,
            )),
      ),
    );
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
    final model = Provider.of<PeerTabModel>(context);
    final List<MenuEntryBase> menu = List.empty(growable: true);
    final List<int> menuIndex = List.empty(growable: true);
    var list = model.orderedNotFilteredTabs();
    for (int i = 0; i < list.length; i++) {
      int tabIndex = list[i];
      int bitMask = 1 << tabIndex;
      menuIndex.add(tabIndex);
      menu.add(MenuEntrySwitch(
          switchType: SwitchType.scheckbox,
          text: model.translatedTabname(tabIndex),
          getter: () async {
            return model.tabHiddenFlag & bitMask == 0;
          },
          setter: (show) async {
            model.onHideShow(tabIndex, show);
            cancelFunc();
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
    focusNode.addListener(() {
      focused.value = focusNode.hasFocus;
      peerSearchTextController.selection = TextSelection(
          baseOffset: 0,
          extentOffset: peerSearchTextController.value.text.length);
    });
    return Container(
      width: 120,
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.background,
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

class PeerSortDropdown extends StatefulWidget {
  const PeerSortDropdown({super.key});

  @override
  State<PeerSortDropdown> createState() => _PeerSortDropdownState();
}

class _PeerSortDropdownState extends State<PeerSortDropdown> {
  String _sortType = peerSort.value;

  @override
  Widget build(BuildContext context) {
    final deco = BoxDecoration(
      color: Theme.of(context).colorScheme.background,
      borderRadius: BorderRadius.circular(5),
    );
    return Container(
      padding: EdgeInsets.all(4.0),
      decoration: deco,
      child: DropdownButtonHideUnderline(
        child: DropdownButton2<String>(
            value: PeerSortType.values.contains(_sortType)
                ? _sortType
                : PeerSortType.remoteId,
            onChanged: (v) async {
              if (v != null) {
                setState(() => _sortType = v);
                await bind.setLocalFlutterConfig(
                  k: "peer-sorting",
                  v: _sortType,
                );
                peerSort.value = _sortType;
              }
            },
            customButton: Icon(
              Icons.sort,
              size: 18,
            ),
            dropdownStyleData: DropdownStyleData(
              decoration: BoxDecoration(
                color: Theme.of(context).cardColor,
                borderRadius: BorderRadius.circular(10),
              ),
              width: 160,
            ),
            items: [
              DropdownMenuItem<String>(
                alignment: Alignment.center,
                child: Text(
                  translate("Sort by"),
                  style: TextStyle(fontWeight: FontWeight.bold),
                ),
                enabled: false,
              ),
              ...PeerSortType.values
                  .map<DropdownMenuItem<String>>(
                    (String value) => DropdownMenuItem<String>(
                      value: value,
                      child: Row(
                        children: [
                          Icon(
                            value == _sortType
                                ? Icons.radio_button_checked_rounded
                                : Icons.radio_button_off_rounded,
                            size: 18,
                          ).paddingOnly(right: 12),
                          Text(
                            translate(value),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                  )
                  .toList(),
            ]),
      ),
    );
  }
}

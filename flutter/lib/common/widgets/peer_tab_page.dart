import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/common/widgets/my_group.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/common/widgets/animated_rotation_widget.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';

import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

class PeerTabPage extends StatefulWidget {
  const PeerTabPage({Key? key}) : super(key: key);
  @override
  State<PeerTabPage> createState() => _PeerTabPageState();
}

class _TabEntry {
  final Widget widget;
  final Function({dynamic hint}) load;
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
        ({dynamic hint}) => gFFI.abModel.pullAb(force: hint == null)),
    _TabEntry(
      MyGroup(
        menuPadding: _menuPadding(),
      ),
      ({dynamic hint}) => gFFI.groupModel.pull(force: hint == null),
    ),
  ];

  @override
  void initState() {
    final uiType = bind.getLocalFlutterOption(k: 'peer-card-ui-type');
    if (uiType != '') {
      peerCardUiType.value = int.parse(uiType) == PeerUiType.list.index
          ? PeerUiType.list
          : PeerUiType.grid;
    }
    hideAbTagsPanel.value =
        bind.mainGetLocalOption(key: "hideAbTagsPanel").isNotEmpty;
    super.initState();
  }

  Future<void> handleTabSelection(int tabIndex) async {
    if (tabIndex < entries.length) {
      gFFI.peerTabModel.setCurrentTab(tabIndex);
      entries[tabIndex].load(hint: false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);
    Widget selectionWrap(Widget widget) {
      return model.multiSelectionMode ? createMultiSelectionBar() : widget;
    }

    return Column(
      textBaseline: TextBaseline.ideographic,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          height: 32,
          child: Container(
            padding: isDesktop ? null : EdgeInsets.symmetric(horizontal: 2),
            child: selectionWrap(Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Expanded(child: _createSwitchBar(context)),
                const PeerSearchBar().marginOnly(right: isMobile ? 0 : 13),
                _createRefresh(),
                _createMultiSelection(),
                Offstage(
                    offstage: !isDesktop,
                    child: _createPeerViewTypeSwitch(context)),
                Offstage(
                  offstage: gFFI.peerTabModel.currentTab == 0,
                  child: PeerSortDropdown().marginOnly(left: 8),
                ),
                Offstage(
                  offstage: gFFI.peerTabModel.currentTab != 3,
                  child: InkWell(
                    child: Obx(() => Container(
                        padding: EdgeInsets.all(4.0),
                        decoration: hideAbTagsPanel.value
                            ? null
                            : BoxDecoration(
                                color: Theme.of(context).colorScheme.background,
                                borderRadius: BorderRadius.circular(6)),
                        child: Icon(
                          Icons.tag_rounded,
                          size: 18,
                        ))),
                    onTap: () async {
                      await bind.mainSetLocalOption(
                          key: "hideAbTagsPanel",
                          value: hideAbTagsPanel.value ? "" : "Y");
                      hideAbTagsPanel.value = !hideAbTagsPanel.value;
                    },
                  ).marginOnly(left: 8),
                ),
              ],
            )),
          ),
        ),
        _createPeersView(),
      ],
    );
  }

  Widget _createSwitchBar(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);

    return ListView(
        scrollDirection: Axis.horizontal,
        physics: NeverScrollableScrollPhysics(),
        children: model.indexs.map((t) {
          final selected = model.currentTab == t;
          final color = selected
              ? MyTheme.tabbar(context).selectedTextColor
              : MyTheme.tabbar(context).unSelectedTextColor
            ?..withOpacity(0.5);
          final hover = false.obs;
          final deco = BoxDecoration(
              color: Theme.of(context).colorScheme.background,
              borderRadius: BorderRadius.circular(6));
          final decoBorder = BoxDecoration(
              border: Border(
            bottom: BorderSide(width: 2, color: color!),
          ));
          return Obx(() => InkWell(
                child: Container(
                  decoration:
                      selected ? decoBorder : (hover.value ? deco : null),
                  child: Tooltip(
                    message:
                        model.tabTooltip(t, gFFI.groupModel.groupName.value),
                    child: Icon(model.tabIcon(t), color: color),
                  ).paddingSymmetric(horizontal: 4),
                ).paddingSymmetric(horizontal: 4),
                onTap: () async {
                  await handleTabSelection(t);
                  await bind.setLocalFlutterOption(
                      k: 'peer-tab-index', v: t.toString());
                },
                onHover: (value) => hover.value = value,
              ));
        }).toList());
  }

  Widget _createPeersView() {
    final model = Provider.of<PeerTabModel>(context);
    Widget child;
    if (model.indexs.isEmpty) {
      child = Center(
        child: Text(translate('Right click to select tabs')),
      );
    } else {
      if (model.indexs.contains(model.currentTab)) {
        child = entries[model.currentTab].widget;
      } else {
        Future.delayed(Duration.zero, () {
          model.setCurrentTab(model.indexs[0]);
        });
        child = entries[0].widget;
      }
    }
    return Expanded(
        child: child.marginSymmetric(vertical: isDesktop ? 12.0 : 6.0));
  }

  Widget _createRefresh() {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    return Offstage(
      offstage: gFFI.peerTabModel.currentTab < 3, // local tab can't see effect
      child: Container(
        padding: EdgeInsets.all(4.0),
        child: AnimatedRotationWidget(
            onPressed: () {
              if (gFFI.peerTabModel.currentTab < entries.length) {
                entries[gFFI.peerTabModel.currentTab].load();
              }
            },
            child: RotatedBox(
                quarterTurns: 2,
                child: Icon(
                  Icons.refresh,
                  size: 18,
                  color: textColor,
                ))),
      ),
    );
  }

  Widget _createPeerViewTypeSwitch(BuildContext context) {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    final types = [PeerUiType.grid, PeerUiType.list];
    final hover = false.obs;
    final deco = BoxDecoration(
      color: Theme.of(context).colorScheme.background,
      borderRadius: BorderRadius.circular(5),
    );

    return Obx(
      () => Container(
        padding: EdgeInsets.all(4.0),
        decoration: hover.value ? deco : null,
        child: InkWell(
            onHover: (value) => hover.value = value,
            onTap: () async {
              final type = types.elementAt(
                  peerCardUiType.value == types.elementAt(0) ? 1 : 0);
              await bind.setLocalFlutterOption(
                  k: 'peer-card-ui-type', v: type.index.toString());
              peerCardUiType.value = type;
            },
            child: Icon(
              peerCardUiType.value == PeerUiType.grid
                  ? Icons.view_list_rounded
                  : Icons.grid_view_rounded,
              size: 18,
              color: textColor,
            )),
      ),
    );
  }

  Widget _createMultiSelection() {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    final model = Provider.of<PeerTabModel>(context);
    if (model.currentTabCachedPeers.isEmpty) return Offstage();
    return Container(
      padding: EdgeInsets.all(4.0),
      child: InkWell(
        onTap: () {
          model.setMultiSelectionMode(true);
        },
        child: Icon(
          IconFont.checkbox,
          size: 18,
          color: textColor,
        ),
      ),
    );
  }

  Widget createMultiSelectionBar() {
    final model = Provider.of<PeerTabModel>(context);
    return Row(
      children: [
        deleteSelection(),
        addSelectionToFav(),
        addSelectionToAb(),
        editSelectionTags(),
        Expanded(child: Container()),
        selectionCount(model.selectedPeers.length),
        selectAll(),
        closeSelection(),
      ],
    );
  }

  Widget deleteSelection() {
    final model = Provider.of<PeerTabModel>(context);
    return InkWell(
        onTap: () {
          onSubmit() async {
            final peers = model.selectedPeers;
            switch (model.currentTab) {
              case 0:
                peers.map((p) async {
                  await bind.mainRemovePeer(id: p.id);
                }).toList();
                await bind.mainLoadRecentPeers();
                break;
              case 1:
                final favs = (await bind.mainGetFav()).toList();
                peers.map((p) {
                  favs.remove(p.id);
                }).toList();
                await bind.mainStoreFav(favs: favs);
                await bind.mainLoadFavPeers();
                break;
              case 2:
                peers.map((p) async {
                  await bind.mainRemoveDiscovered(id: p.id);
                }).toList();
                await bind.mainLoadLanPeers();
                break;
              case 3:
                gFFI.abModel.deletePeers(peers.map((p) => p.id).toList());
                await gFFI.abModel.pushAb();
                break;
              default:
                break;
            }
            gFFI.peerTabModel.setMultiSelectionMode(false);
            showToast(translate('Successful'));
          }

          deletePeerConfirmDialog(onSubmit, translate('Delete'));
        },
        child: Tooltip(
            message: translate('Delete'),
            child: Icon(Icons.delete, color: Colors.red)));
  }

  Widget addSelectionToFav() {
    final model = Provider.of<PeerTabModel>(context);
    return Offstage(
      offstage:
          model.currentTab != PeerTabIndex.recent.index, // show based on recent
      child: InkWell(
        onTap: () async {
          final peers = model.selectedPeers;
          final favs = (await bind.mainGetFav()).toList();
          for (var p in peers) {
            if (!favs.contains(p.id)) {
              favs.add(p.id);
            }
          }
          await bind.mainStoreFav(favs: favs);
          model.setMultiSelectionMode(false);
          showToast(translate('Successful'));
        },
        child: Tooltip(
                message: translate('Add to Favorites'),
                child: Icon(model.icons[PeerTabIndex.fav.index]))
            .marginOnly(left: isMobile ? 15 : 10),
      ),
    );
  }

  Widget addSelectionToAb() {
    final model = Provider.of<PeerTabModel>(context);
    return Offstage(
      offstage:
          !gFFI.userModel.isLogin || model.currentTab == PeerTabIndex.ab.index,
      child: InkWell(
        onTap: () {
          final peers = model.selectedPeers;
          gFFI.abModel.addPeers(peers);
          gFFI.abModel.pushAb();
          model.setMultiSelectionMode(false);
          showToast(translate('Successful'));
        },
        child: Tooltip(
                message: translate('Add to Address Book'),
                child: Icon(model.icons[PeerTabIndex.ab.index]))
            .marginOnly(left: isMobile ? 15 : 10),
      ),
    );
  }

  Widget editSelectionTags() {
    final model = Provider.of<PeerTabModel>(context);
    return Offstage(
      offstage: !gFFI.userModel.isLogin ||
          model.currentTab != PeerTabIndex.ab.index ||
          gFFI.abModel.tags.isEmpty,
      child: InkWell(
              onTap: () {
                editAbTagDialog(List.empty(), (selectedTags) async {
                  final peers = model.selectedPeers;
                  gFFI.abModel.changeTagForPeers(
                      peers.map((p) => p.id).toList(), selectedTags);
                  gFFI.abModel.pushAb();
                  model.setMultiSelectionMode(false);
                  showToast(translate('Successful'));
                });
              },
              child: Tooltip(
                  message: translate('Edit Tag'), child: Icon(Icons.tag)))
          .marginOnly(left: isMobile ? 15 : 10),
    );
  }

  Widget selectionCount(int count) {
    return Align(
      alignment: Alignment.center,
      child: Text('$count selected'),
    );
  }

  Widget selectAll() {
    final model = Provider.of<PeerTabModel>(context);
    return Offstage(
      offstage:
          model.selectedPeers.length >= model.currentTabCachedPeers.length,
      child: InkWell(
        onTap: () {
          model.selectAll();
        },
        child: Tooltip(
                message: translate('Select All'), child: Icon(Icons.select_all))
            .marginOnly(left: 10),
      ),
    );
  }

  Widget closeSelection() {
    final model = Provider.of<PeerTabModel>(context);
    return InkWell(
            onTap: () {
              model.setMultiSelectionMode(false);
            },
            child:
                Tooltip(message: translate('Close'), child: Icon(Icons.clear)))
        .marginOnly(left: 10);
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
  @override
  void initState() {
    if (!PeerSortType.values.contains(peerSort.value)) {
      peerSort.value = PeerSortType.remoteId;
      bind.setLocalFlutterOption(
        k: "peer-sorting",
        v: peerSort.value,
      );
    }
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final style = TextStyle(
        color: Theme.of(context).textTheme.titleLarge?.color,
        fontSize: MenuConfig.fontSize,
        fontWeight: FontWeight.normal);
    List<PopupMenuEntry> items = List.empty(growable: true);
    items.add(PopupMenuItem(
        height: 36,
        enabled: false,
        child: Text(translate("Sort by"), style: style)));
    for (var e in PeerSortType.values) {
      items.add(PopupMenuItem(
          height: 36,
          child: Obx(() => Center(
                child: SizedBox(
                  height: 36,
                  child: getRadio(
                      Text(translate(e), style: style), e, peerSort.value,
                      dense: true, (String? v) async {
                    if (v != null) {
                      peerSort.value = v;
                      await bind.setLocalFlutterOption(
                        k: "peer-sorting",
                        v: peerSort.value,
                      );
                    }
                  }),
                ),
              ))));
    }

    var menuPos = RelativeRect.fromLTRB(0, 0, 0, 0);
    return InkWell(
      child: Icon(
        Icons.sort_rounded,
        size: 18,
      ),
      onTapDown: (details) {
        final x = details.globalPosition.dx;
        final y = details.globalPosition.dy;
        menuPos = RelativeRect.fromLTRB(x, y, x, y);
      },
      onTap: () => showMenu(
        context: context,
        position: menuPos,
        items: items,
        elevation: 8,
      ),
    );
  }
}

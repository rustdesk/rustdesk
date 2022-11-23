import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

class PeerTabPage extends StatefulWidget {
  final List<String> tabs;
  final List<Widget> children;
  const PeerTabPage({required this.tabs, required this.children, Key? key})
      : super(key: key);
  @override
  State<PeerTabPage> createState() => _PeerTabPageState();
}

class _PeerTabPageState extends State<PeerTabPage>
    with SingleTickerProviderStateMixin {
  final RxInt _tabIndex = 0.obs;

  @override
  void initState() {
    setPeer();
    super.initState();
  }

  setPeer() {
    final index = bind.getLocalFlutterConfig(k: 'peer-tab-index');
    if (index != '') {
      _tabIndex.value = int.parse(index);
    }

    final uiType = bind.getLocalFlutterConfig(k: 'peer-card-ui-type');
    if (uiType != '') {
      peerCardUiType.value = int.parse(uiType) == PeerUiType.list.index
          ? PeerUiType.list
          : PeerUiType.grid;
    }
  }

  // hard code for now
  Future<void> _handleTabSelection(int index) async {
    _tabIndex.value = index;
    await bind.setLocalFlutterConfig(k: 'peer-tab-index', v: index.toString());
    switch (index) {
      case 0:
        bind.mainLoadRecentPeers();
        break;
      case 1:
        bind.mainLoadFavPeers();
        break;
      case 2:
        bind.mainDiscover();
        break;
      case 3:

        /// AddressBook initState will refresh ab state
        break;
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
                  Expanded(child: _createSwitchBar(context)),
                  const SizedBox(width: 10),
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
    return ListView(
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        controller: ScrollController(),
        children: super.widget.tabs.asMap().entries.map((t) {
          return Obx(() => InkWell(
                child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: _tabIndex.value == t.key
                          ? Theme.of(context).backgroundColor
                          : null,
                      borderRadius: BorderRadius.circular(isDesktop ? 2 : 6),
                    ),
                    child: Align(
                      alignment: Alignment.center,
                      child: Text(
                        t.value,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                            height: 1,
                            fontSize: 14,
                            color:
                                _tabIndex.value == t.key ? textColor : textColor
                                  ?..withOpacity(0.5)),
                      ),
                    )),
                onTap: () async => await _handleTabSelection(t.key),
              ));
        }).toList());
  }

  Widget _createPeersView() {
    final verticalMargin = isDesktop ? 12.0 : 6.0;
    return Expanded(
      child: Obx(() => widget
              .children[_tabIndex.value]) //: (to) => _tabIndex.value = to)
          .marginSymmetric(vertical: verticalMargin),
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

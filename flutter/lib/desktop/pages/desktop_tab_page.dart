import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/get.dart';

class DesktopTabPage extends StatefulWidget {
  const DesktopTabPage({Key? key}) : super(key: key);

  @override
  State<DesktopTabPage> createState() => _DesktopTabPageState();
}

class _DesktopTabPageState extends State<DesktopTabPage>
    with TickerProviderStateMixin {
  late Rx<TabController> tabController;
  late RxList<TabInfo> tabs;
  static final Rx<int> _selected = 0.obs;

  @override
  void initState() {
    super.initState();
    tabs = RxList.from([
      TabInfo(label: kTabLabelHomePage, icon: Icons.home_sharp, closable: false)
    ], growable: true);
    tabController =
        TabController(length: tabs.length, vsync: this, initialIndex: 0).obs;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          Obx((() => DesktopTabBar(
                controller: tabController,
                tabs: tabs.toList(),
                onTabClose: onTabClose,
                selected: _selected,
                dark: isDarkTheme(),
                mainTab: true,
                onMenu: onTabbarMenu,
              ))),
          Obx((() => Expanded(
                child: TabBarView(
                    controller: tabController.value,
                    children: tabs.map((tab) {
                      switch (tab.label) {
                        case kTabLabelHomePage:
                          return DesktopHomePage(key: ValueKey(tab.label));
                        case kTabLabelSettingPage:
                          return DesktopSettingPage(key: ValueKey(tab.label));
                        default:
                          return Container();
                      }
                    }).toList()),
              ))),
        ],
      ),
    );
  }

  void onTabClose(String label) {
    tabs.removeWhere((tab) => tab.label == label);
    tabController.value = TabController(
        length: tabs.length,
        vsync: this,
        initialIndex: max(0, tabs.length - 1));
  }

  void onTabbarMenu() {
    int index = tabs.indexWhere((tab) => tab.label == kTabLabelSettingPage);
    if (index >= 0) {
      tabController.value.animateTo(index, duration: Duration.zero);
      _selected.value = index;
    } else {
      tabs.add(TabInfo(label: kTabLabelSettingPage, icon: Icons.settings));
      tabController.value = TabController(
          length: tabs.length, vsync: this, initialIndex: tabs.length - 1);
      tabController.value.animateTo(tabs.length - 1, duration: Duration.zero);
      _selected.value = tabs.length - 1;
    }
  }
}

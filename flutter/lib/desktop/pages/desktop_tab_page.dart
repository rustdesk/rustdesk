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
          DesktopTabBar(
            controller: tabController,
            tabs: tabs,
            onTabClose: onTabClose,
            selected: _selected,
            dark: isDarkTheme(),
            mainTab: true,
            onAddSetting: onAddSetting,
          ),
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
    DesktopTabBar.onClose(this, tabController, tabs, label);
  }

  void onAddSetting() {
    DesktopTabBar.onAdd(this, tabController, tabs, _selected,
        TabInfo(label: kTabLabelSettingPage, icon: Icons.settings));
  }
}

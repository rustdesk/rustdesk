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

class _DesktopTabPageState extends State<DesktopTabPage> {
  late RxList<TabInfo> tabs;

  @override
  void initState() {
    super.initState();
    tabs = RxList.from([
      TabInfo(
          label: kTabLabelHomePage,
          selectedIcon: Icons.home_sharp,
          unselectedIcon: Icons.home_outlined,
          closable: false)
    ], growable: true);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
          border: Border.all(color: MyTheme.color(context).border!)),
      child: Scaffold(
        backgroundColor: MyTheme.color(context).bg,
        body: Column(
          children: [
            DesktopTabBar(
              tabs: tabs,
              dark: isDarkTheme(),
              mainTab: true,
              onAddSetting: onAddSetting,
            ),
            Obx((() => Expanded(
                  child: PageView(
                      controller: DesktopTabBar.controller.value,
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
      ),
    );
  }

  void onAddSetting() {
    DesktopTabBar.onAdd(
        tabs,
        TabInfo(
            label: kTabLabelSettingPage,
            selectedIcon: Icons.build_sharp,
            unselectedIcon: Icons.build_outlined));
  }
}

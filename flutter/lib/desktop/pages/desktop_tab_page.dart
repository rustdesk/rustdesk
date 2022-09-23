import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

class DesktopTabPage extends StatefulWidget {
  const DesktopTabPage({Key? key}) : super(key: key);

  @override
  State<DesktopTabPage> createState() => _DesktopTabPageState();
}

class _DesktopTabPageState extends State<DesktopTabPage> {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);

  @override
  void initState() {
    super.initState();
    tabController.add(TabInfo(
        key: kTabLabelHomePage,
        label: kTabLabelHomePage,
        selectedIcon: Icons.home_sharp,
        unselectedIcon: Icons.home_outlined,
        closable: false,
        page: DesktopHomePage(
          key: const ValueKey(kTabLabelHomePage),
        )));
  }

  @override
  Widget build(BuildContext context) {
    RxBool fullscreen = false.obs;
    Get.put(fullscreen, tag: 'fullscreen');
    final tabWidget = Container(
      child: Overlay(initialEntries: [
        OverlayEntry(builder: (context) {
          gFFI.dialogManager.setOverlayState(Overlay.of(context));
          return Scaffold(
              backgroundColor: Theme.of(context).backgroundColor,
              body: DesktopTab(
                controller: tabController,
                tail: ActionIcon(
                  message: 'Settings',
                  icon: IconFont.menu,
                  onTap: onAddSetting,
                  isClose: false,
                ),
              ));
        })
      ]),
    );
    return Platform.isMacOS
        ? tabWidget
        : Obx(() => DragToResizeArea(
            resizeEdgeSize:
                fullscreen.value ? kFullScreenEdgeSize : kWindowEdgeSize,
            child: tabWidget));
  }

  void onAddSetting() {
    tabController.add(TabInfo(
        key: kTabLabelSettingPage,
        label: kTabLabelSettingPage,
        selectedIcon: Icons.build_sharp,
        unselectedIcon: Icons.build_outlined,
        page: DesktopSettingPage(key: const ValueKey(kTabLabelSettingPage))));
  }
}

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
    final dark = isDarkTheme();
    RxBool fullscreen = false.obs;
    Get.put(fullscreen, tag: 'fullscreen');
    return Obx(() => DragToResizeArea(
          resizeEdgeSize: fullscreen.value ? 1.0 : 8.0,
          child: Container(
            decoration: BoxDecoration(
                border: Border.all(color: MyTheme.color(context).border!)),
            child: Scaffold(
                backgroundColor: MyTheme.color(context).bg,
                body: DesktopTab(
                  controller: tabController,
                  theme: dark ? TarBarTheme.dark() : TarBarTheme.light(),
                  tail: ActionIcon(
                    message: 'Settings',
                    icon: IconFont.menu,
                    theme: dark ? TarBarTheme.dark() : TarBarTheme.light(),
                    onTap: onAddSetting,
                    is_close: false,
                  ),
                )),
          ),
        ));
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

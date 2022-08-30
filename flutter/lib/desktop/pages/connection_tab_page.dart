import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage> {
  final tabController = Get.put(DesktopTabController());
  static final IconData selectedIcon = Icons.desktop_windows_sharp;
  static final IconData unselectedIcon = Icons.desktop_windows_outlined;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    final RxBool fullscreen = Get.find(tag: 'fullscreen');
    if (params['id'] != null) {
      tabController.add(TabInfo(
          key: params['id'],
          label: params['id'],
          selectedIcon: selectedIcon,
          unselectedIcon: unselectedIcon,
          page: Obx(() => RemotePage(
	        key: ValueKey(params['id']),
                id: params['id'],
                tabBarHeight:
                    fullscreen.isTrue ? 0 : kDesktopRemoteTabBarHeight,
              ))));
    }
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemove = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");

      final RxBool fullscreen = Get.find(tag: 'fullscreen');
      // for simplify, just replace connectionId
      if (call.method == "new_remote_desktop") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        window_on_top(windowId());
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            page: Obx(() => RemotePage(
                  key: ValueKey(id),
                  id: id,
                  tabBarHeight:
                      fullscreen.isTrue ? 0 : kDesktopRemoteTabBarHeight,
                ))));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = isDarkTheme() ? TarBarTheme.dark() : TarBarTheme.light();
    final RxBool fullscreen = Get.find(tag: 'fullscreen');
    return Obx(() => SubWindowDragToResizeArea(
          resizeEdgeSize: fullscreen.value ? 1.0 : 8.0,
          windowId: windowId(),
          child: Container(
            decoration: BoxDecoration(
                border: Border.all(color: MyTheme.color(context).border!)),
            child: Scaffold(
                backgroundColor: MyTheme.color(context).bg,
                body: Obx(() => DesktopTab(
                      controller: tabController,
                      theme: theme,
                      isMainWindow: false,
                      showTabBar: fullscreen.isFalse,
                      onClose: () {
                        tabController.clear();
                      },
                      tail: AddButton(
                        theme: theme,
                      ).paddingOnly(left: 10),
                      pageViewBuilder: (pageView) {
                        WindowController.fromWindowId(windowId())
                            .setFullscreen(fullscreen.isTrue);
                        return pageView;
                      },
                    ))),
          ),
        ));
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).hide();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

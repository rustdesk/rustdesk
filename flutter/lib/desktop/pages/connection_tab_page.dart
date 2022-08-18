import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../../models/model.dart';

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage> {
  // refactor List<int> when using multi-tab
  // this singleton is only for test
  RxList<TabInfo> tabs = RxList<TabInfo>.empty(growable: true);
  static final Rx<String> _fullscreenID = "".obs;
  final IconData selectedIcon = Icons.desktop_windows_sharp;
  final IconData unselectedIcon = Icons.desktop_windows_outlined;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    if (params['id'] != null) {
      tabs.add(TabInfo(
          label: params['id'],
          selectedIcon: selectedIcon,
          unselectedIcon: unselectedIcon));
    }
  }

  @override
  void initState() {
    super.initState();
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_remote_desktop") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        window_on_top(windowId());
        DesktopTabBar.onAdd(
            tabs,
            TabInfo(
                label: id,
                selectedIcon: selectedIcon,
                unselectedIcon: unselectedIcon));
      } else if (call.method == "onDestroy") {
        print(
            "executing onDestroy hook, closing ${tabs.map((tab) => tab.label).toList()}");
        tabs.forEach((tab) {
          final tag = '${tab.label}';
          ffi(tag).close().then((_) {
            Get.delete<FFI>(tag: tag);
          });
        });
        Get.back();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          Obx(() => Visibility(
              visible: _fullscreenID.value.isEmpty,
              child: DesktopTabBar(
                tabs: tabs,
                onTabClose: onRemoveId,
                dark: isDarkTheme(),
                mainTab: false,
              ))),
          Expanded(child: Obx(() {
            WindowController.fromWindowId(windowId())
                .setFullscreen(_fullscreenID.value.isNotEmpty);
            return PageView(
                controller: DesktopTabBar.controller.value,
                children: tabs
                    .map((tab) => RemotePage(
                          key: ValueKey(tab.label),
                          id: tab.label,
                          tabBarHeight: _fullscreenID.value.isNotEmpty
                              ? 0
                              : kDesktopRemoteTabBarHeight,
                          fullscreenID: _fullscreenID,
                        )) //RemotePage(key: ValueKey(e), id: e))
                    .toList());
          })),
        ],
      ),
    );
  }

  void onRemoveId(String id) {
    ffi(id).close();
    if (tabs.length == 0) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

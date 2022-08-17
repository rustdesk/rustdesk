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

class _ConnectionTabPageState extends State<ConnectionTabPage>
    with TickerProviderStateMixin {
  // refactor List<int> when using multi-tab
  // this singleton is only for test
  RxList<TabInfo> tabs = RxList<TabInfo>.empty(growable: true);
  late Rx<TabController> tabController;
  static final Rx<int> _selected = 0.obs;
  static final Rx<String> _fullscreenID = "".obs;
  IconData icon = Icons.desktop_windows_sharp;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    if (params['id'] != null) {
      tabs.add(TabInfo(label: params['id'], icon: icon));
    }
  }

  @override
  void initState() {
    super.initState();
    tabController = TabController(length: tabs.length, vsync: this).obs;
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_remote_desktop") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        window_on_top(windowId());
        DesktopTabBar.onAdd(this, tabController, tabs, _selected,
            TabInfo(label: id, icon: icon));
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
                controller: tabController,
                tabs: tabs,
                onTabClose: onRemoveId,
                selected: _selected,
                dark: isDarkTheme(),
                mainTab: false,
              ))),
          Expanded(child: Obx(() {
            WindowController.fromWindowId(windowId())
                .setFullscreen(_fullscreenID.value.isNotEmpty);
            return TabBarView(
                controller: tabController.value,
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
    DesktopTabBar.onClose(this, tabController, tabs, id);
    ffi(id).close();
    if (tabs.length == 0) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

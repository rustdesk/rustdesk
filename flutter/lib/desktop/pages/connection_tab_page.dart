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
  final tabController = Get.put(DesktopTabController());
  static final Rx<String> _fullscreenID = "".obs;
  static final IconData selectedIcon = Icons.desktop_windows_sharp;
  static final IconData unselectedIcon = Icons.desktop_windows_outlined;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    if (params['id'] != null) {
      tabController.add(TabInfo(
          key: params['id'],
          label: params['id'],
          selectedIcon: selectedIcon,
          unselectedIcon: unselectedIcon,
          page: RemotePage(
            key: ValueKey(params['id']),
            id: params['id'],
            tabBarHeight:
                _fullscreenID.value.isNotEmpty ? 0 : kDesktopRemoteTabBarHeight,
            fullscreenID: _fullscreenID,
          )));
    }
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemove = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
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
            page: RemotePage(
              key: ValueKey(id),
              id: id,
              tabBarHeight: _fullscreenID.value.isNotEmpty
                  ? 0
                  : kDesktopRemoteTabBarHeight,
              fullscreenID: _fullscreenID,
            )));
      } else if (call.method == "onDestroy") {
        tabController.state.value.tabs.forEach((tab) {
          print("executing onDestroy hook, closing ${tab.label}}");
          final tag = tab.label;
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
    final theme = isDarkTheme() ? TarBarTheme.dark() : TarBarTheme.light();
    return SubWindowDragToResizeArea(
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
                  showTabBar: _fullscreenID.value.isEmpty,
                  tail: AddButton(
                    theme: theme,
                  ).paddingOnly(left: 10),
                  pageViewBuilder: (pageView) {
                    WindowController.fromWindowId(windowId())
                        .setFullscreen(_fullscreenID.value.isNotEmpty);
                    return pageView;
                  },
                ))),
      ),
    );
  }

  void onRemoveId(String id) {
    ffi(id).close();
    if (tabController.state.value.tabs.length == 0) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

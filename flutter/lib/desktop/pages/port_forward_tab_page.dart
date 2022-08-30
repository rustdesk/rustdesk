import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/port_forward_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

class PortForwardTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const PortForwardTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<PortForwardTabPage> createState() => _PortForwardTabPageState(params);
}

class _PortForwardTabPageState extends State<PortForwardTabPage> {
  final tabController = Get.put(DesktopTabController());

  static final IconData selectedIcon = Icons.forward_sharp;
  static final IconData unselectedIcon = Icons.forward_outlined;

  _PortForwardTabPageState(Map<String, dynamic> params) {
    tabController.add(TabInfo(
        key: params['id'],
        label: params['id'],
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        page: PortForwardPage(
          key: ValueKey(params['id']),
          id: params['id'],
          isRDP: params['isRDP'],
        )));
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemove = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      debugPrint(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_port_forward") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        final isRDP = args['isRDP'];
        window_on_top(windowId());
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            page: PortForwardPage(id: id, isRDP: isRDP)));
      } else if (call.method == "onDestroy") {
        tabController.clear();
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
            body: DesktopTab(
              controller: tabController,
              theme: theme,
              isMainWindow: false,
              onClose: () {
                tabController.clear();
              },
              tail: AddButton(
                theme: theme,
              ).paddingOnly(left: 10),
            )),
      ),
    );
  }

  void onRemoveId(String id) {
    ffi("pf_$id").close();
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).hide();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

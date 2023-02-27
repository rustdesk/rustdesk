import 'dart:convert';
import 'dart:io';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
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
  late final DesktopTabController tabController;
  late final bool isRDP;

  static const IconData selectedIcon = Icons.forward_sharp;
  static const IconData unselectedIcon = Icons.forward_outlined;

  _PortForwardTabPageState(Map<String, dynamic> params) {
    isRDP = params['isRDP'];
    tabController =
        Get.put(DesktopTabController(tabType: DesktopTabType.portForward));
    tabController.onSelected = (_, id) {
      WindowController.fromWindowId(windowId())
          .setTitle(getWindowNameWithId(id));
    };
    tabController.add(TabInfo(
        key: params['id'],
        label: params['id'],
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        page: PortForwardPage(
          key: ValueKey(params['id']),
          id: params['id'],
          isRDP: isRDP,
          forceRelay: params['forceRelay'],
        )));
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemoved = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      debugPrint(
          "[Port Forward] call ${call.method} with args ${call.arguments} from window $fromWindowId");
      // for simplify, just replace connectionId
      if (call.method == "new_port_forward") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        final isRDP = args['isRDP'];
        window_on_top(windowId());
        if (tabController.state.value.tabs.indexWhere((e) => e.key == id) >=
            0) {
          debugPrint("port forward $id exists");
          return;
        }
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            page: PortForwardPage(
              key: ValueKey(args['id']),
              id: id,
              isRDP: isRDP,
              forceRelay: args['forceRelay'],
            )));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.PortForward, windowId: windowId());
    });
  }

  @override
  Widget build(BuildContext context) {
    final tabWidget = Container(
      decoration: BoxDecoration(
          border: Border.all(color: MyTheme.color(context).border!)),
      child: Scaffold(
          backgroundColor: Theme.of(context).colorScheme.background,
          body: DesktopTab(
            controller: tabController,
            onWindowCloseButton: () async {
              tabController.clear();
              return true;
            },
            tail: AddButton().paddingOnly(left: 10),
            labelGetter: DesktopTab.labelGetterAlias,
          )),
    );
    return Platform.isMacOS
        ? tabWidget
        : Obx(
          () => SubWindowDragToResizeArea(
              child: tabWidget,
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              windowId: stateGlobal.windowId,
            ),
        );
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/usbip_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

class UsbipTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const UsbipTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<UsbipTabPage> createState() => _UsbipTabPageState(params);
}

class _UsbipTabPageState extends State<UsbipTabPage> {
  late final DesktopTabController tabController;

  static const IconData selectedIcon = Icons.usb;
  static const IconData unselectedIcon = Icons.usb_outlined;

  _UsbipTabPageState(Map<String, dynamic> params) {
    tabController =
        Get.put(DesktopTabController(tabType: DesktopTabType.portForward));
    tabController.onSelected = (id) {
      WindowController.fromWindowId(windowId())
          .setTitle(getWindowNameWithId(id));
    };
    tabController.onRemoved = (_, id) => onRemoveId(id);
    tabController.add(TabInfo(
        key: params['id'],
        label: params['id'],
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        page: UsbipPage(
          key: ValueKey(params['id']),
          id: params['id'],
          password: params['password'],
          isSharedPassword: params['isSharedPassword'],
          tabController: tabController,
          forceRelay: params['forceRelay'],
          connToken: params['connToken'],
        )));
  }

  @override
  void initState() {
    super.initState();

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      debugPrint(
          "[Usbip] call ${call.method} with args ${call.arguments} from window $fromWindowId");
      if (call.method == kWindowEventNewRemoteUsb) {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        windowOnTop(windowId());
        if (tabController.state.value.tabs.indexWhere((e) => e.key == id) >=
            0) {
          debugPrint("usbip $id exists");
          return;
        }
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            page: UsbipPage(
              key: ValueKey(args['id']),
              id: id,
              password: args['password'],
              isSharedPassword: args['isSharedPassword'],
              tabController: tabController,
              forceRelay: args['forceRelay'],
              connToken: args['connToken'],
            )));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.RemoteUsb, windowId: windowId());
    });
  }

  @override
  Widget build(BuildContext context) {
    final child = Scaffold(
      backgroundColor: Theme.of(context).colorScheme.background,
      body: DesktopTab(
        controller: tabController,
        onWindowCloseButton: () async {
          tabController.clear();
          return true;
        },
        tail: AddButton(),
        selectedBorderColor: MyTheme.accent,
        labelGetter: DesktopTab.tablabelGetter,
      ),
    );
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(
            context,
            Scaffold(
                backgroundColor: Theme.of(context).colorScheme.background,
                body: child),
          )
        : workaroundWindowBorder(
            context,
            Container(
              decoration: BoxDecoration(
                  border: Border.all(color: MyTheme.color(context).border!)),
              child: child,
            ));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : Obx(
            () => SubWindowDragToResizeArea(
              child: tabWidget,
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              enableResizeEdges: subWindowManagerEnableResizeEdges,
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

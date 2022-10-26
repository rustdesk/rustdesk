import 'dart:convert';
import 'dart:io';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';

import '../../models/platform_model.dart';

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage> {
  final tabController =
      Get.put(DesktopTabController(tabType: DesktopTabType.remoteScreen));
  static const IconData selectedIcon = Icons.desktop_windows_sharp;
  static const IconData unselectedIcon = Icons.desktop_windows_outlined;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    RemoteCountState.init();
    final RxBool fullscreen = Get.find(tag: 'fullscreen');
    final peerId = params['id'];
    if (peerId != null) {
      ConnectionTypeState.init(peerId);
      tabController.add(TabInfo(
          key: peerId,
          label: peerId,
          selectedIcon: selectedIcon,
          unselectedIcon: unselectedIcon,
          onTabCloseButton: () => tabController.closeBy(peerId),
          page: Obx(() => RemotePage(
                key: ValueKey(peerId),
                id: peerId,
                windowId: windowId(),
                tabBarHeight:
                    fullscreen.isTrue ? 0 : kDesktopRemoteTabBarHeight,
                windowBorderWidth: fullscreen.isTrue ? 0 : kWindowBorderWidth,
              ))));
      _update_remote_count();
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
        ConnectionTypeState.init(id);
        window_on_top(windowId());
        ConnectionTypeState.init(id);
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            onTabCloseButton: () => tabController.closeBy(id),
            page: Obx(() => RemotePage(
                  key: ValueKey(id),
                  id: id,
                  windowId: windowId(),
                  tabBarHeight:
                      fullscreen.isTrue ? 0 : kDesktopRemoteTabBarHeight,
                  windowBorderWidth: fullscreen.isTrue ? 0 : kWindowBorderWidth,
                ))));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
      _update_remote_count();
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.RemoteDesktop, windowId: windowId());
    });
  }

  @override
  Widget build(BuildContext context) {
    final RxBool fullscreen = Get.find(tag: 'fullscreen');
    final tabWidget = Container(
      decoration: BoxDecoration(
          border: Border.all(
              color: MyTheme.color(context).border!,
              width: kWindowBorderWidth)),
      child: Scaffold(
          backgroundColor: Theme.of(context).backgroundColor,
          body: Obx(() => DesktopTab(
                controller: tabController,
                showTabBar: fullscreen.isFalse,
                onWindowCloseButton: handleWindowCloseButton,
                tail: const AddButton().paddingOnly(left: 10),
                pageViewBuilder: (pageView) {
                  WindowController.fromWindowId(windowId())
                      .setFullscreen(fullscreen.isTrue);
                  return pageView;
                },
                tabBuilder: (key, icon, label, themeConf) => Obx(() {
                  final connectionType = ConnectionTypeState.find(key);
                  if (!connectionType.isValid()) {
                    return Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        icon,
                        label,
                      ],
                    );
                  } else {
                    final msgDirect = translate(
                        connectionType.direct.value == ConnectionType.strDirect
                            ? 'Direct Connection'
                            : 'Relay Connection');
                    final msgSecure = translate(
                        connectionType.secure.value == ConnectionType.strSecure
                            ? 'Secure Connection'
                            : 'Insecure Connection');
                    return Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        icon,
                        Tooltip(
                          message: '$msgDirect\n$msgSecure',
                          child: SvgPicture.asset(
                            'assets/${connectionType.secure.value}${connectionType.direct.value}.svg',
                            width: themeConf.iconSize,
                            height: themeConf.iconSize,
                          ).paddingOnly(right: 5),
                        ),
                        label,
                      ],
                    );
                  }
                }),
              ))),
    );
    return Platform.isMacOS
        ? tabWidget
        : Obx(() => SubWindowDragToResizeArea(
            resizeEdgeSize:
                fullscreen.value ? kFullScreenEdgeSize : kWindowEdgeSize,
            windowId: windowId(),
            child: tabWidget));
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).hide();
    }
    ConnectionTypeState.delete(id);
    _update_remote_count();
  }

  int windowId() {
    return widget.params["windowId"];
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.length;
    if (connLength <= 1) {
      tabController.clear();
      return true;
    } else {
      final opt = "enable-confirm-closing-tabs";
      final bool res;
      if (!option2bool(opt, await bind.mainGetOption(key: opt))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        tabController.clear();
      }
      return res;
    }
  }

  _update_remote_count() =>
      RemoteCountState.find().value = tabController.length;
}

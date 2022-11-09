import 'dart:convert';
import 'dart:io';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../../models/platform_model.dart';

/// File Transfer for multi tabs
class FileManagerTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const FileManagerTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<FileManagerTabPage> createState() => _FileManagerTabPageState(params);
}

class _FileManagerTabPageState extends State<FileManagerTabPage> {
  DesktopTabController get tabController => Get.find<DesktopTabController>();

  static const IconData selectedIcon = Icons.file_copy_sharp;
  static const IconData unselectedIcon = Icons.file_copy_outlined;

  _FileManagerTabPageState(Map<String, dynamic> params) {
    Get.put(DesktopTabController(tabType: DesktopTabType.fileTransfer));
    tabController.add(TabInfo(
        key: params['id'],
        label: params['id'],
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        onTabCloseButton: () => () => tabController.closeBy(params['id']),
        page: FileManagerPage(key: ValueKey(params['id']), id: params['id'])));
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemoved = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "[FileTransfer] call ${call.method} with args ${call.arguments} from window $fromWindowId to ${windowId()}");
      // for simplify, just replace connectionId
      if (call.method == "new_file_transfer") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        window_on_top(windowId());
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            onTabCloseButton: () => tabController.closeBy(id),
            page: FileManagerPage(key: ValueKey(id), id: id)));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.FileTransfer, windowId: windowId());
    });
  }

  @override
  Widget build(BuildContext context) {
    final tabWidget = Container(
      decoration: BoxDecoration(
          border: Border.all(color: MyTheme.color(context).border!)),
      child: Scaffold(
          backgroundColor: Theme.of(context).backgroundColor,
          body: DesktopTab(
            controller: tabController,
            onWindowCloseButton: handleWindowCloseButton,
            tail: const AddButton().paddingOnly(left: 10),
          )),
    );
    return Platform.isMacOS
        ? tabWidget
        : SubWindowDragToResizeArea(
            child: tabWidget,
            resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
            windowId: stateGlobal.windowId,
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

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.state.value.tabs.length;
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
}

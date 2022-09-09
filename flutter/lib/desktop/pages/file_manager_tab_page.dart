import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../../mobile/widgets/dialog.dart';

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
        onTabCloseButton: () => handleTabCloseButton(params['id']),
        page: FileManagerPage(key: ValueKey(params['id']), id: params['id'])));
  }

  @override
  void initState() {
    super.initState();

    tabController.onRemove = (_, id) => onRemoveId(id);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId} to ${windowId()}");
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
            onTabCloseButton: () => handleTabCloseButton(id),
            page: FileManagerPage(key: ValueKey(id), id: id)));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return SubWindowDragToResizeArea(
      windowId: windowId(),
      child: Container(
        decoration: BoxDecoration(
            border: Border.all(color: MyTheme.color(context).border!)),
        child: Scaffold(
            backgroundColor: MyTheme.color(context).bg,
            body: DesktopTab(
              controller: tabController,
              onWindowCloseButton: handleWindowCloseButton,
              tail: const AddButton().paddingOnly(left: 10),
            )),
      ),
    );
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).hide();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }

  void handleTabCloseButton(String peerId) {
    final session = ffi('ft_$peerId');
    if (session.ffiModel.pi.hostname.isNotEmpty) {
      tabController.jumpBy(peerId);
      clientClose(session.dialogManager);
    } else {
      tabController.closeBy(peerId);
    }
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.state.value.tabs.length;
    if (connLength < 1) {
      return true;
    } else if (connLength == 1) {
      final currentConn = tabController.state.value.tabs[0];
      handleTabCloseButton(currentConn.key);
      return false;
    } else {
      final res = await closeConfirmDialog();
      if (res) {
        tabController.clear();
      }
      return res;
    }
  }
}

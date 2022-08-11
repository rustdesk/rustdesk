import 'dart:convert';
import 'dart:math';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

/// File Transfer for multi tabs
class FileManagerTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const FileManagerTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<FileManagerTabPage> createState() => _FileManagerTabPageState(params);
}

class _FileManagerTabPageState extends State<FileManagerTabPage>
    with TickerProviderStateMixin {
  // refactor List<int> when using multi-tab
  // this singleton is only for test
  var connectionIds = List<String>.empty(growable: true).obs;
  var initialIndex = 0;
  late Rx<TabController> tabController;
  static final Rx<int> _selected = 0.obs;

  _FileManagerTabPageState(Map<String, dynamic> params) {
    if (params['id'] != null) {
      connectionIds.add(params['id']);
    }
  }

  @override
  void initState() {
    super.initState();
    tabController =
        TabController(length: connectionIds.length, vsync: this).obs;
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_file_transfer") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        window_on_top(windowId());
        final indexOf = connectionIds.indexOf(id);
        if (indexOf >= 0) {
          initialIndex = indexOf;
          tabController.value.animateTo(initialIndex, duration: Duration.zero);
        } else {
          connectionIds.add(id);
          initialIndex = connectionIds.length - 1;
          tabController.value = TabController(
              length: connectionIds.length,
              initialIndex: initialIndex,
              vsync: this);
        }
        _selected.value = initialIndex;
      } else if (call.method == "onDestroy") {
        print("executing onDestroy hook, closing ${connectionIds}");
        connectionIds.forEach((id) {
          final tag = 'ft_${id}';
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
          Obx(
            () => DesktopTabBar(
              controller: tabController,
              tabs: connectionIds
                  .map((e) => TabInfo(label: e, icon: Icons.file_copy_sharp))
                  .toList(),
              onTabClose: onRemoveId,
              selected: _selected,
              dark: isDarkTheme(),
              mainTab: false,
            ),
          ),
          Expanded(
            child: Obx(
              () => TabBarView(
                  controller: tabController.value,
                  children: connectionIds
                      .map((e) => FileManagerPage(
                          key: ValueKey(e),
                          id: e)) //RemotePage(key: ValueKey(e), id: e))
                      .toList()),
            ),
          )
        ],
      ),
    );
  }

  void onRemoveId(String id) {
    final indexOf = connectionIds.indexOf(id);
    if (indexOf == -1) {
      return;
    }
    connectionIds.removeAt(indexOf);
    initialIndex = max(0, initialIndex - 1);
    tabController.value = TabController(
        length: connectionIds.length, initialIndex: initialIndex, vsync: this);
    if (connectionIds.length == 0) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }
}

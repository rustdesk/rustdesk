import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/titlebar_widget.dart';
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
    with SingleTickerProviderStateMixin {
  // refactor List<int> when using multi-tab
  // this singleton is only for test
  List<String> connectionIds = List.empty(growable: true);
  var initialIndex = 0.obs;

  _FileManagerTabPageState(Map<String, dynamic> params) {
    if (params['id'] != null) {
      connectionIds.add(params['id']);
    }
  }

  @override
  void initState() {
    super.initState();
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_file_transfer") {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        final indexOf = connectionIds.indexOf(id);
        if (indexOf >= 0) {
          initialIndex.value = indexOf;
        } else {
          connectionIds.add(id);
          initialIndex.value = connectionIds.length - 1;
        }
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Obx(
        ()=> DefaultTabController(
          initialIndex: initialIndex.value,
          length: connectionIds.length,
          animationDuration: Duration.zero,
          child: Column(
            children: [
              DesktopTitleBar(
                child: TabBar(
                    isScrollable: true,
                    labelColor: Colors.white,
                    physics: NeverScrollableScrollPhysics(),
                    indicatorColor: Colors.white,
                    tabs: connectionIds
                        .map((e) => Tab(
                              child: Row(
                                mainAxisSize: MainAxisSize.min,
                                crossAxisAlignment: CrossAxisAlignment.center,
                                children: [
                                  Text(e),
                                  SizedBox(
                                    width: 4,
                                  ),
                                  InkWell(
                                      onTap: () {
                                        onRemoveId(e);
                                      },
                                      child: Icon(
                                        Icons.highlight_remove,
                                        size: 20,
                                      ))
                                ],
                              ),
                            ))
                        .toList()),
              ),
              Expanded(
                child: TabBarView(
                    children: connectionIds
                        .map((e) => Container(
                            child: FileManagerPage(
                                key: ValueKey(e),
                                id: e))) //RemotePage(key: ValueKey(e), id: e))
                        .toList()),
              )
            ],
          ),
        ),
      ),
    );
  }

  void onRemoveId(String id) {
    final indexOf = connectionIds.indexOf(id);
    if (indexOf == -1) {
      return;
    }
    connectionIds.removeAt(indexOf);
    initialIndex.value = max(0, initialIndex.value - 1);
  }
}

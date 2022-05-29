import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage>
    with SingleTickerProviderStateMixin {
  // refactor List<int> when using multi-tab
  // this singleton is only for test
  late String connectionId;
  late TabController tabController;

  _ConnectionTabPageState(Map<String, dynamic> params) {
    connectionId = params['id'] ?? "";
  }

  @override
  void initState() {
    super.initState();
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "call ${call.method} with args ${call.arguments} from window ${fromWindowId}");
      // for simplify, just replace connectionId
      if (call.method == "new_remote_desktop") {
        setState(() {
          FFI.close();
          connectionId = jsonDecode(call.arguments)["id"];
        });
      }
    });
    tabController = TabController(length: 1, vsync: this);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        TabBar(
            controller: tabController,
            isScrollable: true,
            labelColor: Colors.black87,
            physics: NeverScrollableScrollPhysics(),
            tabs: [
              Tab(
                text: connectionId,
              ),
            ]),
        Expanded(
            child: TabBarView(controller: tabController, children: [
          RemotePage(key: ValueKey(connectionId), id: connectionId)
        ]))
      ],
    );
  }
}

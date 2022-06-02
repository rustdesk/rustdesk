import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/titlebar_widget.dart';
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
  List<String> connectionIds = List.empty(growable: true);
  var initialIndex = 0;

  _ConnectionTabPageState(Map<String, dynamic> params) {
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
      if (call.method == "new_remote_desktop") {
        setState(() {
          final args = jsonDecode(call.arguments);
          final id = args['id'];
          final indexOf = connectionIds.indexOf(id);
          if (indexOf >= 0) {
            setState(() {
              initialIndex = indexOf;
            });
          } else {
            connectionIds.add(id);
            setState(() {
              initialIndex = connectionIds.length - 1;
            });
          }
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: DefaultTabController(
        initialIndex: initialIndex,
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
                          child: RemotePage(
                              key: ValueKey(e),
                              id: e))) //RemotePage(key: ValueKey(e), id: e))
                      .toList()),
            )
          ],
        ),
      ),
    );
  }

  void onRemoveId(String id) {
    final indexOf = connectionIds.indexOf(id);
    if (indexOf == -1) {
      return;
    }
    setState(() {
      connectionIds.removeAt(indexOf);
      initialIndex = max(0, initialIndex - 1);
    });
  }
}

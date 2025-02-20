import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

void showPeerSelectionDialog(
    {bool singleSelection = false,
    required Function(List<String>) onPeersCallback}) async {
  // load recent peers, we can directly use the peers in `gFFI.recentPeersModel`.
  // The plugin is not used for now, so just left it empty here.
  final peers = '';
  if (peers.isEmpty) {
    // debugPrint("load recent peers failed.");
    return;
  }

  Map<String, dynamic> map = jsonDecode(peers);
  List<dynamic> peersList = map['peers'] ?? [];
  final selected = List<String>.empty(growable: true);

  submit() async {
    onPeersCallback.call(selected);
  }

  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title:
          Text(translate(singleSelection ? "Select peers" : "Select a peer")),
      content: SizedBox(
        height: 300.0,
        child: ListView.builder(
          itemBuilder: (context, index) {
            final Map<String, dynamic> peer = peersList[index];
            final String platform = peer['platform'] ?? "";
            final String id = peer['id'] ?? "";
            final String alias = peer['alias'] ?? "";
            return GestureDetector(
              onTap: () {
                setState(() {
                  if (selected.contains(id)) {
                    selected.remove(id);
                  } else {
                    selected.add(id);
                  }
                });
              },
              child: Container(
                key: ValueKey(index),
                height: 50.0,
                decoration: BoxDecoration(
                    color: Theme.of(context).highlightColor,
                    borderRadius: BorderRadius.circular(12.0)),
                padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 4.0),
                margin: EdgeInsets.symmetric(vertical: 4.0),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  mainAxisSize: MainAxisSize.max,
                  children: [
                    // platform
                    SizedBox(
                      width: 8.0,
                    ),
                    Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        getPlatformImage(platform, size: 34.0),
                      ],
                    ),
                    SizedBox(
                      width: 8.0,
                    ),
                    // id/alias
                    Expanded(child: Text(alias.isEmpty ? id : alias)),
                  ],
                ),
              ),
            );
          },
          itemCount: peersList.length,
          itemExtent: 50.0,
        ),
      ),
      onSubmit: submit,
    );
  });
}

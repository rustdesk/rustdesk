import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';

void showPeerSelectionDialog(
    {bool singleSelection = false,
    required Function(List<String>) onPeersCallback}) {
  final peers = bind.mainLoadRecentPeersSync();
  if (peers.isEmpty) {
    debugPrint("load recent peers sync failed.");
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

import 'package:flutter/material.dart';
import 'package:flutter_hbb/main.dart';
import 'package:get/get.dart';

class RefreshWrapper extends StatefulWidget {
  final Widget Function(BuildContext context) builder;
  const RefreshWrapper({super.key, required this.builder});

  @override
  State<RefreshWrapper> createState() => RefreshWrapperState();

  static RefreshWrapperState? of(BuildContext context) {
    final state = context.findAncestorStateOfType<RefreshWrapperState>();
    if (state == null) {
      debugPrint(
          "RefreshWrapperState not exists in this context, perhaps RefreshWrapper is not exists?");
    }
    return state;
  }
}

class RefreshWrapperState extends State<RefreshWrapper> {
  @override
  Widget build(BuildContext context) {
    return widget.builder(context);
  }

  rebuild() {
    debugPrint("=====Global State Rebuild (win-${kWindowId ?? 'main'})=====");
    if (Get.context != null) {
      (context as Element).visitChildren(_rebuildElement);
    }
    setState(() {});
  }

  /// set root tree dirty to trigger global rebuild
  void _rebuildElement(Element el) {
    el.markNeedsBuild();
    el.visitChildren(_rebuildElement);
  }
}

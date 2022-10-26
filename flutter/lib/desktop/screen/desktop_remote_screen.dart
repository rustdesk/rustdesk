import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/remote_tab_page.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

/// multi-tab desktop remote screen
class DesktopRemoteScreen extends StatelessWidget {
  final Map<String, dynamic> params;

  const DesktopRemoteScreen({Key? key, required this.params}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    RxBool fullscreen = false.obs;
    Get.put(fullscreen, tag: 'fullscreen');
    return MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: gFFI.ffiModel),
          ChangeNotifierProvider.value(value: gFFI.imageModel),
          ChangeNotifierProvider.value(value: gFFI.cursorModel),
          ChangeNotifierProvider.value(value: gFFI.canvasModel),
        ],
        child: Scaffold(
          body: ConnectionTabPage(
            params: params,
          ),
        ));
  }
}

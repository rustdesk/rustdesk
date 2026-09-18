import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/usbip_tab_page.dart';
import 'package:provider/provider.dart';

/// multi-tab USB forwarding screen
class DesktopUsbipScreen extends StatelessWidget {
  final Map<String, dynamic> params;

  const DesktopUsbipScreen({Key? key, required this.params}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: gFFI.ffiModel),
      ],
      child: Scaffold(
        backgroundColor: isLinux ? Colors.transparent : null,
        body: UsbipTabPage(
          params: params,
        ),
      ),
    );
  }
}

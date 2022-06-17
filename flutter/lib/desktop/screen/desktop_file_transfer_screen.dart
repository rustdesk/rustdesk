import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_tab_page.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:provider/provider.dart';

/// multi-tab file transfer remote screen
class DesktopFileTransferScreen extends StatelessWidget {
  final Map<String, dynamic> params;

  const DesktopFileTransferScreen({Key? key, required this.params})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: gFFI.ffiModel),
        ChangeNotifierProvider.value(value: gFFI.imageModel),
        ChangeNotifierProvider.value(value: gFFI.cursorModel),
        ChangeNotifierProvider.value(value: gFFI.canvasModel),
      ],
      child: MaterialApp(
          navigatorKey: globalKey,
          debugShowCheckedModeBanner: false,
          title: 'RustDesk - File Transfer',
          theme: ThemeData(
            primarySwatch: Colors.blue,
            visualDensity: VisualDensity.adaptivePlatformDensity,
          ),
          home: FileManagerTabPage(
            params: params,
          ),
          navigatorObservers: [
            // FirebaseAnalyticsObserver(analytics: analytics),
            FlutterSmartDialog.observer
          ],
          builder: FlutterSmartDialog.init(
              builder: isAndroid
                  ? (_, child) => AccessibilityListener(
                        child: child,
                      )
                  : null)),
    );
  }
}
